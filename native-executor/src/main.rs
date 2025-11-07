#![allow(clippy::missing_errors_doc)]
mod utils;

use std::mem::MaybeUninit;

use parity_scale_codec::Encode;
use wasm_types::{BoundedString, Kind as MessageKind, Message};
use wasmtime::{
    AsContext, Caller, Config, Engine, Extern, Func, InstanceAllocationStrategy, Linker, Memory,
    MemoryType, Module, OptLevel, PoolingAllocationConfig, ProfilingStrategy, Store,
};

// Código WebAssembly em formato de texto.
const WAT_CODE: &[u8] = include_bytes!("../../wasm_runtime.wat");

// Um megabyte em bytes.
const MEGABYTE: usize = 1024 * 1024;

// 64kb é o tamanho da "página de memória" no WebAssembly.
// A memória total disponível SEMPRE é multipla desse valor.
// Referencia:
// https://webassembly.github.io/spec/core/exec/runtime.html#memory-instances
const WASM_PAGE_SIZE: u64 = 65536;

// Número máximo de páginas de memória que pode ser utilizado pelo WASM.
const MAX_WASM_PAGES: u64 = 0x10000;

// Quantidade máxima de memória em bytes que pode ser utilizado pelo WASM.
#[allow(clippy::cast_possible_truncation)]
const MAX_MEMORY_SIZE: usize = MAX_WASM_PAGES.saturating_mul(WASM_PAGE_SIZE) as usize;

// Número máximo de "instancias" que podem rodar em paralelo.
const MAX_INSTANCE_COUNT: u32 = 8;

/// Estado compartilhado entre o Host e a Instância WASM
pub struct State {
    /// Memoria que será IMPORTADA na instância WASM, a memória é criada
    /// antes da instância e precisa estar armazenada aqui para poder ser
    /// acessada dentro de funções importadas.
    pub memory: Memory,
}

impl State {
    /// Cria um `Store<State>` garantido que o State tem uma memória válida.
    ///
    /// # Contexto
    /// Para criar um `State` precisamos do `wasmtime::Memory`, para criar `wasmtime::Memory`
    /// precisamos do `wasmtime::Store`, que para ser criado precisa do `State`... temos um
    /// problema de dependencia cíclica, a forma comum de resolver isso é tornar algumas das
    /// dependencias opcional, só podemos alterar o `State`, portanto seria necessário alterar
    /// o campo `memory` para o tipo `Option<Memory>`, então criamos um `State` com `None` para
    /// só inicializa-lo no final, a desvantagem é que isso obriga esse `Option<Memory>` ser
    /// tratado em todo lugar que precisar ler esse valor, mesmo sendo 100% garantido que esse
    /// valor jamais será `None`.
    ///
    /// A solução escolhida usa o `MaybeUninit` para criar um `wasmtime::Memory` não-inicializado,
    /// requer `unsafe` pois o programador deve garantir que a memória não será utilizada até ser
    /// devidamente inicializada, como o Wasmtime não utiliza o `State` apenas precisa armazena-lo
    /// no `wasmtime::Store`, então é seguro utilizar o `MaybeUninit` para evitar ter que utilizar
    /// `match` ou `unwrap` no restante do código.
    ///
    /// # Passos
    /// Cria-se o `State` fingindo que o campo `memory` foi inicializado com `MaybeUninit`, então
    /// cria-se o `wasmtime::Store` e o `wasmtime::Memory` que é utilizado para substituir a
    /// `memory` que fingimos ter inicializado antes, agora não precisamos tratar
    ///
    /// Referencias:
    /// - <https://github.com/bytecodealliance/wasmtime/issues/4922#issuecomment-1251086171>
    /// - <https://github.com/bytecodealliance/wasmtime/issues/9579>
    /// - <https://doc.rust-lang.org/std/mem/union.MaybeUninit.html>
    pub fn new(engine: &Engine, memory_type: MemoryType) -> anyhow::Result<Store<Self>> {
        let state = Self {
            // SAFETY: A memória será inicializada manualmente mais abaixo.
            #[allow(invalid_value, clippy::uninit_assumed_init)]
            memory: unsafe { MaybeUninit::<Memory>::zeroed().assume_init() },
        };

        // Cria-se o `wastime::Store` com o `State`.
        let mut store = Store::new(engine, state);

        // Cria-se o `wastime::Memory` com o `wastime::Store`, que
        // sobreecreve o campo `memory` não-inicializado do `State`.
        // Ref: https://doc.rust-lang.org/std/mem/union.MaybeUninit.html#initializing-a-struct-field-by-field
        unsafe {
            // Encontra o endereço de memória do `state.memory`.
            let ptr = std::ptr::addr_of_mut!(store.data_mut().memory);
            // Cria e substitui a memória "fake" pela memória válida.
            ptr.write(Memory::new(&mut store, memory_type)?);
        }

        Ok(store)
    }

    #[must_use]
    pub const fn memory(&self) -> Memory {
        self.memory
    }
}

fn main() -> anyhow::Result<()> {
    ////////////////////////////////////////
    // Configura o compilador WebAssembly //
    ////////////////////////////////////////
    // Referencias:
    // https://github.com/paritytech/polkadot-sdk/blob/polkadot-stable2509-rc3/substrate/client/executor/wasmtime/src/runtime.rs#L219-L279
    // https://github.com/paritytech/polkadot-sdk/blob/polkadot-stable2509-rc4/substrate/client/executor/src/wasm_runtime.rs#L304-L323
    let mut config = Config::new();
    // Otimize o código para velocidade e tamanho.
    config.cranelift_opt_level(OptLevel::SpeedAndSize);

    // Permite compilar o código usando várias threads.
    config.parallel_compilation(true);

    // Configura o tamanho máximo da stack para 4 megabytes.
    config.max_wasm_stack(4 * MEGABYTE);

    // Desativa o suporte a Garbage Collector
    config.gc_support(false);
    config.wasm_gc(false);

    // Desativa algumas features opicionais do WebAssembly.
    config.wasm_stack_switching(false);
    config.wasm_reference_types(false);
    config.cranelift_nan_canonicalization(false);
    config.wasm_simd(false);
    config.wasm_relaxed_simd(false);
    config.relaxed_simd_deterministic(false);
    config.wasm_bulk_memory(true);
    config.wasm_multi_value(false);
    config.wasm_multi_memory(false);
    config.wasm_threads(false);
    config.wasm_memory64(false);
    config.wasm_tail_call(false);
    config.wasm_extended_const(false);
    config.profiler(ProfilingStrategy::None);

    // Configura os limites de memória por instância
    config.memory_guaranteed_dense_image_size(u64::MAX);
    let mut pooling_config = PoolingAllocationConfig::default();
    pooling_config
        .max_unused_warm_slots(4)
        //   size: 32384
        //   table_elements: 1249
        //   memory_pages: 2070
        .max_core_instance_size(512 * 1024)
        .table_elements(8192)
        .max_memory_size(MAX_MEMORY_SIZE)
        .total_tables(MAX_INSTANCE_COUNT)
        .total_memories(MAX_INSTANCE_COUNT)
        // Determina quantas instâncias no máximo podem existir
        // em paralello desse mesmo módulo.
        .total_core_instances(MAX_INSTANCE_COUNT);
    config.allocation_strategy(InstanceAllocationStrategy::Pooling(pooling_config));

    // Cria a Engine usando a configuração que escolhemos.
    let engine = Engine::new(&config)?;

    //////////////////////////////////
    // Compila o código WebAssembly //
    //////////////////////////////////
    let module = Module::new(&engine, WAT_CODE)?;

    // Configura os limites de memória.
    //
    // Cada página tem 64KB, o programa inicia com 2 página de memória, que pode ser
    // expandido para no máximo 16 páginas pelo código. É possível expandir a memória
    // a partir do webassembly com o método `core::arch::wasm32::memory_grow`.
    // ref: https://doc.rust-lang.org/core/arch/wasm32/fn.memory_grow.html
    let memory_type = MemoryType::new(2, Some(16));

    // Inicia um Store, utilizado para compartilhar um estado entre
    // o host e o WebAssembly.
    let mut store = State::new(&engine, memory_type)?;

    // Imprime o que é exportado e importado pelo WASM.
    utils::print_module_details(&module);

    // Define a função `console_log` que será importada e chamada pelo WebAssembly.
    // - `offset` é o endereço de memória onde a string começa, a string deve estar em formato utf-8
    // - `length` é o tamanho da string em bytes.
    #[allow(clippy::cast_possible_truncation)]
    let console_log_func =
        Func::wrap(&mut store, |caller: Caller<'_, State>, offset: u32, length: u32| {
            // Recupera o contexto, que utilizaremos para ler a memória.
            let ctx = caller.as_context();

            // Define o intervalo de memória que será lido.
            let start = usize::try_from(offset).unwrap_or(usize::MAX);
            let end = start.saturating_add(length as usize);

            // Verifica se o intervalo de memória está dentro dos limites da memória.
            let Some(bytes) = ctx.data().memory.data(&ctx).get(start..end) else {
                anyhow::bail!("out of bounds memory access");
            };

            // Converte os bytes lidos para uma string utf-8.
            let Ok(string) = std::str::from_utf8(bytes) else {
                anyhow::bail!("invalid utf-8 string");
            };

            // Imprime a string.
            println!("{string}");

            // Retorna Ok(()) para indicar que essa função foi executada com sucesso.
            Ok(())
        });

    // Imports do módulo WebAssembly.
    let mut linker = Linker::<State>::new(&engine);
    let memory = Extern::Memory(store.data().memory);
    linker.define(&mut store, "env", "memory", memory)?;
    linker.define(&mut store, "env", "console_log", console_log_func)?;

    // Cria uma instância do módulo WebAssembly
    let instance = linker.instantiate(&mut store, &module)?;

    /////////////////////////////////////////////////
    // Extrai a função `add` do módulo WebAssembly //
    /////////////////////////////////////////////////
    // obs: veja o código WebAssembly em `wasm_runtime/src/lib.rs` para
    // entender como a função `add` foi definida.
    let export_name = "add";
    let run = instance.get_typed_func::<(u32, u32), u32>(&mut store, export_name)?;

    //////////////////////////
    // Chama a função `add` //
    //////////////////////////
    println!("Chamando o método {export_name:?}...");
    println!("---------------------------------------------");
    let result = run.call(&mut store, (15, 10))?;
    println!("---------------------------------------------");
    println!("result = {result}\n\n");

    //////////////////////////////////////////////////
    // Extrai a função `call` do módulo WebAssembly //
    //////////////////////////////////////////////////
    // obs: veja o código WebAssembly em `wasm_runtime/src/lib.rs` para
    // entender como a função `add` foi definida.
    let export_name = "call";
    let run = instance.get_typed_func::<(u32, u32), u32>(&mut store, export_name)?;

    // Serializa uma struct para envia-la para o WebAssembly.
    let (offset, length) = {
        // Serializa o tipo `Message` em um vetor de bytes
        let message =
            Message { kind: MessageKind::Ping, message: BoundedString::from("message from host") };
        let encoded = message.encode();
        println!("mensagem: {message:?}");
        println!("encodada: {}", const_hex::encode_prefixed(&encoded));

        // Escreve a mensagem encodada na memoria do WebAssembly
        let ptr = 128;
        let memory_mut = store.data().memory();
        memory_mut.write(&mut store, ptr, &encoded)?;

        // Indica onde inicia a mensagem e o seu tamanho em bytes.
        let ptr = u32::try_from(ptr)?;
        let len = u32::try_from(encoded.len())?;
        (ptr, len)
    };

    ///////////////////////////
    // Chama a função `call` //
    ///////////////////////////
    println!();
    println!("Chamando o método {export_name:?}...");
    println!("---------------------------------------------");
    let result = run.call(&mut store, (offset, length))?;
    println!("---------------------------------------------");
    println!("result = {result}");
    Ok(())
}

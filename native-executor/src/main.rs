use std::mem::MaybeUninit;

use wasmtime::{
    AsContext, Caller, Config, Engine, Func, Instance, InstanceAllocationStrategy, Memory,
    MemoryType, Module, OptLevel, PoolingAllocationConfig, ProfilingStrategy, Store,
};

// Código WebAssembly em formato de texto.
const WAT_CODE: &[u8] = include_bytes!("../../wasm_runtime.wat");

const MEGABYTE: usize = 1024 * 1024;
const WASM_PAGE_SIZE: u64 = 65536;
const MAX_WASM_PAGES: u64 = 0x10000;
const MAX_INSTANCE_COUNT: u32 = 8;
#[allow(clippy::cast_possible_truncation)]
const MAX_MEMORY_SIZE: usize = MAX_WASM_PAGES.saturating_mul(WASM_PAGE_SIZE) as usize;

/// Estado da instância
pub struct State {
    pub memory: Memory,
}

impl State {
    #[allow(invalid_value, clippy::missing_errors_doc)]
    /// Inicializa o estado, store e memória.
    pub fn new(engine: &Engine, memory_type: MemoryType) -> anyhow::Result<Store<Self>> {
        // O `Store` precisa do `State` para ser criado, e a memória precisa do `Store` para ser
        // criada, e o `State` precisa da memória, logo temos uma dependencia cíclica, para
        // resolver esse dilema o rust nos fornece o tipo `MaybeUninit`, que é utilizado
        // para inicializar um valor com um valor "nulo" que é parecido com um `null`, porém
        // dever ser utilizado com cuidado, depois de criado pode inicializar o `State`.
        // Referencias:
        // - https://github.com/bytecodealliance/wasmtime/issues/4922#issuecomment-1251086171
        // - https://doc.rust-lang.org/std/mem/union.MaybeUninit.html
        #[allow(clippy::uninit_assumed_init)]
        let state = Self {
            // SAFETY: A `memory` será inicializada corretamente abaixo.
            memory: unsafe { MaybeUninit::<Memory>::zeroed().assume_init() },
        };

        // Agora podemos criar o `Store` com o `State`.
        let mut store = Store::new(engine, state);

        // Agora podemos criar a memória e inicializar o `State`.
        // Ref: https://doc.rust-lang.org/std/mem/union.MaybeUninit.html#initializing-a-struct-field-by-field
        unsafe {
            let ptr = std::ptr::addr_of_mut!(store.data_mut().memory);
            ptr.write(Memory::new(&mut store, memory_type)?);
        }

        Ok(store)
    }
}

fn main() -> anyhow::Result<()> {
    ////////////////////////////////////////
    // Configura o compilador WebAssembly //
    ////////////////////////////////////////
    // Referencia:
    // https://github.com/paritytech/polkadot-sdk/blob/polkadot-stable2509-rc3/substrate/client/executor/wasmtime/src/runtime.rs#L219-L279
    // https://github.com/paritytech/polkadot-sdk/blob/polkadot-stable2509-rc4/substrate/client/executor/src/wasm_runtime.rs#L304-L323

    let mut config = Config::new();
    // Otimize para velocidade e tamanho.
    config.cranelift_opt_level(OptLevel::SpeedAndSize);

    // Configura o tamanho máximo da stack para 4 megabytes.
    config.max_wasm_stack(4 * MEGABYTE);
    config.wasm_stack_switching(false);
    config.wasm_reference_types(false);

    // Desativa algumas o suporte a Garbage Collector
    config.gc_support(false);
    config.wasm_gc(false);

    // Desativa algumas features opicionais do WebAssembly.
    config.cranelift_nan_canonicalization(false);
    config.wasm_simd(false);
    config.parallel_compilation(true);
    config.wasm_relaxed_simd(false);
    config.relaxed_simd_deterministic(false);
    config.wasm_bulk_memory(false);
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

    // Configura a Engine com as opções definidas.
    let engine = Engine::new(&config)?;

    //////////////////////////////////
    // Compila o código WebAssembly //
    //////////////////////////////////
    let module = Module::new(&engine, WAT_CODE)?;

    // Configura a memória que será utilizada pelo WebAssembly.
    //
    // Cada página tem 64KB, aqui foi configurado 2 página de memória, que pode ser
    // expandido para no máximo 16 páginas pelo código. É possível expandir a memória
    // chamando de dentro do webassembly o método `core::arch::wasm32::memory_grow`.
    // ref: https://doc.rust-lang.org/core/arch/wasm32/fn.memory_grow.html
    let memory_type = MemoryType::new(2, Some(16));

    // Inicia um Store, utilizado para compartilhar um estado entre
    // o host e o WebAssembly.
    let mut store = State::new(&engine, memory_type)?;

    // Define uma função que pode ser chamada pelo WebAssembly.
    // - `offset` é o endereço de memória onde a string começa.
    // - `len` é o tamanho da string em bytes.
    // obs: A string deve estar encodada em utf-8.
    #[allow(clippy::cast_possible_truncation)]
    let console_log_func =
        Func::wrap(&mut store, |caller: Caller<'_, State>, offset: u32, len: u32| {
            // Convert o `Caller` para um contexto, que utilizaremos para ler a memória.
            let ctx = caller.as_context();

            // Define o intervalo de memória que será lido.
            let start = usize::try_from(offset).unwrap_or(usize::MAX);
            let end = start.saturating_add(len as usize);

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

            // Retorna Ok(()) para indicar que a closure foi executada com sucesso.
            Ok(())
        });

    // Imports do módulo WebAssembly.
    let memory = store.data().memory;
    let imports = [memory.into(), console_log_func.into()];

    // Cria uma instância do módulo WebAssembly
    let instance = Instance::new(&mut store, &module, &imports)?;

    /////////////////////////////////////////////////
    // Extrai a função `add` do módulo WebAssembly //
    /////////////////////////////////////////////////
    // obs: veja o código WebAssembly em `wasm_runtime/src/lib.rs` para
    // entender como a função `add` foi definida.
    let run = instance.get_typed_func::<(u32, u32), u32>(&mut store, "add")?;

    //////////////////////////
    // Chama a função `add` //
    //////////////////////////
    let result = run.call(&mut store, (15, 10))?;

    println!("result = {result}");
    Ok(())
}

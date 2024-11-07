use std::mem::MaybeUninit;

use wasmtime::{Config, Engine, Instance, Module, OptLevel, Memory, MemoryType, Caller, Func, Store, AsContext};

// Código WebAssembly em formato de texto.
const WAT_CODE: &[u8] = include_bytes!("../../wasm_runtime.wat");

struct State {
    // `MaybeUninit` é utilizado para inicializar o estado com um valor nulo.
    // Ex: A depende de B, e B depende de A, então é necessário inicializar um dos dois com um valor nulo.
    memory: MaybeUninit<Memory>,
}

impl State {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            memory: MaybeUninit::uninit(),
        }
    }

    pub fn init(&mut self, memory: Memory) {
        self.memory = MaybeUninit::new(memory);
    }

    pub const fn memory(&self) -> &Memory {
        // SAFETY: `memory` foi inicializado.
        unsafe { self.memory.assume_init_ref() }
    }
}

fn main() -> anyhow::Result<()> {
    ////////////////////////////////////////
    // Configura o compilador WebAssembly //
    ////////////////////////////////////////
    let mut config = Config::new();
    // Otimize para velocidade e tamanho.
    config.cranelift_opt_level(OptLevel::SpeedAndSize);
    // Desativa algumas features opicionais do WebAssembly.
    config.cranelift_nan_canonicalization(false);
    config.wasm_tail_call(false);
    config.parallel_compilation(true);
    config.wasm_multi_value(false);
    config.wasm_multi_memory(false);
    config.wasm_bulk_memory(true);
    // config.wasm_reference_types(false);
    // config.wasm_threads(false);
    config.wasm_relaxed_simd(false);
    config.wasm_simd(false);

    // Configura a Engine com as opções definidas.
    let engine = Engine::new(&config)?;

    //////////////////////////////////
    // Compila o código WebAssembly //
    //////////////////////////////////
    let module = Module::new(&engine, WAT_CODE)?;

    // Inicia um Store, utilizado para compartilhar um estado entre
    // o host e o WebAssembly. (não é necessário para este exemplo)
    let mut store = Store::new(&engine, State::new());

    // Configura a memória que será utilizada pelo WebAssembly.
    //
    // Cada página tem 64KB, aqui foi configurado 2 página de memória, que pode ser
    // expandido para no máximo 16 páginas pelo código. É possível expandir a memória
    // chamando de dentro do webassembly o método `core::arch::wasm32::memory.grow`.
    // ref: https://doc.rust-lang.org/core/arch/wasm32/fn.memory_grow.html
    let memory_type = MemoryType::new(2, Some(16));
    let memory = Memory::new(&mut store, memory_type)?;

    // Inicializa o estado com a memória criada.
    store.data_mut().init(memory);

    // Define uma função que pode ser chamada pelo WebAssembly.
    #[allow(clippy::cast_possible_truncation)]
    let hello_func = Func::wrap(&mut store, |caller: Caller<'_, State>, offset: u32, len: u32| {
        // Convert o `Caller` para um contexto, que utilizaremos para ler a memória.
        let ctx = caller.as_context();

        // Le a memória do host.
        let memory = ctx.data().memory();

        // Define o intervalo de memória que será lido.
        let start = usize::try_from(offset).unwrap_or(usize::MAX);
        let end = start.saturating_add(len as usize);

        // Verifica se o intervalo de memória está dentro dos limites da memória.
        let Some(bytes) = memory.data(&ctx).get(start..end) else {
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

    // Imports do módulo WebAssembly
    let imports = [memory.into(), hello_func.into()];

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

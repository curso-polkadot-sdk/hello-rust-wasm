use core::str;
use std::usize;

use wasmtime::{Config, Engine, Instance, Module, OptLevel, Memory, MemoryType, Caller, Func, Store, AsContext};

// Código WebAssembly em formato de texto.
const WAT_CODE: &[u8] = include_bytes!("../../wasm_runtime.wat");

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
    let mut store = Store::new(&engine, ());

    // Configura a memória que será utilizada pelo WebAssembly.
    //
    // Cada página tem 64KB, aqui foi configurado 2 página de memória, que pode ser
    // expandido para no máximo 16 páginas pelo código. É possível expandir a memória
    // chamando de dentro do webassembly o método `core::arch::wasm32::memory.grow`.
    // ref: https://doc.rust-lang.org/core/arch/wasm32/fn.memory_grow.html
    let memory_type = MemoryType::new(2, Some(16));
    let memory = Memory::new(&mut store, memory_type)?;

    // Define uma função que pode ser chamada pelo WebAssembly.
    #[allow(clippy::cast_possible_truncation)]
    let hello_func = Func::wrap(&mut store, move |caller: Caller<'_, ()>, offset: u32, len: u32| {
        // Captura o `memory` para que possamos ler a memória de dentro dessa função.
        // Obs: só é possível capturar quando a closure é anotada com `move`.
        // veja: https://doc.rust-lang.org/book/ch13-01-closures.html#closure-type-inference-and-annotation
        let memory = memory;

        // Convert o `Caller` para um contexto, que utilizaremos para ler a memória.
        let ctx = caller.as_context();

        // Define o intervalo de memória que será lido.
        let start = usize::try_from(offset).unwrap_or(usize::MAX);
        let end = start.saturating_add(len as usize);

        // Verifica se o intervalo de memória está dentro dos limites da memória.
        let Some(data) = memory.data(&ctx).get(start..end) else {
            anyhow::bail!("out of bounds memory access");
        };

        // Converte os bytes lidos para uma string utf-8.
        let Ok(string) = str::from_utf8(data) else {
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

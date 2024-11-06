use wasmtime::{Config, Engine, Instance, Module, OptLevel, Store};

// Código WebAssembly em formato de texto.
const WAT_CODE: &[u8] = include_bytes!("../../wasm_runtime.wat");

fn main() -> anyhow::Result<()> {
    // Configura o compilador WebAssembly.
    let mut config = Config::new();
    // Otimize para velocidade e tamanho.
    config.cranelift_opt_level(OptLevel::SpeedAndSize);
    // Desativa algumas funcionalidades que opicionais do WebAssembly.
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

    // Compila o código WebAssembly.
    // .exe .app
    let module = Module::new(&engine, WAT_CODE)?;

    // Inicia um Store com o estado inicial.
    let mut store = Store::new(&engine, ());

    // Cria uma instância do módulo WebAssembly.
    let instance = Instance::new(&mut store, &module, &[])?;

    // Extrai a função `add` do módulo WebAssembly.
    let run = instance.get_typed_func::<(u32, u32), u32>(&mut store, "add")?;

    // Chama a função `add` com os argumentos 15 e 20.
    // obs: veja o código WebAssembly em `wasm_runtime/src/lib.rs`
    let result = run.call(&mut store, (15, 20))?;

    println!("result = {result}");
    Ok(())
}

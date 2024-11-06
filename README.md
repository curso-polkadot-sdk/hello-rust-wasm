# Hello Rust Wasm

Projeto demonstrando como compilar um código em rust para WebAssembly, e como executar esse binário também utilizando rust em um projeto separado. Esse workspace é composto de dois projetos:

- `wasm-runtime`: Código `no_std` que pode ser compilado para WebAssembly (a.k.a. `wasm32-unknown-unknown`)
- `native-executor`: Aplicação nativa que compila e executa o código WebAssembly.


## Passo 1 - Dependencias
- Certifique que o Rust esta instalado na sua maquina.
- Instale o target para WASM `rustup target add wasm32-unknown-unknown`
- Instale o otimizador de WASM `cargo install wasm-opt`
```sh
# Instalar Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Instalar o target `wasm32-unknown-unknown`, para o rust
# ser capaz de compilar seu código para WASM.
rustup target add wasm32-unknown-unknown

# Instalar o `wasm-opt`, utilizado para gerar o arquivo .wat,
# também otimiza e reduz o tamanho do seu código WASM final.
cargo install wasm-opt
```

Rode os testes para garantir que esta tudo funcionando:
```sh
# Roda os testes de todos os projetos definidos no
# `Cargo.toml` desse workspace.
cargo test -p wasm-runtime

# Formatar os arquivos do projeto (obs: precisa the nightly)
cargo +nightly fmt

# Verificar boas práticas no código
# Obs: pode ser lento dependendo da maquina.
cargo clippy --workspace --tests --all-features -- \
    -Dwarnings \
    -Dclippy::unwrap_used \
    -Dclippy::expect_used \
    -Dclippy::nursery \
    -Dclippy::pedantic \
    -Aclippy::module_name_repetitions
```

## Passo 2 - Compile o WASM
Nesse passo vamos compilar o `wasm-runtime` para gerar um arquivo `.wasm`, há duas formas diferentes de compilar o código.

#### Opção 1 - Usando o build.sh (recomendado)
Para compilar o projeto em wasm utilize o script `./build.sh`
```sh
./build.sh
```

#### Opção 2 - Manualmente
Obs: todos os comandos devem ser executados a partir da raiz do projeto.
```sh
# Compila o projeto `wasm-runtime` para WASM
cargo build -p wasm-runtime --release --target=wasm32-unknown-unknown

# Copia o binário WASM gerado para a raiz do projeto
cp ./target/wasm32-unknown-unknown/release/wasm_runtime.wasm ./
```

Os passos a seguir são opcionais, necessários só se quiser otimizar ou gerar o arquivo `.wat`:
```sh
# Otimizar o WASM final com o wasm-opt (também reduz o tamanho do binário)
wasm-opt -O3 --dce --precompute --precompute-propagate --optimize-instructions --optimize-casts --strip --strip-debug \
    --output ./wasm_runtime.wasm \
    ./target/wasm32-unknown-unknown/release/wasm_runtime.wasm

# Gerar o arquivo .WAT (WASM legível em formato texto)
wasm-opt -O3 --dce --precompute --precompute-propagate --optimize-instructions --optimize-casts --strip --strip-debug \
    --emit-text \
    --output ./wasm_runtime.wat \
    ./target/wasm32-unknown-unknown/release/wasm_runtime.wasm
```

## Passo 3 - Executar WASM
Certifique-se que no arquivo `native-executor/src/main.rs` aponta para o arquivo WASM correto na raiz do projeto, você pode utilizar tanto o arquivo `.wat` ou `.wasm`. Então execute o `native-executor` para rodar o seu código em WASM.
```sh
cargo run -p native-executor
```
No final você deve ver o resultado da execusão do seu WebAssembly.

<!-- Para quem esta se perguntando o que da para fazer em WebAssembly, um exemplo é *Ray Tracing* no navegador:
* https://github.com/rustwasm/wasm-bindgen/tree/main/examples/raytrace-parallel
* Exemplo onlinehttps://wasm-bindgen.netlify.app/exbuild/raytrace-parallel/ -->

**Dicas de estudos:**
1. Modifique a função `add` no arquivo `wasm-runtime/src/libs.rs`, compile o WASM denovo, e observe como o comportamento muda ao executa-lo novamente.
2. Crie outras funções no `wasm-runtime` e chame elas a partir do `native-executor`.
3. Modifique o exemplo para chamar funções do `native-executor` a partir do `wasm-runtime` para você imprimir um "hello world" na tela: https://docs.wasmtime.dev/examples-rust-hello-world.html
4. Siga o exemplo na documentação do wasmtime para fornecer uma quantidade de memória finita para seu WASM: https://docs.wasmtime.dev/examples-rust-memory.html
5. Use a memória para ler strings criadas dentro do webassembly.

## Material Complementar
- Outras linguagens que também compilam para WASM: https://github.com/appcypher/awesome-wasm-langs
- Tutoriais, compiladores e outras ferramentas para WASM: https://github.com/mbasso/awesome-wasm
- Projeto que executa WebAssembly em javascript, funciona tanto a partir do Navegador quanto no NodeJS. https://gist.github.com/Lohann/209894dc55ed9eb6bd7b3465ad3a81cc
- Comparação entre os diferentes WebAssembly runtimes: https://00f.net/2023/01/04/webassembly-benchmark-2023/

## Outros Runtimes:
- [wasmtime](https://github.com/bytecodealliance/wasmtime) é considerado o mais seguro, por isso é utilizado no substrate:
> CVEs for quite a few vulnerabilities in wasmtime have been assigned. Wasmtime did responsible security disclosure after every vulnerability, and went above and beyond to prevent similar vulnerabilities from happening again.
>
> Changes are made carefully, and the project is also constantly being fuzzed to discover new bugs.
>
> If the intent is to run arbitrary, untrusted code outside a browser environment, wasmtime feels like the most secure option.
- [wasmer](https://github.com/wasmerio/wasmer) tem o maior ecosistema, mais bibliotecas e ferramentas utilizando ele.
- [iwasm](https://github.com/bytecodealliance/wasm-micro-runtime/tree/main/product-mini) é o mais rápido, também é bem pequeno (50 KB), e pode ser utilizado para rodar WASM em dispositvos embarcados, como IoT, celular, etc.

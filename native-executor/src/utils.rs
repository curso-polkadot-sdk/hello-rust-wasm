use std::fmt::{Display, Formatter, Result as FmtResult, Write};
use wasmtime::{ExternType, FuncType, Module, Mutability};

/// Imprime informações sobre o módulo WASM, incluindo o tipos que devem ser importados e
/// os tipos que são exportados.
pub fn print_module_details(module: &Module) {
    println!("Dados que devem ser importados: ");
    for import in module.imports() {
        print!(" - \"{}\" \"{}\" ", import.module(), import.name());
        match import.ty() {
            ExternType::Func(func_type) => println!("{func_type}"),
            ExternType::Global(global_type) => println!("(global {})", global_type.content()),
            ExternType::Table(table_type) => match table_type.maximum() {
                Some(maximum) => println!("(table {} {maximum})", table_type.minimum()),
                None => println!("(table {})", table_type.minimum()),
            },
            ExternType::Memory(memory_type) => match memory_type.maximum() {
                Some(maximum) => println!("(memory {} {maximum})", memory_type.minimum()),
                None => println!("(memory {})", memory_type.minimum()),
            },
            ExternType::Tag(tag_type) => println!("{}", tag_type.ty()),
        }
    }
    println!();
    println!("Dados exportados pelo WASM: ");
    for export in module.exports() {
        print!(" - ");
        if !matches!(export.ty(), ExternType::Func(_) | ExternType::Global(_)) {
            print!("{} ", export.name());
        }
        match export.ty() {
            ExternType::Func(func_type) => {
                println!("{}", Wasm2RustFn::fmt_fn(export.name(), func_type));
            },
            ExternType::Global(global_type) => match global_type.mutability() {
                Mutability::Const => println!("const {}: {}", export.name(), global_type.content()),
                Mutability::Var => {
                    println!("static mut {}: {}", export.name(), global_type.content());
                },
            },
            ExternType::Table(table_type) => match table_type.maximum() {
                Some(maximum) => println!("(table {} {maximum})", table_type.minimum()),
                None => println!("(table {})", table_type.minimum()),
            },
            ExternType::Memory(memory_type) => match memory_type.maximum() {
                Some(maximum) => println!("(memory {} {maximum})", memory_type.minimum()),
                None => println!("(memory {})", memory_type.minimum()),
            },
            ExternType::Tag(tag_type) => println!("{}", tag_type.ty()),
        }
    }
    println!();
}

/// struct auxiliar que implementa `Display` e transforma uma declaração
/// de função em Wasm `FuncType` em uma declaração de função em Rust.
struct Wasm2RustFn<'a> {
    name: &'a str,
    func: FuncType,
}

impl<'a> Wasm2RustFn<'a> {
    // Formata o `FuncType` como uma função rust.
    fn fmt_fn(name: &'a str, func: FuncType) -> String {
        format!("{}", Self { name, func })
    }
}

/// Formata uma lista de tipos `T` separando com virgula e espaço.
fn join<T: Display, I: Iterator<Item = T>>(mut items: I, f: &mut Formatter<'_>) -> FmtResult {
    if let Some(item) = items.next() {
        T::fmt(&item, f)?;
    } else {
        return Ok(());
    }
    for item in items {
        f.write_char(',')?;
        f.write_char(' ')?;
        T::fmt(&item, f)?;
    }
    Ok(())
}

impl Display for Wasm2RustFn<'_> {
    /// Faz a formatação da função rust.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> FmtResult {
        f.write_str("fn ")?;
        <str as std::fmt::Display>::fmt(self.name, f)?;

        // Parametros da função.
        f.write_char('(')?;
        join(self.func.params(), f)?;
        f.write_char(')')?;

        // Retorno da função caso exista.
        let result_count = self.func.results().count();
        if result_count > 0 {
            f.write_str(" -> ")?;
        }
        if result_count > 1 {
            f.write_char('(')?;
            join(self.func.results(), f)?;
            f.write_char(')')
        } else {
            join(self.func.results(), f)
        }
    }
}

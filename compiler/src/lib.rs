//! Mathless compiler (`mlc`) — Phase 1.
//!
//! Pipeline (see `docs/phase1/WBS.md`): source → **lex/parse (W2)** → typecheck + IR
//! (W3) → codegen (W4). Per D19 codegen lowers a non-Rust IR to `no_std` + `extern "C"`
//! + `repr(C)` Rust, then `cargo build --crate-type cdylib`.

pub mod ast;
pub mod error;
pub mod ir;
pub mod lexer;
pub mod parser;
pub mod typeck;

pub use ast::*;
pub use error::ParseError;
pub use typeck::{check, TypeError};

/// Parse Mathless source into a [`Module`] AST (W2).
pub fn parse(src: &str) -> Result<ast::Module, ParseError> {
    let tokens = lexer::tokenize(src)?;
    parser::parse(tokens)
}

/// Parse then typecheck, returning the typed IR (W2 + W3).
pub fn compile_to_ir(src: &str) -> Result<ir::IrModule, CompileError> {
    let module = parse(src).map_err(CompileError::Parse)?;
    check(&module).map_err(CompileError::Type)
}

/// Compile a `.mls` source string to emitted Rust module source (W4). Not yet implemented.
pub fn compile_to_rust(_src: &str) -> Result<String, CompileError> {
    Err(CompileError::NotImplemented)
}

#[derive(Debug, PartialEq, Eq)]
pub enum CompileError {
    Parse(ParseError),
    Type(TypeError),
    /// Codegen is not implemented until W4.
    NotImplemented,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codegen_is_stubbed_until_w4() {
        assert_eq!(
            compile_to_rust("export fn f() -> f64 { return 0 }"),
            Err(CompileError::NotImplemented)
        );
    }
}

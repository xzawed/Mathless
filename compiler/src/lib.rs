//! Mathless compiler (`mlc`) — Phase 1.
//!
//! Pipeline (see `docs/phase1/WBS.md`): source → **lex/parse (W2)** → **typecheck + IR
//! (W3)** → **codegen (W4)**. Per D19 codegen lowers the non-Rust IR to `extern "C"`
//! Rust, then `cargo build --crate-type cdylib`.

pub mod abi;
pub mod ast;
pub mod codegen;
pub mod emit;
pub mod error;
pub mod header;
pub mod iface;
pub mod ir;
pub mod lexer;
pub mod parser;
pub mod reserved;
pub mod typeck;

pub use abi::ML_MODULE_ABI_VERSION;
pub use ast::*;
pub use codegen::CodegenError;
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

/// Full front→middle→back: source → typed IR → emitted `extern "C"` Rust source (W4).
pub fn compile_to_rust(src: &str) -> Result<String, CompileError> {
    let ir = compile_to_ir(src)?;
    codegen::emit(&ir).map_err(CompileError::Codegen)
}

#[derive(Debug, PartialEq, Eq)]
pub enum CompileError {
    Parse(ParseError),
    Type(TypeError),
    Codegen(CodegenError),
}

impl std::fmt::Display for CompileError {
    /// Delegate to the stage that failed. Each inner error already names itself
    /// (`parse error at <line>:<col>: …`, `type error: …`, `codegen error: …`), so this adds
    /// no prefix of its own — a caller printing `{e}` gets the real message, never a `Debug`
    /// dump of the enum.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CompileError::Parse(e) => write!(f, "{e}"),
            CompileError::Type(e) => write!(f, "{e}"),
            CompileError::Codegen(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for CompileError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            CompileError::Parse(e) => Some(e),
            CompileError::Type(e) => Some(e),
            CompileError::Codegen(e) => Some(e),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compiles_discount_to_extern_c_rust() {
        let rust = compile_to_rust(include_str!("../../examples/discount.mls")).expect("compile");
        assert!(rust.contains(r#"pub extern "C" fn mlx_discount"#));
        assert!(rust.contains("ml_module_abi_version"));
    }

    #[test]
    fn emitted_abi_version_matches_the_single_source_constant() {
        // The version lives in exactly one place; codegen interpolates it, so bumping the
        // constant can't drift from what the module actually exports.
        let rust = compile_to_rust("export fn f() -> f64 { return 1.0 }").expect("compile");
        assert!(
            rust.contains(&format!(
                "ml_module_abi_version() -> u32 {{ {ML_MODULE_ABI_VERSION} }}"
            )),
            "{rust}"
        );
    }
}

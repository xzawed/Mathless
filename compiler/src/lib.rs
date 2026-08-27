//! Mathless compiler (`mlc`) — Phase 1 skeleton.
//!
//! The real lexer/parser/typecheck/codegen land in W2–W4 (see `docs/phase1/WBS.md`).
//! Per D19, codegen will lower a **non-Rust IR** to `no_std` + `extern "C"` + `repr(C)`
//! Rust, then `cargo build --crate-type cdylib`. Not implemented yet.

/// Compile a `.mls` source string to emitted Rust module source.
///
/// Unimplemented until W4. Returns [`CompileError::NotImplemented`] for now so the
/// pipeline shape is testable before the stages exist.
pub fn compile_to_rust(_src: &str) -> Result<String, CompileError> {
    Err(CompileError::NotImplemented)
}

#[derive(Debug, PartialEq, Eq)]
pub enum CompileError {
    NotImplemented,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pipeline_is_stubbed_until_w4() {
        assert_eq!(compile_to_rust("export fn f() -> f64 { return 0 }"),
                   Err(CompileError::NotImplemented));
    }
}

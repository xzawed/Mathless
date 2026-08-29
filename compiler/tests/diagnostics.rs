//! Diagnostics (STATUS 3b-#5): a failed `mlc build` must report a source-position message,
//! not a `Debug` dump of the error enum. `ParseError`/`TypeError`/`CodegenError` already
//! carry `Display`; `CompileError` and the CLI have to stop discarding it.

use std::process::Command;

use mlc::{compile_to_ir, compile_to_rust, CompileError};

#[test]
fn compile_error_display_keeps_the_parse_position() {
    let err = compile_to_ir("export fn f(mut: f64) -> f64 { return 0.0 }").unwrap_err();
    let shown = err.to_string();
    assert!(
        shown.contains("parse error at 1:13"),
        "should keep line:col — {shown}"
    );
    assert!(
        shown.contains("keyword `mut`"),
        "should keep the message — {shown}"
    );
    assert!(
        !shown.contains("ParseError {"),
        "must not be a Debug dump — {shown}"
    );
}

#[test]
fn compile_error_display_reports_type_errors() {
    let err = compile_to_ir("export fn f(a: f64) -> f64 { return a < 1.0 }").unwrap_err();
    let shown = err.to_string();
    assert!(shown.starts_with("type error:"), "{shown}");
    assert!(!shown.contains("TypeError {"), "{shown}");
}

#[test]
fn type_errors_quote_the_surface_type_names() {
    // The user wrote `f64`, not `F64` — diagnostics must not leak Rust variant names.
    let err = compile_to_ir("export fn f(a: f64) -> f64 { return a < 1.0 }").unwrap_err();
    let shown = err.to_string();
    assert!(
        shown.contains("expected f64, found bool"),
        "surface spelling — {shown}"
    );
    assert!(!shown.contains("F64") && !shown.contains("Bool"), "{shown}");

    let err = compile_to_ir("export fn f() -> i32 { let mut x = 1 x = 1.0 return x }").unwrap_err();
    let shown = err.to_string();
    assert!(
        shown.contains("cannot assign f64 to 'x' of type i32"),
        "{shown}"
    );
}

#[test]
fn compile_error_display_reports_codegen_errors() {
    // `block_always_returns` is enforced in typeck now, so reach codegen's own error by
    // constructing the case it still guards: build the IR directly.
    use mlc::codegen;
    use mlc::ir::*;
    let ir = IrModule {
        functions: vec![IrFunction {
            name: "f".into(),
            params: vec![],
            ret: IrType::F64,
            fallible: false,
            body: vec![], // falls off the end
        }],
        errors: vec![],
    };
    let err = CompileError::Codegen(codegen::emit(&ir).unwrap_err());
    let shown = err.to_string();
    assert!(shown.starts_with("codegen error:"), "{shown}");
    assert!(!shown.contains("CodegenError {"), "{shown}");
}

#[test]
fn compile_error_is_a_std_error_with_a_source() {
    let err = compile_to_rust("export fn f() -> f64 { return }").unwrap_err();
    let dyn_err: &dyn std::error::Error = &err;
    assert!(
        dyn_err.source().is_some(),
        "the inner error should be reachable as the source"
    );
}

#[test]
fn emit_error_does_not_wrap_the_message_in_debug() {
    let out = std::env::temp_dir().join(format!("mlc_diag_{}", std::process::id()));
    let err = mlc::emit::emit_artifacts("export fn f(mut: f64) -> f64 { return 0.0 }", "f", &out)
        .unwrap_err();
    let shown = err.to_string();
    assert!(shown.contains("parse error at 1:13"), "{shown}");
    assert!(
        !shown.contains("Parse("),
        "no enum variant leakage — {shown}"
    );
}

#[test]
fn the_cli_prints_a_source_position_message_on_stderr() {
    // The whole point of 3b-#5: what a user actually sees from `mlc build`.
    let dir = std::env::temp_dir().join(format!("mlc_diag_cli_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let src = dir.join("bad.mls");
    std::fs::write(&src, "export fn f(a: f64) -> f64 { return a < 1.0 }").unwrap();

    let out = Command::new(env!("CARGO_BIN_EXE_mlc"))
        .args(["build".as_ref(), src.as_os_str()])
        .arg("-o")
        .arg(&dir)
        .output()
        .expect("run mlc");

    assert!(!out.status.success(), "the build should fail");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("type error:"),
        "stderr should carry the real message — {stderr}"
    );
    assert!(
        !stderr.contains("TypeError {") && !stderr.contains("Type("),
        "stderr must not be a Debug dump — {stderr}"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

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
            exported: true,
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

/// Run the real `mlc build` on `source` and return its stderr, asserting it failed.
fn cli_stderr_for(tag: &str, source: &str) -> String {
    let dir = std::env::temp_dir().join(format!("mlc_diag_cli_{tag}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let src = dir.join("bad.mls");
    std::fs::write(&src, source).unwrap();

    let out = Command::new(env!("CARGO_BIN_EXE_mlc"))
        .args(["build".as_ref(), src.as_os_str()])
        .arg("-o")
        .arg(&dir)
        .output()
        .expect("run mlc");
    assert!(!out.status.success(), "the build should fail");
    let stderr = String::from_utf8_lossy(&out.stderr).into_owned();

    let _ = std::fs::remove_dir_all(&dir);
    stderr
}

#[test]
fn the_cli_prints_a_type_error_message_on_stderr() {
    // The whole point of 3b-#5: what a user actually sees from `mlc build`.
    let stderr = cli_stderr_for("type", "export fn f(a: f64) -> f64 { return a < 1.0 }");
    assert!(
        stderr.contains("type error:") && stderr.contains("expected f64, found bool"),
        "stderr should carry the real message — {stderr}"
    );
    assert!(
        !stderr.contains("TypeError {") && !stderr.contains("Type("),
        "stderr must not be a Debug dump — {stderr}"
    );
}

#[test]
fn the_cli_prints_the_parse_position_on_stderr() {
    // The position must survive all the way to the binary's stderr, not just the library
    // (Grok review: the type-error fixture alone never proves line:col reaches the CLI).
    let stderr = cli_stderr_for("parse", "export fn f(mut: f64) -> f64 { return 0.0 }");
    assert!(
        stderr.contains("parse error at 1:13:"),
        "stderr should carry line:col — {stderr}"
    );
    assert!(
        stderr.contains("keyword `mut`"),
        "…and the message — {stderr}"
    );
    assert!(
        !stderr.contains("ParseError {") && !stderr.contains("Parse("),
        "stderr must not be a Debug dump — {stderr}"
    );
}

#[test]
fn dead_code_after_return_says_so_instead_of_blaming_the_return() {
    // The function DOES return on every path — the problem is the statement after it. Saying
    // "may not return on all paths" sends the reader looking for a missing `return`.
    for src in [
        "export fn f() -> i32 { return 1 let x = 2 }",
        "export fn f() -> i32 { return 1 while true { } }",
        "export fn f(b: bool) -> i32 { return 1 if b { return 2 } }",
    ] {
        let shown = compile_to_ir(src).unwrap_err().to_string();
        assert!(
            shown.contains("unreachable"),
            "should name the dead code — got: {shown}"
        );
        assert!(
            !shown.contains("may not return on all paths"),
            "and must not blame the return: {shown}"
        );
    }
}

#[test]
fn a_fallible_fn_reports_dead_code_after_fail_too() {
    let shown = compile_to_ir("error E = 1\nexport fn f() -> i32! { fail E let x = 2 }")
        .unwrap_err()
        .to_string();
    assert!(shown.contains("unreachable"), "{shown}");
}

#[test]
fn a_genuinely_missing_return_still_says_so() {
    // The other message must survive — this function really can fall off the end.
    let shown = compile_to_ir("export fn f(b: bool) -> i32 { if b { return 1 } }")
        .unwrap_err()
        .to_string();
    assert!(shown.contains("may not return on all paths"), "{shown}");
    assert!(!shown.contains("unreachable"), "{shown}");
}

/// Every diagnostic must reach the console as ONE line.
///
/// The messages are written as multi-line Rust string literals, which only stay single-line
/// because each continued line ends in a `\`. Drop that backslash and the literal keeps the
/// newline AND the 13-21 columns of source indentation that follow it — so the user sees the
/// compiler's own formatting bleeding into their terminal. It compiles, it looks plausible in
/// a `contains(...)` assertion, and it is wrong: the same shape this repo has now met six times.
///
/// Two messages shipped that way in #89 (the `out string` and `-> string` rejections) because
/// every test that touched them only asserted `contains("string")`. This asserts the property
/// itself, over one source per diagnostic family, so the whole class cannot regress.
#[test]
fn no_diagnostic_leaks_a_newline_or_source_indentation() {
    // One invalid source per diagnostic family. Add a row whenever a message is added.
    const BAD: &[&str] = &[
        // parse
        "export fn f(mut: f64) -> f64 { return 0.0 }",
        "export fn f(s: string) -> bool { return s == \"oops }",
        "export fn f(s: string) -> bool { return s == \"a\nb\" }",
        "export fn f(s: string) -> bool { return s == \"한글\" }",
        // types and operators
        "export fn f(a: f64) -> f64 { return a < 1.0 }",
        "export fn f(a: f64, b: i32) -> f64 { return a + b }",
        "export fn f(a: f64, b: f64) -> f64 { return a % b }",
        "export fn f(a: i32) -> i32 { return a / 0 }",
        "export fn f(a: string, b: string) -> bool { return a < b }",
        "export fn f(a: string, b: i32) -> bool { return a == b }",
        // string scope — the three rejections, including the two that shipped mangled
        "export fn f(s: string) -> string { return s }",
        "export fn f(s: string) -> bool { let t = s return t == s }",
        "export fn f(s: string, out t: string) -> bool { return true }",
        // out params
        "fn f(a: f64, out t: i32) -> f64 { return a }",
        "export fn f(a: f64, out t: i32) -> f64 { return t as f64 }",
        "export fn f(a: f64, out t: i32) -> f64 { return a }",
        // names and scopes
        "export fn f(__d: i32) -> i32 { return __d }",
        "export fn f(mlx_x: i32) -> i32 { return mlx_x }",
        "export fn f(a: i32) -> i32 { return b }",
        "export fn f(a: i32) -> i32 { a = 1 return a }",
        // functions, calls, recursion, fallibility
        // An internal `-> T!` is legal since SPEC-fallible-calls. These are the five rules
        // that replaced that one rejection, so the family stays covered as it grew.
        "error E = 1\nfn g(x: i32) -> i32! { fail E }\nexport fn f(x: i32) -> i32 { let y = try g(x) return y }",
        "fn g(x: i32) -> i32 { return x }\nexport fn f(x: i32) -> i32! { let y = try g(x) return y }",
        "error E = 1\nexport fn g(x: i32) -> i32! { fail E }\nexport fn f(x: i32) -> i32! { let y = try g(x) return y }",
        "error E = 1\nfn g(x: i32) -> i32! { fail E }\nexport fn f(x: i32) -> i32! { let y = g(x) return y }",
        "error E = 1\nfn g(x: i32) -> i32! { fail E }\nexport fn f(x: i32) -> i32! { let y = try g(x, 1) return y }",
        "fn g() -> i32 { return g() }\nexport fn f() -> i32 { return 0 }",
        "export fn floor(x: f64) -> f64 { return x }",
        "export fn f() -> i32 { return 1 }\nexport fn F() -> i32 { return 2 }",
        // control flow
        "export fn f(b: bool) -> i32 { if b { return 1 } }",
        "export fn f() -> i32 { return 1 let x = 2 }",
    ];

    for src in BAD {
        let shown = compile_to_ir(src)
            .map(|_| String::new())
            .unwrap_or_else(|e| e.to_string());
        assert!(!shown.is_empty(), "expected a diagnostic for: {src}");
        assert!(
            !shown.contains('\n'),
            "diagnostic spans lines (a missing line-continuation in the format literal)\n\
             source: {src}\nmessage: {shown:?}"
        );
        // Leaked source indentation shows up as a run of spaces; real prose never has one.
        assert!(
            !shown.contains("   "),
            "diagnostic carries the compiler's own indentation\n\
             source: {src}\nmessage: {shown:?}"
        );
    }
}

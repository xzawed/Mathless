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

/// The compiler holds two views of "this statement ends the block", and they disagreed.
///
/// `ir::block_always_returns` counts `TryCall { dest: IrTryDest::Return, .. }` as a terminator
/// "exactly as a plain `return` does" — which is why `return try g(n)` satisfies the
/// all-paths-return check. The dead-code scan in `check_block` matched only `Return` and
/// `Fail`, so it did not.
///
/// Measured: this compiled, exit 0, and wrote a `.dll`,
///
/// ```text
/// export fn g(n: i32) -> i32! {
///   return try half(n)
///   return 999
/// }
/// ```
///
/// while the same shape with a plain `return` was rejected. The dead statement is dead either
/// way; only the report went missing.
#[test]
fn dead_code_after_a_try_return_is_reported_like_any_other() {
    let shown = compile_to_ir(
        "error E = 1\n\
         fn half(n: i32) -> i32! { if n < 0 { fail E }  return n / 2 }\n\
         export fn g(n: i32) -> i32! { return try half(n)  return 999 }\n",
    )
    .map(|_| String::from("<it compiled>"))
    .unwrap_or_else(|e| e.to_string());
    assert!(
        shown.contains("unreachable"),
        "a `return try …` ends the block, so what follows is dead: {shown}"
    );
    assert!(
        !shown.contains("may not return on all paths"),
        "and it must not be reported as a missing return: {shown}"
    );

    // The other three `try` destinations do NOT end the block — they bind or assign and then
    // fall through — so a statement after one of them is ordinary live code.
    compile_to_ir(
        "error E = 1\n\
         fn half(n: i32) -> i32! { if n < 0 { fail E }  return n / 2 }\n\
         export fn g(n: i32) -> i32! { let h = try half(n)  return h + 1 }\n",
    )
    .expect("a try-let is not a terminator");
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
        // (Calling an EXPORTED fallible callee is legal since the wrapper refactor; these two
        // are the rules that still reject a try call.)
        "error E = 1
fn g(x: i32, out t: i32) -> i32! { t = 1 fail E }
export fn f(x: i32) -> i32! { let y = try g(x) return y }",
        "fn g(s: string) -> string! { return s }
export fn f(s: string) -> string! { return try g(s) }",
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

/// The gaps a user actually hits, and whether the message names the gap.
///
/// Measured 2026-09-05 by writing twelve business rules in today's surface: nine compiled,
/// and all three failures were reported as a stray CHARACTER rather than a missing feature.
///
///   xs: f64[]            -> "unexpected character '['"
///   c.tier               -> "unexpected character '.'"
///   struct P { x: i32 }  -> "expected 'fn', found Ident(\"struct\")"
///
/// None of those contains the words "array", "field" or "struct declaration", so a user is
/// told a character is wrong when a FEATURE is missing. The lexer already does better for
/// `&` and `|` ("did you mean `&&`?"); this extends the same idea to the gaps.
///
/// **A floor, not a freeze.** These assert that the message NAMES the thing — not its exact
/// wording — because `language_gaps.rs` deliberately prints diagnostics instead of pinning
/// them, so that an improvement never reads as a failure.
#[test]
fn a_missing_feature_is_reported_as_a_missing_feature() {
    for (src, want, what) in [
        (
            "export fn total(lines: f64[]) -> f64 { return 0.0 }",
            "array",
            "an array type",
        ),
        (
            "export fn f(xs: f64) -> f64 { return xs[0] }",
            "array",
            "an index expression",
        ),
        (
            "export fn f(a: f64) -> f64 { return a.x }",
            "field",
            "a field access",
        ),
        (
            "struct P { x: i32 }\nexport fn f(a: i32) -> i32 { return a }",
            "struct",
            "a struct declaration",
        ),
        (
            "export fn f(a: f64?) -> f64 { return 0.0 }",
            "option",
            "an option type",
        ),
    ] {
        let shown = compile_to_ir(src)
            .map(|_| String::from("<it compiled>"))
            .unwrap_or_else(|e| e.to_string());
        assert!(
            shown.to_lowercase().contains(want),
            "the diagnostic for {what} never says {want:?} — a user is told a character is \
             wrong when a feature is missing.\n  source:  {src}\n  message: {shown}"
        );
    }
}

/// The two ways the change above nearly gave a confident WRONG answer. Both were found by
/// Grok verify and are pinned here because both would regress silently.
#[test]
fn naming_the_gap_does_not_name_the_wrong_gap() {
    // `eat()` is shared. Before scoping, an `Ident("struct")` anywhere a token was expected
    // claimed "the top level takes `export fn`…", which is nonsense inside a parameter list.
    let shown = compile_to_ir("export fn f(a: f64 struct) -> f64 { return a }")
        .map(|_| String::new())
        .unwrap_or_else(|e| e.to_string());
    assert!(
        shown.contains("expected ')'"),
        "a struct-shaped token mid-declaration must still report the token that was \
         expected, not the top-level rule: {shown}"
    );

    // A range reaches the lexer two different ways: `0..3` leaves a dot whose PREVIOUS
    // character is a dot (the number path ate `0.`), `a..b` leaves one whose NEXT is.
    //
    // These used to be `for i in 0..3` and `while a..b`. Both now report the STATEMENT
    // instead — `for` loops are named, and the earlier-error rule prefers that over the
    // range the lexer choked on later. That is a better answer, and this test asserting the
    // old one would have made the improvement look like a regression: exactly what
    // language_gaps.rs warns about, walked into by the test written to avoid it. The sources
    // moved to a position where the range genuinely IS the first thing wrong.
    for src in [
        "export fn f(a: f64) -> f64 { let x = 0..3  return a }",
        "export fn f(a: i32, b: i32) -> i32 { let x = a..b  return a }",
    ] {
        let shown = compile_to_ir(src)
            .map(|_| String::new())
            .unwrap_or_else(|e| e.to_string());
        assert!(
            shown.contains("range"),
            "a range must be reported as a range, not as field access: {src}\n  {shown}"
        );
    }

    // And none of these words became reserved: #101 made function names unrestricted.
    compile_to_ir(
        "fn struct_ok(a: i32) -> i32 { return a }\n\
         export fn f(a: i32) -> i32 { return struct_ok(a) }",
    )
    .expect("naming a function after a gap keyword must still compile");
}

/// The ordering §9-2 measured and #144 deliberately left: lexing runs to completion before the
/// parser sees a token, so a bad CHARACTER late in the file hides a real error early in it.
///
///   struct P { x: i32 }          <- the actual mistake, line 1
///   export fn f(p: P) -> i32 { return p.x }   <- reported instead, because `.` fails lexing
///
/// Both messages are true after #144, so the user was told *something* real either way — but
/// not the first thing wrong with their program, which is the one they can act on.
#[test]
fn the_earlier_error_wins_even_when_the_later_one_is_a_lex_error() {
    let shown = compile_to_ir("struct P { x: i32 }\nexport fn f(p: P) -> i32 { return p.x }")
        .map(|_| String::new())
        .unwrap_or_else(|e| e.to_string());
    assert!(
        shown.contains("1:1"),
        "the struct on line 1 is the first thing wrong; the `.` on line 2 only fails later: \
         {shown}"
    );
    assert!(
        shown.contains("struct"),
        "and it should still name the gap, not the token: {shown}"
    );
}

/// Statement position gets the same treatment as the top level (#144 did declarations only).
#[test]
fn control_flow_that_does_not_exist_is_named_in_statement_position() {
    for (src, want) in [
        (
            "export fn f(a: f64) -> f64 { for i in 0..3 { } return a }",
            "for",
        ),
        (
            "export fn f(a: f64, b: f64) -> f64 { if a > b { return a } else { return b } }",
            "else",
        ),
        (
            "export fn f(a: f64, b: f64) -> f64 { while a > b { break } return a }",
            "break",
        ),
        (
            "export fn f(a: f64, b: f64) -> f64 { while a > b { continue } return a }",
            "continue",
        ),
    ] {
        let shown = compile_to_ir(src)
            .map(|_| String::new())
            .unwrap_or_else(|e| e.to_string());
        assert!(
            shown.contains(want) && shown.contains("not in Mathless yet"),
            "`{want}` should be reported as a missing feature: {shown}"
        );
    }
}

/// The other half of the earlier-error rule: truncation must not INVENT an earlier error.
///
/// Parsing a prefix could in principle fail before the point where lexing stopped, and then
/// a truncation artefact would hide the real reason (Grok verify raised it). It cannot here —
/// this parser is single pass, so a failure at token k is decided by tokens up to k, which are
/// identical in the prefix and in the whole file — but that is an argument, and the repository's
/// standing rule is to measure instead.
#[test]
fn a_late_bad_character_is_still_reported_when_nothing_earlier_is_wrong() {
    let shown = compile_to_ir(
        "export fn f(a: f64) -> f64 { return a }\n\
         export fn g(a: f64) -> f64 { return a @ }",
    )
    .map(|_| String::new())
    .unwrap_or_else(|e| e.to_string());
    assert!(
        shown.contains("unexpected character '@'"),
        "everything before the `@` is valid, so the lex error must survive: {shown}"
    );
    assert!(
        shown.contains("2:"),
        "and keep its own position rather than the truncation's: {shown}"
    );
}

/// Identifiers must be ASCII, because they are emitted RAW into every backend.
///
/// The lexer already rejects a non-ASCII byte inside a string literal, and says why:
/// "generated artifacts stay ASCII (non-ASCII trips MSVC C4819 under `/WX`)". `emit.rs`
/// rejects a non-ASCII module name for the same reason. Identifiers were the hole in the
/// middle — and they are the ones that reach the generated header verbatim.
///
/// Measured before this check existed, on the real compiler:
///   export fn café(x: f64) -> f64 { return x }          -> accepted; header not ASCII
///   export fn f(가격: f64) -> f64 { return 가격 }        -> accepted; header not ASCII
///   export fn f(gebühr: f64) -> f64 { return gebühr }   -> accepted; header not ASCII
///
/// Downstream that produced either a rustc error inside generated code the user never wrote
/// (`#[no_mangle] requires ASCII identifier`) or a `.h` whose meaning depends on the reader's
/// code page — MSVC read one of them as a valid identifier under CP949 and rejected it under
/// CP1252. A header that compiles or not depending on the locale is the worst of the three.
#[test]
fn a_non_ascii_identifier_is_rejected_in_the_frontend() {
    for (what, src) in [
        ("an exported function name", "export fn caf\u{e9}(x: f64) -> f64 { return x }"),
        (
            "a parameter name",
            "export fn f(geb\u{fc}hr: f64) -> f64 { return geb\u{fc}hr }",
        ),
        (
            "a local name",
            "export fn f(x: f64) -> f64 { let \u{ac00}\u{aca9} = x  return \u{ac00}\u{aca9} }",
        ),
        (
            "an internal function name",
            "fn \u{e9}t(x: f64) -> f64 { return x }\nexport fn f(x: f64) -> f64 { return \u{e9}t(x) }",
        ),
    ] {
        let shown = compile_to_ir(src)
            .map(|_| String::from("<it compiled>"))
            .unwrap_or_else(|e| e.to_string());
        assert!(
            shown.to_lowercase().contains("ascii"),
            "{what} must be rejected with a reason mentioning ASCII, because it is emitted raw \
             into the .h and .pas.\n  source:  {src}\n  message: {shown}"
        );
    }

    // And the ordinary case still works — this must not become "identifiers must be letters".
    compile_to_ir("export fn f(_x: f64) -> f64 { let y2 = _x  return y2 }")
        .expect("plain ASCII identifiers, including `_` and digits, must still compile");
}

/// Four edge cases the code audit measured, all of which the frontend accepted or misreported.
///
/// The first is the one that matters: a NUL inside a string literal is ASCII, so the
/// non-ASCII guard misses it, and codegen lowers the literal to `b"…\0"` — a C string that
/// ENDS at the embedded NUL. Measured: `s == "a\0b"` emitted `b"a<NUL>b\0"`, so the module
/// compares one byte where the user wrote three. A silent wrong answer at the ABI.
#[test]
fn a_control_character_in_a_string_literal_is_rejected() {
    for (what, src) in [
        (
            "NUL, which truncates the C string",
            "export fn f(s: string) -> bool { return s == \"a\u{0}b\" }",
        ),
        (
            "BEL",
            "export fn f(s: string) -> bool { return s == \"a\u{7}b\" }",
        ),
    ] {
        let shown = compile_to_ir(src)
            .map(|_| String::from("<it compiled>"))
            .unwrap_or_else(|e| e.to_string());
        assert!(
            shown.to_lowercase().contains("control"),
            "a control character ({what}) must be rejected in a string literal: {shown}"
        );
    }
    // Ordinary literals keep working — this must not become "letters and digits only".
    compile_to_ir("export fn f(s: string) -> bool { return s == \"KR-01 (x)\" }")
        .expect("printable ASCII must still be accepted in a literal");
}

/// Error names are emitted as constants into the Delphi unit, where Pascal is
/// case-INSENSITIVE — so two names differing only in case are one identifier declared twice.
///
/// Measured before this check: `error E_Neg = 1` + `error E_NEG = 2` compiled and emitted
///   ML_M_ERR_E_Neg = 1;
///   ML_M_ERR_E_NEG = 2;
/// into the same `.pas`. Function names (typeck) and parameter names both already dedup
/// case-insensitively, with the comment "(Delphi binding)"; error names land in the same unit
/// and were the one kind that did not.
#[test]
fn error_names_are_unique_case_insensitively_like_every_other_emitted_name() {
    let shown = compile_to_ir(
        "error E_Neg = 1\nerror E_NEG = 2\n\
         export fn f(x: f64) -> f64! { if x < 0.0 { fail E_Neg }  return x }\n",
    )
    .map(|_| String::from("<it compiled>"))
    .unwrap_or_else(|e| e.to_string());
    assert!(
        shown.contains("case-insensitively"),
        "two error names differing only in case become one Pascal identifier declared twice: \
         {shown}"
    );
}

/// A byte-order mark and a non-breaking space are invisible, so quoting them helps nobody.
#[test]
fn invisible_characters_are_handled_or_named() {
    // A BOM is what a Windows editor writes by default. It is not an error in the source.
    compile_to_ir("\u{feff}export fn f(x: f64) -> f64 { return x }")
        .expect("a leading UTF-8 BOM must be skipped, not reported as a stray character");

    // A non-breaking space IS an error, but the message has to say WHICH character, because
    // the user's editor shows a space and the old message quoted one too.
    let shown = compile_to_ir("export fn f(x: f64)\u{a0}-> f64 { return x }")
        .map(|_| String::new())
        .unwrap_or_else(|e| e.to_string());
    assert!(
        shown.contains("U+00A0"),
        "an invisible character must be named by codepoint, not quoted: {shown}"
    );
}

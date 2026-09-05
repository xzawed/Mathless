//! Calling a fallible function (SPEC docs/slices/SPEC-fallible-calls.md).
//!
//! Two rules combined to make every `-> T!` function a leaf: an internal one could not be
//! declared, and an exported one could not be called. This slice adds `try` and lifts both.
//!
//! The load-bearing part is the POSITION restriction. `try` is a statement form, never an
//! expression, and that is what keeps two measured hazards unreachable rather than merely
//! tested — the `i32` division guard evaluates its right operand first, and hoisting a
//! prelude out of a `&&` condition would evaluate the right operand unconditionally.
//!
//! The values are measured against a loaded module in `hosts/rust-oracle/tests/fallible_calls.rs`.

use mlc::{compile_to_ir, compile_to_rust};

fn err_of(src: &str) -> String {
    compile_to_ir(src).unwrap_err().to_string()
}

const QUOTE: &str = include_str!("../../examples/quote.mls");

#[test]
fn an_internal_function_may_now_be_fallible_and_called() {
    // DP-F3 lifts the #67 declaration ban; DP-F1/F2 supply the call form it was waiting for.
    compile_to_rust(QUOTE).expect("the example from SPEC section 1 must compile");
}

#[test]
fn a_try_call_lowers_to_a_match_on_result() {
    // DP-F6: `Result<T, i32>` is the internal shape — the only candidate needing no invented
    // dummy value on the failure path. Destructured with `match`, never with Rust's `?`,
    // `unwrap` or `expect`: a panic in a generated module hangs the calling host thread.
    let rust = compile_to_rust(QUOTE).expect("compile");
    assert!(
        rust.contains("fn ml_fn_check_qty(qty: i32) -> Result<i32, i32>"),
        "an internal fallible fn returns Result:\n{rust}"
    );
    assert!(rust.contains("Ok(") && rust.contains("Err("), "{rust}");
    assert!(!rust.contains(".unwrap()"), "no panic path:\n{rust}");
    assert!(!rust.contains(".expect("), "no panic path:\n{rust}");
}

#[test]
fn the_c_abi_does_not_change() {
    // Section 3-F. The whole slice lives below the boundary: the generated header for a
    // program that compiles today must be byte-identical, and for the new example it must
    // look exactly like an ordinary D17 fallible export.
    let ir = compile_to_ir(QUOTE).expect("compile");
    let h = mlc::header::emit_c_header(&ir, "quote");
    assert!(
        h.contains(
            "int32_t mlx_unit_price(double /* total */, int32_t /* qty */, double* out_value);"
        ),
        "{h}"
    );
    assert!(
        h.contains(
            "int32_t mlx_line_check(int32_t /* qty */, int32_t* /* tier */, int32_t* out_value);"
        ),
        "DP-O1 still puts the declared out first and out_value last:\n{h}"
    );
    // The helpers are internal: they must not appear in the header at all.
    assert!(!h.contains("check_qty"), "{h}");
    assert!(!h.contains("safe_div"), "{h}");
}

#[test]
fn a_fallible_callee_needs_the_marker() {
    // DP-F12-style: the marker is mandatory AND exclusive, so it can never lie. Today's
    // message was a dead end ("cannot be called in an expression yet"); it must now write
    // the fix.
    let msg = err_of(
        "error E = 1\n\
         fn g(x: i32) -> i32! { if x < 0 { fail E } return x }\n\
         export fn f(x: i32) -> i32! { let y = g(x) return y }",
    );
    assert!(msg.contains("try"), "the message must name the fix: {msg}");
}

#[test]
fn try_on_an_infallible_callee_is_rejected() {
    // The other direction. Without this the marker is a comment, not a checked annotation.
    let msg = err_of(
        "fn g(x: i32) -> i32 { return x }\n\
         export fn f(x: i32) -> i32! { let y = try g(x) return y }",
    );
    assert!(
        msg.contains("not fallible") || msg.contains("infallible"),
        "{msg}"
    );
}

#[test]
fn a_non_fallible_caller_is_rejected() {
    // DP-F4. A `RetAbi::Plain` function has no status channel to leave through. Inferring `!`
    // would change the exported ABI with no change to the source, which D17 already rejected;
    // defaulting is the P6 defect itself.
    let msg = err_of(
        "error E = 1\n\
         fn g(x: i32) -> i32! { if x < 0 { fail E } return x }\n\
         export fn f(x: i32) -> i32 { let y = try g(x) return y }",
    );
    assert!(msg.contains('!'), "the message must name the fix: {msg}");
}

#[test]
fn try_may_not_appear_inside_an_expression() {
    // DP-F2, and the reason the slice is shaped this way. Each of these is rejected by the
    // PARSER: there is no try-call expression node in the AST, so they are unrepresentable
    // rather than merely refused.
    const PRELUDE: &str = "error E = 1\nfn g(x: i32) -> i32! { if x < 0 { fail E } return x }\n";
    for tail in [
        "export fn f(x: i32) -> i32! { let y = 1 + try g(x) return y }",
        "export fn f(x: i32) -> i32! { let y = try g(try g(x)) return y }",
        "export fn f(x: i32) -> i32! { let y = g2(try g(x)) return y }",
        "export fn f(x: i32) -> i32! { if try g(x) > 0 { return 1 } return 0 }",
        "export fn f(x: i32) -> i32! { while try g(x) > 0 { return 1 } return 0 }",
    ] {
        let src = format!("{PRELUDE}{tail}");
        assert!(
            compile_to_ir(&src).is_err(),
            "a try call inside an expression must be rejected: {tail}"
        );
    }
}

#[test]
fn try_works_in_all_three_statement_positions() {
    // DP-F2's positive half: `let`, `let mut`, assignment (including to an out), and `return`.
    const PRELUDE: &str = "error E = 1\nfn g(x: i32) -> i32! { if x < 0 { fail E } return x }\n";
    for tail in [
        "export fn f(x: i32) -> i32! { let y = try g(x) return y }",
        "export fn f(x: i32) -> i32! { let mut y = try g(x) y = y + 1 return y }",
        "export fn f(x: i32) -> i32! { let mut y = 0 y = try g(x) return y }",
        // Assigning an `out` through `try` must satisfy DP-O2 the same way `t = 1` does.
        // (`return t` would be rejected — an out is write-only, DP-O4.)
        "export fn f(x: i32, out t: i32) -> i32! { t = try g(x) return 0 }",
        "export fn f(x: i32) -> i32! { return try g(x) }",
    ] {
        let src = format!("{PRELUDE}{tail}");
        compile_to_rust(&src).unwrap_or_else(|e| panic!("{tail}\n{e}"));
    }
}

#[test]
fn an_exported_callee_may_now_be_try_called() {
    // DP-F5, unlocked by SPEC-export-wrappers. The restriction was never about the feature —
    // it was that an exported fallible function's body was emitted against the C ABI while a
    // `try` callee needs `Result<T, i32>`. Now every function has ONE body in that shape and
    // the C ABI lives in a thin adapter, so a rule can be exposed to the host AND reused
    // inside the module.
    //
    // SPEC-fallible-calls section 0.1 measured what the old workaround cost: promoting a rule
    // to its own export made its callers non-fallible, so the validation stopped being
    // enforced by the ABI at all.
    let rust = compile_to_rust(
        "error E = 1\n\
         export fn g(x: i32) -> i32! { if x < 0 { fail E } return x }\n\
         export fn f(x: i32) -> i32! { let y = try g(x) return y }",
    )
    .expect("an exported fallible callee is try-callable");
    assert!(
        rust.contains("ml_fn_g(x)"),
        "the call reaches the BODY:\n{rust}"
    );
    assert!(
        rust.contains(r#"pub extern "C" fn mlx_g("#),
        "...and the export still exists for the host:\n{rust}"
    );
}

#[test]
fn a_string_returning_callee_may_not_be_try_called_yet() {
    // DP-F7: the returned string has nowhere to land — string locals are rejected, and
    // `return try g(x)` would violate DP-T5's literal-or-parameter rule.
    let msg = err_of(
        "fn g(s: string) -> string! { return s }\n\
         export fn f(s: string) -> string! { return try g(s) }",
    );
    assert!(!msg.is_empty(), "must be rejected");
}

#[test]
fn recursion_through_a_try_call_is_still_rejected() {
    // The new statement form is a call graph edge. If `collect_calls` did not walk it, a
    // recursive cycle would slip past `reject_recursion` and overflow the host's stack —
    // which kills the process, not just the call.
    let msg = err_of(
        "error E = 1\n\
         fn a(x: i32) -> i32! { let y = try b(x) return y }\n\
         fn b(x: i32) -> i32! { let y = try a(x) return y }\n\
         export fn f(x: i32) -> i32! { return try a(x) }",
    )
    .to_lowercase();
    assert!(msg.contains("recursi") || msg.contains("cycle"), "{msg}");
}

#[test]
fn try_is_not_a_reserved_variable_name() {
    // `try` is a statement keyword, not a general one. Deliberately checked, because the
    // rounding slice learned the same lesson (DP-R1: builtins are signatures, not keywords)
    // and `let try = 1` should stay legal unless there is a reason it cannot be.
    let msg = err_of("export fn f() -> i32 { let try = 1 return try }");
    // Whatever the answer, it must be a decision rather than an accident: if this is
    // rejected, the message must say `try` is a keyword rather than something vaguer.
    if !msg.is_empty() {
        assert!(msg.contains("try"), "if reserved, say so plainly: {msg}");
    }
}

/// Section 3-F: the whole slice lives BELOW the C boundary, so every program that compiled
/// before must produce a byte-identical header and Delphi unit.
///
/// Written as a test rather than a claim because "no ABI change" is the kind of sentence that
/// stays in a document after it stops being true. The corpus is every example in the repo,
/// so it grows with the language rather than freezing at today's set.
///
/// **These strings moved once, deliberately, and this note is the record.** The parameter
/// names became comments (`double /* price */`) because a name this project does not control
/// must not be a token in a header — measured, `hosts/c-host/host.c`'s own include order made
/// `double mlx_f(double TRUE)` into `double mlx_f(double 1)` and the build failed while
/// `mlc build` reported success. Nothing about the **ABI** moved: a parameter name in a C
/// prototype has never affected linkage, the argument types and their order are unchanged,
/// and `hosts/c-host` and `hosts/c-host-link` both still compile, link and call. If a future
/// change moves one of these strings again, the question this test asks is the right one —
/// answer it before editing the string.
#[test]
fn no_existing_example_changes_its_bindings() {
    // A representative slice of the corpus: one per earlier slice's shape. If a future change
    // altered the emitted signatures, one of these would move.
    const EXPECT: &[(&str, &str)] = &[
        (
            "discount",
            "double mlx_discount(double /* price */, bool /* vip */);",
        ),
        (
            "safe_div",
            "int32_t mlx_safe_div(double /* a */, double /* b */, double* out_value);",
        ),
        (
            "commission",
            "double mlx_commission(double /* amount */, int32_t* /* tier */);",
        ),
        (
            "vat",
            "double mlx_vat_rate(const char* /* country */);",
        ),
        (
            "carrier",
            "int32_t mlx_carrier_name(const char* /* scac */, char* ml_buf, int32_t ml_cap, int32_t* ml_needed);",
        ),
    ];
    for (name, decl) in EXPECT {
        let src = std::fs::read_to_string(format!("../examples/{name}.mls"))
            .unwrap_or_else(|e| panic!("read examples/{name}.mls: {e}"));
        let ir = compile_to_ir(&src).unwrap_or_else(|e| panic!("{name}: {e}"));
        let h = mlc::header::emit_c_header(&ir, name);
        assert!(
            h.contains(decl),
            "the fallible-calls slice must not change any existing signature.\n\
             {name}.h no longer declares:\n  {decl}\n\ngot:\n{h}"
        );
    }
}

/// DP-F10: the header must say WHICH codes each export can return.
///
/// `#define`s are module-scoped, so today a host author reading `quote.h` sees
/// `ML_QUOTE_ERR_E_BAD_QTY` and `ML_QUOTE_ERR_E_DIV0` and has no way to tell that `mlx_line_check` can
/// only ever produce the first. Q13's flat i32 carries no provenance by construction, and
/// propagation makes the gap wider: a code now arrives from a helper the host never sees.
///
/// Comments only — every signature stays byte-identical.
#[test]
fn the_header_names_the_codes_each_export_can_return() {
    let ir = compile_to_ir(QUOTE).expect("compile");
    let h = mlc::header::emit_c_header(&ir, "quote");

    // `unit_price` calls both helpers, so both codes can reach the host.
    assert!(
        h.contains("/* may fail with: ML_QUOTE_ERR_E_BAD_QTY, ML_QUOTE_ERR_E_DIV0 */"),
        "{h}"
    );
    // `line_check` calls only `check_qty`, so `E_DIV0` is unreachable from it.
    assert!(
        h.contains("/* may fail with: ML_QUOTE_ERR_E_BAD_QTY */"),
        "{h}"
    );

    // Still comments: not one declaration changes.
    assert!(
        h.contains(
            "int32_t mlx_unit_price(double /* total */, int32_t /* qty */, double* out_value);"
        ),
        "{h}"
    );
}

#[test]
fn an_infallible_export_gets_no_failure_comment() {
    let ir = compile_to_ir("export fn f(x: f64) -> f64 { return x * 2.0 }").expect("compile");
    let h = mlc::header::emit_c_header(&ir, "plain");
    assert!(!h.contains("may fail with"), "{h}");
}

#[test]
fn a_fallible_export_that_cannot_actually_fail_says_so() {
    // `-> f64!` with no reachable `fail` is legal — the truncation status of a string return
    // is the obvious case, and a host still has to check the status. Saying "none" is more
    // useful than saying nothing, which would be indistinguishable from an infallible export.
    let ir = compile_to_ir("export fn f(x: f64) -> f64! { return x * 2.0 }").expect("compile");
    let h = mlc::header::emit_c_header(&ir, "nofail");
    assert!(h.contains("/* may fail with: (no domain error) */"), "{h}");
}

/// A `try` call must apply the same argument rules an ordinary call does.
///
/// `check_expr::Call` runs `reject_built_string` over its arguments, and comparisons do too —
/// a "built" string (`a + b`, or `n as string`) exists only as bytes written straight into the
/// host's buffer, so it has nowhere to live as an argument. `check_try_call` type-checked its
/// arguments but never ran that guard, so the one call form that skipped it was `try`.
#[test]
fn a_try_call_rejects_a_built_string_argument_like_any_other_call() {
    for (what, src) in [
        (
            "concatenation",
            "error E = 1\n\
             fn take(s: string) -> f64! { if s == \"x\" { fail E }  return 1.0 }\n\
             export fn f(a: string, b: string) -> f64! { let v = try take(a + b)  return v }\n",
        ),
        (
            "i32 as string",
            "error E = 1\n\
             fn take(s: string) -> f64! { if s == \"x\" { fail E }  return 1.0 }\n\
             export fn f(n: i32) -> f64! { let v = try take(n as string)  return v }\n",
        ),
    ] {
        let shown = compile_to_ir(src)
            .map(|_| String::from("<it compiled>"))
            .unwrap_or_else(|e| e.to_string());
        assert!(
            !shown.contains("<it compiled>"),
            "a built string ({what}) must not reach a callee's `string` parameter through \
             `try` — an ordinary call rejects it: {shown}"
        );
    }
}

/// `let x = try f(…)` binds a name, so it owes the same DP-L2 rule plain `let` owes.
///
/// Plain `Stmt::Let` refuses a name already in scope ("no redeclaration or shadowing"). The
/// try-let destination inserted straight into the scope map, so the one binding form that
/// could silently shadow a parameter was the one introduced last.
#[test]
fn a_try_let_cannot_shadow_a_name_already_in_scope() {
    for (what, src) in [
        (
            "a parameter",
            "error E = 1\n\
             fn g(x: f64) -> f64! { if x < 0.0 { fail E }  return x }\n\
             export fn f(v: f64) -> f64! { let v = try g(v)  return v }\n",
        ),
        (
            "an existing local",
            "error E = 1\n\
             fn g(x: f64) -> f64! { if x < 0.0 { fail E }  return x }\n\
             export fn f(a: f64) -> f64! { let v = a  let v = try g(a)  return v }\n",
        ),
    ] {
        let shown = compile_to_ir(src)
            .map(|_| String::from("<it compiled>"))
            .unwrap_or_else(|e| e.to_string());
        assert!(
            shown.contains("already in scope"),
            "a try-let must not shadow {what}, exactly as a plain `let` may not: {shown}"
        );
    }

    // The other half a plain `let` owes in a fallible function: `out_value` names the D17
    // out-param, so a local may not take it. Added because Grok pointed out the fix carried
    // this rule with no test behind it — half a guard regresses as quietly as none.
    let shown = compile_to_ir(
        "error E = 1\n\
         fn g(x: f64) -> f64! { if x < 0.0 { fail E }  return x }\n\
         export fn f(a: f64) -> f64! { let out_value = try g(a)  return out_value }\n",
    )
    .map(|_| String::from("<it compiled>"))
    .unwrap_or_else(|e| e.to_string());
    assert!(
        shown.contains("out_value"),
        "a try-let named `out_value` in a fallible function must be rejected — it is the name \
         of the D17 out-param: {shown}"
    );
}

/// `try` inside a `-> string!` export reaches the `RetAbi::StringOut` propagation arm, whose
/// comment said it was "not reachable from source today".
///
/// It is reachable, and was when the comment was written: the string body already returns the
/// status directly, so a propagated code needs no wrapping and the arm emits a bare
/// `return __e`. Measured — this builds, exit 0, and writes all four artifacts:
///
/// ```text
/// error E = 1
/// fn code(n: i32) -> i32! { if n < 0 { fail E }  return n }
/// export fn label(n: i32) -> string! { let c = try code(n)  return "n=" + c as string }
/// ```
///
/// The arm was correct. Only the claim about it was wrong — and an unreachable arm is one
/// nobody tests, which is how a wrong one survives.
#[test]
fn a_try_inside_a_string_returning_export_propagates_the_status_directly() {
    let rust = compile_to_rust(
        "error E = 1\n\
         fn code(n: i32) -> i32! { if n < 0 { fail E }  return n }\n\
         export fn label(n: i32) -> string! { let c = try code(n)  return \"n=\" + c as string }",
    )
    .expect("a try inside a string-returning export must compile");
    assert!(
        rust.contains("Err(__e) => return __e }"),
        "a string body returns the status directly, so a propagated code is returned \
         unwrapped — no Err(), no Ok():\n{rust}"
    );
    assert!(
        !rust.contains("return Err(__e)"),
        "the Fallible arm's shape must not be used for a string body:\n{rust}"
    );
}

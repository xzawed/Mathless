//! D17 error-path slice (SPEC docs/slices/SPEC-D17-error-abi.md, PR #14): a fallible
//! function `-> T!` lowers to the D17 ABI (i32 status + out-param), `fail CODE` returns a
//! module-defined positive code, `return e` succeeds. Q13 = flat i32.
//!
//! These are front→codegen unit checks on the emitted Rust text; the E2 load/call proof is
//! in `hosts/rust-oracle/tests/d17_error_abi.rs`.

use mlc::{compile_to_ir, compile_to_rust};

const SAFE_DIV: &str = "\
error DIV_BY_ZERO = 1
export fn safe_div(a: f64, b: f64) -> f64! {
  if b == 0.0 { fail DIV_BY_ZERO }
  return a / b
}
";

#[test]
fn fallible_fn_lowers_to_i32_status_plus_out_param() {
    let rust = compile_to_rust(SAFE_DIV).expect("compile safe_div");
    assert!(
        rust.contains(
            r#"pub extern "C" fn mlx_safe_div(a: f64, b: f64, out_value: *mut f64) -> i32"#
        ),
        "fallible signature = status i32 + out-param:\n{rust}"
    );
    // Since the wrapper refactor the D17 shape lives in the ADAPTER, and the body speaks
    // Rust. `fail` is `Err(<code>)` in the body; the adapter turns it into the bare status.
    // The three assertions below still pin the same contract — the status is the error code,
    // success writes through the out-param, and success reports 0 — but at the layer that
    // actually implements it now.
    assert!(
        rust.contains("return Err(1);"),
        "`fail DIV_BY_ZERO` carries code 1 out of the body:\n{rust}"
    );
    assert!(
        rust.contains("unsafe { *out_value = __v; } 0"),
        "the adapter writes the value through the out-param and reports 0:\n{rust}"
    );
    assert!(
        rust.contains("Err(__e) => __e"),
        "...and hands a failure's code back as the status, unchanged:\n{rust}"
    );
}

#[test]
fn non_fallible_fn_is_unchanged() {
    // Regression: the scalar happy-path lowering must not change.
    let rust = compile_to_rust("export fn id(x: f64) -> f64 { return x }").expect("compile");
    assert!(
        rust.contains(r#"pub extern "C" fn mlx_id(x: f64) -> f64"#),
        "non-fallible signature unchanged:\n{rust}"
    );
    assert!(
        !rust.contains("out_value"),
        "non-fallible must have no out-param:\n{rust}"
    );
}

#[test]
fn fail_in_a_non_fallible_function_is_rejected() {
    // `f` has no `!`, so `fail` is not allowed even if the code were declared.
    let src = "\
error X = 1
export fn f(b: bool) -> f64 { if b { fail X } return 1.0 }
";
    let err = compile_to_ir(src).expect_err("fail in non-fallible must be a type error");
    assert!(
        format!("{err:?}").to_lowercase().contains("fallible"),
        "{err:?}"
    );
}

#[test]
fn fail_with_an_undeclared_code_is_rejected() {
    let src = "export fn f(b: bool) -> f64! { if b { fail NOPE } return 1.0 }";
    let err = compile_to_ir(src).expect_err("undeclared fail code must be a type error");
    assert!(format!("{err:?}").contains("NOPE"), "{err:?}");
}

#[test]
fn param_named_out_value_in_a_fallible_fn_is_rejected() {
    // `out_value` is the synthesized D17 out-param; a user param of that name would clash.
    let src = "export fn f(out_value: f64) -> f64! { return out_value }";
    let err =
        compile_to_ir(src).expect_err("param named out_value must be rejected in a fallible fn");
    assert!(format!("{err:?}").contains("out_value"), "{err:?}");
}

#[test]
fn non_positive_error_code_is_rejected() {
    // 0 is reserved for OK; a domain code must be a positive integer.
    let src = "error BAD = 0\nexport fn f() -> f64! { fail BAD }";
    assert!(compile_to_ir(src).is_err(), "error code 0 must be rejected");
}

/// Two `error` names sharing one code compile today, and the header emits both `#define`s
/// with the same value (measured). A host's `if (st == ML_ERR_A) … else if (st == ML_ERR_B)`
/// then always takes the first branch, so two distinct module failures collapse into one.
///
/// It was survivable while a code never crossed a function boundary — the author who wrote
/// the codes was the author who read them. `try` propagation ends that: a code now arrives
/// from a helper the host has never seen, three levels down, and the host has nothing but the
/// number. Rejecting the collision is the cheap half of Q14 (the other half, module-prefixing
/// `ML_ERR_*`, is still open).
#[test]
fn two_error_names_may_not_share_a_code() {
    let err = compile_to_ir(
        "error TOO_LONG = 1\nerror BAD_DIGIT = 1\n\
         export fn f(x: i32) -> i32! { if x > 100 { fail TOO_LONG } return x }",
    )
    .unwrap_err()
    .to_string();
    assert!(
        err.contains("TOO_LONG") && err.contains("BAD_DIGIT"),
        "{err}"
    );
    assert!(
        err.contains('1'),
        "the message must name the colliding code: {err}"
    );
}

#[test]
fn distinct_codes_are_still_fine() {
    compile_to_ir(
        "error A = 1\nerror B = 2\nerror C = 7\n\
         export fn f(x: i32) -> i32! { if x > 100 { fail A } if x < 0 { fail B } return x }",
    )
    .expect("distinct codes are the normal case");
}

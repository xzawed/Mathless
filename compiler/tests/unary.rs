//! Unary-operator slice (SPEC docs/slices/SPEC-unary.md): prefix `-` and `!`. Expression
//! level only — no ABI change. The E2 load/call proof is in
//! `hosts/rust-oracle/tests/unary.rs` and the C host.

use mlc::{compile_to_ir, compile_to_rust};

const NEGATE_IF: &str = include_str!("../../examples/negate_if.mls");

#[test]
fn a_negative_literal_can_finally_be_written() {
    // The gap this slice exists to close: `return -5` used to be a parse error.
    let rust = compile_to_rust("export fn f() -> i32 { return -5 }").expect("negative i32");
    assert!(rust.contains("-5i32") || rust.contains("(-5i32)"), "{rust}");
    compile_to_rust("export fn f() -> f64 { return -1.5 }").expect("negative f64");
}

#[test]
fn unary_not_and_neg_lower_to_rust() {
    let rust = compile_to_rust(NEGATE_IF).expect("compile negate_if");
    assert!(rust.contains("!flip"), "logical not:\n{rust}");
    assert!(rust.contains("-x"), "negation:\n{rust}");
    assert!(rust.contains("mlx_negate_if"), "{rust}");
}

#[test]
fn unary_binds_tighter_than_multiplication() {
    // `-a * b` is `(-a) * b`, not `-(a * b)`. Both evaluate the same for `*`, so assert on
    // the emitted shape: the negation must be inside the left operand.
    let rust =
        compile_to_rust("export fn f(a: i32, b: i32) -> i32 { return -a * b }").expect("compile");
    assert!(
        rust.contains("((-a) * b)"),
        "unary should bind tighter than `*`:\n{rust}"
    );
}

#[test]
fn nested_unary_is_allowed() {
    compile_to_rust("export fn f(x: i32) -> i32 { return - -x }").expect("double negation");
    compile_to_rust("export fn f(b: bool) -> bool { return !!b }").expect("double not");
}

#[test]
fn negating_a_bool_is_rejected() {
    let err = compile_to_ir("export fn f(b: bool) -> bool { return -b }").unwrap_err();
    let msg = format!("{err:?}").to_lowercase();
    assert!(msg.contains("bool"), "{err:?}");
}

#[test]
fn logical_not_on_a_number_is_rejected() {
    // Rust's `!` is bitwise-not on integers — the type rule is what keeps the lowering honest.
    for src in [
        "export fn f(x: i32) -> i32 { return !x }",
        "export fn f(x: f64) -> f64 { return !x }",
    ] {
        let err = compile_to_ir(src).unwrap_err();
        assert!(
            format!("{err:?}").to_lowercase().contains("bool"),
            "{err:?}"
        );
    }
}

#[test]
fn the_fallible_marker_does_not_clash_with_unary_not() {
    // `!` after a return type marks fallibility; `!` before an expression is the operator.
    let rust =
        compile_to_rust("error E = 1\nexport fn g(b: bool) -> i32! { if !b { fail E } return 1 }")
            .expect("compile fallible-with-not");
    assert!(rust.contains("!b"), "{rust}");
    assert!(
        rust.contains("out_value"),
        "still the fallible ABI:\n{rust}"
    );
}

#[test]
fn unary_works_in_a_while_condition() {
    let rust = compile_to_rust(
        "export fn f(done: bool) -> i32 { let mut i = 0 while !done { i = i + 1 return i } return 0 }",
    )
    .expect("compile");
    assert!(
        rust.lines()
            .any(|l| l.trim().starts_with("while ") && l.contains("!done")),
        "{rust}"
    );
}

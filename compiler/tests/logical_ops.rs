//! Logical-operator slice (SPEC docs/slices/SPEC-logical-ops.md): `&&` and `||`.
//! Expression level — no ABI change. The E2 load/call proof is in
//! `hosts/rust-oracle/tests/logical_ops.rs` and the C host.

use mlc::{compile_to_ir, compile_to_rust};

const COUNT_BOUNDED: &str = include_str!("../../examples/count_bounded.mls");

#[test]
fn two_conditions_in_a_while_header_now_compile() {
    // The gap this slice exists to close: a `while` could not combine two tests at all.
    let rust = compile_to_rust(COUNT_BOUNDED).expect("compile count_bounded");
    assert!(
        rust.lines()
            .any(|l| l.trim().starts_with("while ") && l.contains("&&")),
        "the loop header should carry the conjunction:\n{rust}"
    );
}

#[test]
fn and_and_or_lower_to_rust_operators() {
    // Rust's `&&`/`||` short-circuit, which is what the SPEC specifies (DP-B2). Nothing can
    // observe that yet — this asserts the lowering, not the behaviour.
    let rust = compile_to_rust("export fn f(a: bool, b: bool) -> bool { return a && b }")
        .expect("compile &&");
    assert!(rust.contains("&&"), "{rust}");
    let rust = compile_to_rust("export fn f(a: bool, b: bool) -> bool { return a || b }")
        .expect("compile ||");
    assert!(rust.contains("||"), "{rust}");
}

#[test]
fn and_binds_tighter_than_or() {
    // `a && b || c` is `(a && b) || c`, not `a && (b || c)`.
    let rust =
        compile_to_rust("export fn f(a: bool, b: bool, c: bool) -> bool { return a && b || c }")
            .expect("compile");
    assert!(
        rust.contains("((a && b) || c)"),
        "&& must bind tighter than ||:\n{rust}"
    );
}

#[test]
fn comparison_binds_tighter_than_and() {
    let rust = compile_to_rust("export fn f(a: i32, b: i32) -> bool { return a > 0 && b > 0 }")
        .expect("compile");
    assert!(
        rust.contains("((a > 0i32) && (b > 0i32))"),
        "comparisons group before &&:\n{rust}"
    );
}

#[test]
fn unary_not_binds_tighter_than_and() {
    let rust = compile_to_rust("export fn f(a: bool, b: bool) -> bool { return !a && b }")
        .expect("compile");
    assert!(
        rust.contains("((!a) && b)"),
        "`!a && b` is `(!a) && b`:\n{rust}"
    );
}

#[test]
fn non_bool_operands_are_rejected() {
    for src in [
        "export fn f(a: bool) -> bool { return 1 && a }",
        "export fn f(a: bool) -> bool { return a && 1 }",
        "export fn f(x: f64, y: f64) -> bool { return x || y }",
    ] {
        let err = compile_to_ir(src).unwrap_err();
        assert!(
            format!("{err:?}").to_lowercase().contains("bool"),
            "{src} -> {err:?}"
        );
    }
}

#[test]
fn the_result_is_bool_not_a_number() {
    let err = compile_to_ir("export fn f(a: bool, b: bool) -> i32 { return a && b }").unwrap_err();
    let msg = format!("{err:?}").to_lowercase();
    assert!(msg.contains("mismatch") || msg.contains("i32"), "{err:?}");
}

#[test]
fn a_single_ampersand_suggests_the_double() {
    // The most likely typo in this slice deserves better than "unexpected character".
    for (src, want) in [
        (
            "export fn f(a: bool, b: bool) -> bool { return a & b }",
            "&&",
        ),
        (
            "export fn f(a: bool, b: bool) -> bool { return a | b }",
            "||",
        ),
    ] {
        let err = compile_to_ir(src).unwrap_err();
        let shown = err.to_string();
        assert!(
            shown.contains(want),
            "the error should suggest `{want}` — got: {shown}"
        );
    }
}

//! Numeric-conversion slice (SPEC docs/slices/SPEC-numeric-conversion.md): the `as` cast.
//! The value semantics of `f64 as i32` are pinned by the SPEC as a *Mathless* rule, and
//! measured against a real module in `hosts/rust-oracle/tests/numeric_conversion.rs`.

use mlc::{compile_to_ir, compile_to_rust};

const LINE_TOTAL: &str = include_str!("../../examples/line_total.mls");

#[test]
fn a_count_can_finally_meet_a_price() {
    // The gap this slice closes: `each * qty` was a type error, so multiplication had to be
    // simulated with a loop.
    let rust = compile_to_rust(LINE_TOTAL).expect("compile line_total");
    assert!(rust.contains("as f64"), "the cast is emitted:\n{rust}");
    assert!(rust.contains("mlx_line_total"), "{rust}");
}

#[test]
fn both_numeric_directions_are_allowed() {
    compile_to_rust("export fn f(x: i32) -> f64 { return x as f64 }").expect("i32 as f64");
    compile_to_rust("export fn f(x: f64) -> i32 { return x as i32 }").expect("f64 as i32");
}

#[test]
fn an_identity_cast_is_allowed() {
    compile_to_rust("export fn f(x: i32) -> i32 { return x as i32 }").expect("i32 as i32");
}

#[test]
fn bool_is_not_convertible() {
    // Numbers are not truthy and truth is not numeric — the stance conditions already take.
    for src in [
        "export fn f(b: bool) -> i32 { return b as i32 }",
        "export fn f(x: i32) -> bool { return x as bool }",
    ] {
        let err = compile_to_ir(src).unwrap_err();
        assert!(
            format!("{err:?}").to_lowercase().contains("bool"),
            "{src} -> {err:?}"
        );
    }
}

#[test]
fn implicit_mixing_is_still_rejected() {
    // The slice adds a way to convert, it does not reopen DP-I2.
    let err = compile_to_ir("export fn f(a: f64, b: i32) -> f64 { return a * b }").unwrap_err();
    assert!(
        format!("{err:?}").to_lowercase().contains("operand"),
        "{err:?}"
    );
}

#[test]
fn a_cast_binds_looser_than_unary_minus() {
    // `-x as f64` is `(-x) as f64`: unary applies to the primary, then the cast wraps it.
    // This is Rust/C#/Kotlin binding (DP-N1, reversed 2026-08-31). The old Mathless binding
    // was the other way round and diverged silently at `i32::MIN` — see SPEC section 2.1.
    //
    // The negation is emitted as `wrapping_neg` since DP-I4 moved i32's wrap rule into the
    // code (see `compiler/tests/i32_type.rs`), so the pin is on the nesting — the cast wraps
    // the negation — rather than on the spelling of the minus.
    let rust = compile_to_rust("export fn f(x: i32) -> f64 { return -x as f64 }").expect("compile");
    assert!(
        rust.contains("((x).wrapping_neg() as f64)"),
        "expected (-x) as f64, with the cast on the OUTSIDE:\n{rust}"
    );
}

#[test]
fn a_cast_can_be_chained() {
    compile_to_rust("export fn f(x: f64) -> f64 { return x as i32 as f64 }").expect("chained");
}

#[test]
fn casting_an_expression_needs_parentheses_like_any_primary() {
    // `a + b as f64` casts only `b`; the parenthesised form casts the sum.
    let rust = compile_to_rust("export fn f(a: i32, b: i32) -> f64 { return (a + b) as f64 }")
        .expect("compile");
    assert!(rust.contains("as f64"), "{rust}");
}

#[test]
fn a_call_inside_a_cast_is_still_seen_by_the_recursion_check() {
    // The cast node had to be added to the call-graph walk; a cycle hiding behind `as` must
    // not slip through.
    let err = compile_to_ir(
        "fn f(x: i32) -> i32 { return f(x) as i32 }\nexport fn g() -> i32 { return f(1) }",
    )
    .unwrap_err();
    assert!(
        format!("{err:?}").to_lowercase().contains("recursi"),
        "{err:?}"
    );
}

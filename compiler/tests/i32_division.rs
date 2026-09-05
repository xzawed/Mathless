//! `i32` division and remainder slice (SPEC docs/slices/SPEC-i32-division.md).
//!
//! DP-D1 = (b) TOTAL operators: `x / 0 == 0`, `x % 0 == 0`, `i32::MIN / -1` wraps. The values
//! themselves are measured through a loaded module in `hosts/rust-oracle/tests/i32_division.rs`
//! — this file pins the frontend: the new `%` token, its precedence, the type rules, the
//! literal-zero rejection (DP-D5), and the guarded shape codegen must emit.

use mlc::{compile_to_ir, compile_to_rust};

#[test]
fn i32_division_and_remainder_are_accepted() {
    // The hole this slice closes: `cents / 100` had to go through f64 (SPEC section 1).
    compile_to_rust("export fn f(a: i32, b: i32) -> i32 { return a / b }").expect("i32 /");
    compile_to_rust("export fn f(a: i32, b: i32) -> i32 { return a % b }").expect("i32 %");
}

#[test]
fn remainder_binds_at_the_multiplicative_level() {
    // DP: `%` sits with `*` and `/`, left-associative (SPEC section 2.1).
    let rust = compile_to_rust("export fn f(a: i32, b: i32, c: i32) -> i32 { return a % b * c }")
        .expect("compile");
    // `(a % b) * c`, not `a % (b * c)`: the remainder is the left operand of the product.
    //
    // Asserted through the shape rather than the operator spelling — i32 `*` is now emitted as
    // `wrapping_mul` (DP-I4, see i32_type.rs), and the previous pin on the literal text `* c)`
    // broke on that without the precedence changing at all. What this test is about is which
    // subexpression is the RECEIVER of the product: the guarded remainder, closing with `}`.
    assert!(
        rust.contains("}).wrapping_mul(c)"),
        "expected the guarded remainder to be the left operand of the product:\n{rust}"
    );
    assert!(
        !rust.contains("(b).wrapping_mul(c)"),
        "`b * c` must not be the product — that would be `a % (b * c)`:\n{rust}"
    );
}

#[test]
fn division_is_emitted_with_a_zero_guard() {
    // SPEC section 2.4: a naive `/` panics on `b == 0` AND on `i32::MIN / -1`, and a panic in a
    // generated module spins in `ml_panic`'s `loop {}` (STATUS 5-4). Both edges must be closed
    // in the emitted code, not documented away.
    let rust = compile_to_rust("export fn f(a: i32, b: i32) -> i32 { return a / b }").expect("ok");
    assert!(
        rust.contains("wrapping_div"),
        "i32::MIN / -1 must not reach the plain operator:\n{rust}"
    );
    assert!(
        rust.contains("== 0"),
        "the zero divisor must be guarded:\n{rust}"
    );
    assert!(
        !rust.contains("(a / b)"),
        "the plain operator must not be emitted for i32:\n{rust}"
    );
}

#[test]
fn remainder_is_emitted_with_a_zero_guard() {
    let rust = compile_to_rust("export fn f(a: i32, b: i32) -> i32 { return a % b }").expect("ok");
    assert!(rust.contains("wrapping_rem"), "{rust}");
    assert!(rust.contains("== 0"), "{rust}");
}

#[test]
fn the_f64_division_path_is_unchanged() {
    // f64 `/` stays a plain operator — `f64 /0` is inf, not a trap, so it needs no guard.
    let rust =
        compile_to_rust("export fn f(a: f64, b: f64) -> f64 { return a / b }").expect("f64 /");
    assert!(rust.contains("(a / b)"), "{rust}");
    assert!(!rust.contains("wrapping_div"), "{rust}");
}

#[test]
fn f64_remainder_is_rejected() {
    // DP-D4: floating-point remainder is a separate semantic argument (fmod vs IEEE remainder).
    let err = compile_to_ir("export fn f(a: f64, b: f64) -> f64 { return a % b }").unwrap_err();
    let msg = format!("{err:?}").to_lowercase();
    assert!(msg.contains("%") || msg.contains("remainder"), "{err:?}");
    assert!(
        msg.contains("i32"),
        "the message should name the type that works: {err:?}"
    );
}

#[test]
fn mixing_types_across_the_new_operators_is_rejected() {
    // DP-I2 is not relaxed by this slice.
    for src in [
        "export fn f(a: i32, b: f64) -> i32 { return a % b }",
        "export fn f(a: f64, b: i32) -> f64 { return a / b }",
        "export fn f(a: i32, b: bool) -> i32 { return a / b }",
    ] {
        let err = compile_to_ir(src).unwrap_err();
        assert!(!format!("{err:?}").is_empty(), "{src} must be rejected");
    }
}

#[test]
fn a_literal_zero_divisor_is_a_compile_error() {
    // DP-D5: statically decidable and certainly a mistake, so reject it even though the
    // operator is total at runtime. This is a syntactic check, NOT constant folding.
    for src in [
        "export fn f(a: i32) -> i32 { return a / 0 }",
        "export fn f(a: i32) -> i32 { return a % 0 }",
    ] {
        let err = compile_to_ir(src).unwrap_err();
        let msg = format!("{err:?}").to_lowercase();
        assert!(
            msg.contains("zero") || msg.contains("0"),
            "{src} -> {err:?}"
        );
    }
    // Not folding: a non-literal zero still compiles, because the operator is total.
    compile_to_rust("export fn f(a: i32) -> i32 { let z = 0 return a / z }")
        .expect("a variable divisor is allowed — the operator is total");
    // And f64 `/ 0.0` stays legal: it is inf, a defined f64 value.
    compile_to_rust("export fn f(a: f64) -> f64 { return a / 0.0 }").expect("f64 /0.0 is inf");
}

#[test]
fn the_boundary_failure_pattern_still_compiles() {
    // SPEC section 4: (b) does not block (a). A caller who wants a domain error writes the
    // guard at the export, where D17's `fail` is legal.
    compile_to_rust(
        "error E_DIV0 = 1\n\
         export fn boxes(qty: i32, per_box: i32) -> i32! {\n\
           if per_box == 0 { fail E_DIV0 }\n\
           return qty / per_box\n\
         }",
    )
    .expect("the documented boundary pattern must compile");
}

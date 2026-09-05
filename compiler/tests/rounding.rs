//! `f64` rounding builtins (SPEC docs/slices/SPEC-rounding.md).
//!
//! DP-R1 = builtin SIGNATURES, not lexer keywords. DP-R3 = match C exactly.
//! The value proof is in `hosts/rust-oracle/tests/rounding.rs`, where the emitted module is
//! compared to std bit-for-bit. This file pins the frontend and the two traps in section 2.4.

use mlc::{compile_to_ir, compile_to_rust};

#[test]
fn the_four_builtins_are_callable() {
    for f in ["floor", "ceil", "round", "trunc"] {
        compile_to_rust(&format!(
            "export fn d(x: f64) -> f64 {{ return {f}(x * 0.045) }}"
        ))
        .unwrap_or_else(|e| panic!("{f} must be callable: {e:?}"));
    }
}

#[test]
fn a_builtin_name_is_still_usable_as_a_variable() {
    // DP-R1's whole point: these are signatures, not keywords. `round` is 회차 in an
    // amortisation schedule, and a language that bans that name to gain a builtin has made a
    // bad trade.
    compile_to_rust(
        "export fn f(months: i32) -> i32 { let mut round = 0 while round < months { round = round + 1 } return round }",
    )
    .expect("`round` must still work as a local name");
    compile_to_rust("export fn f(floor: f64) -> f64 { return floor }")
        .expect("...and as a parameter name");
}

#[test]
fn a_user_function_may_not_shadow_a_builtin() {
    // Rejected at the DECLARATION, like the `ml_`/`mlx_` prefixes — not silently shadowed.
    for name in ["floor", "ceil", "round", "trunc"] {
        let err = compile_to_ir(&format!(
            "fn {name}(x: f64) -> f64 {{ return x }}\nexport fn f() -> f64 {{ return 1.0 }}"
        ))
        .unwrap_err();
        let msg = format!("{err:?}").to_lowercase();
        assert!(
            msg.contains("built-in") || msg.contains("builtin"),
            "{err:?}"
        );
        assert!(msg.contains(name), "{err:?}");
    }
    // An export by that name is equally a collision.
    assert!(compile_to_ir("export fn floor(x: f64) -> f64 { return x }").is_err());
}

#[test]
fn the_argument_must_be_one_f64() {
    for src in [
        "export fn f(a: i32) -> f64 { return floor(a) }",
        "export fn f(a: bool) -> f64 { return floor(a) }",
        "export fn f(a: f64) -> f64 { return floor() }",
        "export fn f(a: f64) -> f64 { return floor(a, a) }",
    ] {
        assert!(compile_to_ir(src).is_err(), "{src} must be rejected");
    }
    // i32 needs an explicit cast, like everywhere else (DP-I2 is not relaxed).
    compile_to_rust("export fn f(a: i32) -> f64 { return floor(a as f64) }").expect("cast is fine");
}

#[test]
fn round_does_not_use_the_naive_add_half_form() {
    // Section 2.4 trap 1: `floor(x + 0.5)` returns 1 for 0.49999999999999994. The emitted
    // helper must compute the fractional part instead. Pinning the SHAPE here is what stops
    // someone "simplifying" it later; the value is measured in the oracle test.
    let rust = compile_to_rust("export fn f(x: f64) -> f64 { return round(x) }").expect("compile");
    assert!(
        !rust.contains("+ 0.5"),
        "round must not be lowered as floor(x + 0.5):\n{rust}"
    );
    assert!(
        rust.contains("0.5"),
        "it still has to compare against a half:\n{rust}"
    );
}

#[test]
fn the_helpers_carry_the_signed_zero_step() {
    // Section 2.4 trap 2: without it `ceil(-0.5)` is +0.0 and C disagrees. `x * 0.0` is the
    // sign-carrying multiply (IEEE: the sign of a product is the XOR of the signs).
    let rust = compile_to_rust("export fn f(x: f64) -> f64 { return ceil(x) }").expect("compile");
    assert!(rust.contains("* 0.0"), "signed-zero step missing:\n{rust}");
}

#[test]
fn no_std_only_operations_are_used() {
    // The reason this slice is not a one-line lowering: `f64::floor` lives in std, and taking
    // `libm` would end the zero-dependency property. Nothing method-shaped may appear.
    let rust = compile_to_rust(
        "export fn f(x: f64) -> f64 { return floor(x) + ceil(x) + round(x) + trunc(x) }",
    )
    .expect("compile");
    for m in [".floor()", ".ceil()", ".round()", ".trunc()", ".abs()"] {
        assert!(
            !rust.contains(m),
            "std-only method {m} in a no_std crate:\n{rust}"
        );
    }
}

#[test]
fn only_the_builtins_actually_called_are_emitted() {
    // A module that rounds nothing should not carry the helpers.
    let plain = compile_to_rust("export fn f(x: f64) -> f64 { return x + 1.0 }").expect("compile");
    assert!(
        !plain.contains("ml_trunc"),
        "unused helper emitted:\n{plain}"
    );

    let one = compile_to_rust("export fn f(x: f64) -> f64 { return floor(x) }").expect("compile");
    assert!(one.contains("ml_floor"), "{one}");
    assert!(!one.contains("ml_ceil"), "only what is called:\n{one}");
}

/// Every rounder the frontend accepts is a rounder codegen emits.
///
/// The compile-time half of this is stronger and lives in the types: `Rounder` is an enum and
/// `emit_rounding_helpers` matches on it exhaustively, so adding a variant does not build
/// until the emitter answers. Measured — adding a fifth variant gives
/// `error[E0004]: non-exhaustive patterns: `Rounder::Sqrt` not covered` at two sites.
///
/// This test covers what the type cannot: `Rounder::ALL` is a hand-written list, and dropping
/// a variant FROM it compiles fine — the signature would simply never be registered, and a
/// call would be rejected as an unknown function. That is a benign direction, but it is the
/// one direction left, so it gets an assertion rather than an argument.
///
/// Before the enum, the missing direction was the dangerous one: with `BUILTIN_ROUNDERS` as
/// `&[&str]` and a `_ => {}` in the emitter, adding `"sqrt"` made `return sqrt(x)` pass the
/// frontend, emit `ml_sqrt(x)`, and emit no `fn ml_sqrt` — measured.
#[test]
fn every_declared_rounder_is_emitted_when_it_is_called() {
    for r in mlc::typeck::Rounder::ALL {
        let name = r.name();
        let src = format!("export fn f(x: f64) -> f64 {{ return {name}(x) }}");
        let rust = mlc::compile_to_rust(&src)
            .unwrap_or_else(|e| panic!("the frontend must accept the built-in `{name}`: {e}"));
        assert!(
            rust.contains(&format!("fn ml_{name}(")),
            "`{name}` is registered as a built-in but codegen emits no `fn ml_{name}`:\n{rust}"
        );
        assert!(
            rust.contains(&format!("ml_{name}(x)")),
            "...and the call must reach it:\n{rust}"
        );
    }
}

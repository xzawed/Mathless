//! Generated identifiers must not be spellable as user names.
//!
//! codegen injects identifiers into the SAME Rust scope as the user's parameters and locals:
//! `__d` for the i32 division/remainder guard, the `ml_*` rounding helpers, `out_value` for
//! the D17 out-param. `out_value` was reserved from the start; the other two were not, and
//! both were reachable from ordinary source:
//!
//! ```text
//! export fn f(__d: i32, b: i32) -> i32 { return __d / b }
//!     f(17,5) = 1   f(100,10) = 1   f(-17,5) = 1     <- always 1, measured
//!
//! export fn f(ml_floor: f64) -> f64 { return floor(ml_floor) }
//!     error[E0618]: expected function, found `f64`   <- in generated Rust
//! ```
//!
//! The first is the worse one: it compiled, returned a plausible number, and was wrong — the
//! fourth instance of the shape this repo has now closed four times (STATUS 3a-3).
//!
//! There is no codegen-side escape: Mathless identifiers and Rust identifiers are the same
//! character set, so no temporary name is unspellable. The frontend has to reserve them.

use mlc::{compile_to_ir, compile_to_rust};

fn err_of(src: &str) -> String {
    format!("{:?}", compile_to_ir(src).unwrap_err()).to_lowercase()
}

#[test]
fn the_division_guard_temporary_cannot_be_shadowed() {
    // The measured miscompile. Rejected at the source now, so the program that returned 1 for
    // every divisor cannot be written at all.
    let msg = err_of("export fn f(__d: i32, b: i32) -> i32 { return __d / b }");
    assert!(msg.contains("__d"), "the name must be quoted back: {msg}");
    assert!(
        msg.contains("generated") || msg.contains("compiler"),
        "the message must say why the name is unavailable: {msg}"
    );
}

#[test]
fn a_rounding_helper_name_cannot_be_shadowed() {
    // Same class, loud instead of silent: this one reached rustc as `ml_floor(ml_floor)`.
    let msg = err_of("export fn f(ml_floor: f64) -> f64 { return floor(ml_floor) }");
    assert!(msg.contains("ml_floor"), "{msg}");
}

#[test]
fn the_reservation_covers_every_place_a_name_enters_the_generated_scope() {
    // Parameters and locals both land in the emitted function body RAW, so both need the
    // rule. Function names no longer do: since the wrapper refactor every function is emitted
    // as `ml_fn_<name>`, so a user function name never enters the generated scope at all
    // (DP-W4) — the case at the end asserts that freedom instead of the old rejection.
    for src in [
        // parameters
        "export fn f(__x: i32) -> i32 { return __x }",
        "export fn f(ml_helper: f64) -> f64 { return ml_helper }",
        "export fn f(mlx_thing: f64) -> f64 { return mlx_thing }",
        // locals
        "export fn f(a: i32) -> i32 { let __t = a return __t }",
        "export fn f(a: f64) -> f64 { let ml_sz = a return ml_sz }",
        "export fn f(a: i32) -> i32 { let mut __acc = a __acc = a return __acc }",
    ] {
        assert!(compile_to_ir(src).is_err(), "{src} must be rejected");
    }

    // ...and a FUNCTION name is no longer one of those places. `__helper` becomes
    // `ml_fn___helper`, which collides with nothing — the generated temporaries `__v`, `__e`
    // and `__d` are locals, not functions.
    compile_to_ir(
        "fn __helper(a: i32) -> i32 { return a }\nexport fn f(a: i32) -> i32 { return __helper(a) }",
    )
    .expect("a function name never reaches the generated scope");
}

#[test]
fn ordinary_names_that_merely_look_similar_are_untouched() {
    // The rule is about PREFIXES the compiler owns, not about underscores generally. A single
    // leading underscore is a normal private-ish name and stays legal; so does anything with
    // `ml` in it that is not the reserved namespace.
    for src in [
        "export fn f(_x: i32) -> i32 { return _x }",
        "export fn f(a: i32) -> i32 { let _t = a return _t }",
        "export fn f(html: f64) -> f64 { return html }",
        "export fn f(ml: f64) -> f64 { return ml }",
        "export fn f(mlx: f64) -> f64 { return mlx }",
        "export fn f(a_ml_b: f64) -> f64 { return a_ml_b }",
    ] {
        compile_to_rust(src).unwrap_or_else(|e| panic!("{src} must still compile: {e:?}"));
    }
}

#[test]
fn the_division_still_works_for_an_ordinarily_named_parameter() {
    // The fix must not disturb the guard itself; the values are measured in
    // hosts/rust-oracle/tests/i32_division.rs.
    let rust =
        compile_to_rust("export fn f(a: i32, b: i32) -> i32 { return a / b }").expect("compile");
    assert!(
        rust.contains("let __d ="),
        "the guard is unchanged:\n{rust}"
    );
    assert!(rust.contains("wrapping_div"), "{rust}");
}

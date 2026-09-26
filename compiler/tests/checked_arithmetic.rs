//! Checked i32 arithmetic in fallible bodies — what the bindings promise (`SPEC-checked-arithmetic`).
//!
//! The values live in `hosts/rust-oracle/tests/checked_arithmetic.rs`. What is here is decided
//! without loading a module: which modules declare `ML_ST_OVERFLOW` (§2.5 — exactly those
//! that can return it, one predicate for both bindings), and that the fingerprint does not
//! move (DP-O4).

use mlc::compile_to_ir;
use mlc::header::{emit_c_header, emit_delphi_unit};
use mlc::iface::fingerprint;

fn declares(src: &str) -> (bool, bool) {
    let ir = compile_to_ir(src).expect("compile");
    let h = emit_c_header(&ir, "m");
    let pas = emit_delphi_unit(&ir, "m");
    (
        h.contains("#define ML_ST_OVERFLOW (-3)"),
        pas.contains("ML_ST_OVERFLOW = -3;"),
    )
}

/// **§2.5: the status is declared exactly where it can be returned — and both bindings agree.**
#[test]
fn only_a_module_that_can_overflow_declares_the_status() {
    for (src, can) in [
        // Every checked operation, each in a fallible body.
        ("export fn f(a: i32, b: i32) -> i32! { return a + b }", true),
        ("export fn f(a: i32, b: i32) -> i32! { return a - b }", true),
        ("export fn f(a: i32, b: i32) -> i32! { return a * b }", true),
        ("export fn f(a: i32) -> i32! { return -a }", true),
        ("export fn f(a: i32, b: i32) -> i32! { return a / b }", true),
        ("export fn f(x: f64) -> i32! { return x as i32 }", true),
        // The other two fallible shapes.
        ("export fn f(a: i32) -> string! { return (a * 2) as string }", true),
        (
            "export fn f(a: i32) -> [i32]! {\n  result 1\n  result[0] = a * 2\n}",
            true,
        ),
        // An internal fallible helper counts: it is a fallible body too.
        (
            "fn h(a: i32) -> i32! { return a * 2 }\nexport fn f(a: i32) -> i32! { let x = try h(a)  return x }",
            true,
        ),
        // Not checked: infallible bodies, `%` (DP-O5), f64 arithmetic, widening casts.
        ("export fn f(a: i32, b: i32) -> i32 { return a * b }", false),
        ("export fn f(a: i32, b: i32) -> i32! { return a % b }", false),
        ("export fn f(x: f64, y: f64) -> f64! { return x * y }", false),
        ("export fn f(a: i32) -> f64! { return a as f64 }", false),
        ("error E = 1\nexport fn f(a: i32) -> i32! { if a < 0 { fail E }  return a }", false),
    ] {
        let (h, pas) = declares(src);
        assert_eq!(h, pas, "the header and the unit disagree about {src:?}");
        assert_eq!(h, can, "{src:?} declares ML_ST_OVERFLOW: {h}, expected {can}");
    }
}

/// **DP-O4: the fingerprint does not see which reserved negatives a body reaches.**
///
/// The same signature with and without an overflowing operation has one fingerprint — the
/// manifest records signatures and error codes, and a reserved status is the ABI's (D17).
#[test]
fn checking_arithmetic_does_not_move_the_fingerprint() {
    let fp = |src: &str| fingerprint(&compile_to_ir(src).expect("compile"));
    assert_eq!(
        fp("export fn f(a: i32) -> i32! { return a }"),
        fp("export fn f(a: i32) -> i32! { return a * 2 }"),
    );
}

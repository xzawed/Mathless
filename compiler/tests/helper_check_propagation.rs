//! Helper check propagation — what is decided without loading a module
//! (`SPEC-helper-check-propagation` acceptance E, DP-P6, DP-P8, DP-P9).
//!
//! The values live in `hosts/rust-oracle/tests/helper_check_propagation.rs`. What is here: which
//! modules declare `ML_ST_OVERFLOW` now that `-3` can come through a helper (one set, read by
//! codegen and both bindings), which bodies the generated Rust carries, and that the
//! fingerprint does not move.

use mlc::header::{emit_c_header, emit_delphi_unit};
use mlc::iface::fingerprint;
use mlc::{compile_to_ir, compile_to_rust};

fn declares(src: &str) -> (bool, bool) {
    let ir = compile_to_ir(src).expect("compile");
    let h = emit_c_header(&ir, "m");
    let pas = emit_delphi_unit(&ir, "m");
    (
        h.contains("#define ML_ST_OVERFLOW (-3)"),
        pas.contains("ML_ST_OVERFLOW = -3;"),
    )
}

/// **Acceptance E: a module whose only way to `-3` is a helper declares it — and a module
/// whose helpers cannot overflow does not.**
///
/// The prototype measured the first row wrong: the module returned `-3` and neither binding
/// named it, because the predicate looked only at the fallible function's own body.
#[test]
fn a_module_that_reaches_overflow_only_through_a_helper_declares_the_status() {
    for (src, can) in [
        // The helper holds the only checked operation.
        (
            "fn sq(x: i32) -> i32 { return x * x }\nexport fn f(x: i32) -> i32! { return sq(x) }",
            true,
        ),
        // Two hops down.
        (
            "fn h2(x: i32) -> i32 { return x * x }\nfn h1(x: i32) -> i32 { return h2(x) }\n\
             export fn f(x: i32) -> i32! { return h1(x) }",
            true,
        ),
        // Through an EXPORTED infallible function (DP-P2).
        (
            "export fn wmul(a: i32, b: i32) -> i32 { return a * b }\n\
             export fn f(a: i32, b: i32) -> i32! { return wmul(a, b) }",
            true,
        ),
        // `f64 as i32` inside an f64 helper (DP-P3).
        (
            "fn toi(x: f64) -> f64 { return (x as i32) as f64 }\nexport fn f(x: f64) -> f64! { return toi(x) }",
            true,
        ),
        // The string and array shapes.
        (
            "fn sq(x: i32) -> i32 { return x * x }\nexport fn f(x: i32) -> string! { return sq(x) as string }",
            true,
        ),
        (
            "fn sq(x: i32) -> i32 { return x * x }\nexport fn f(x: i32) -> [i32]! {\n  result 1\n  result[0] = sq(x)\n}",
            true,
        ),
        // A helper with no checked operation, under a fallible caller: nothing to declare.
        // This is `examples/order.mls`'s shape (`known`).
        (
            "fn known(s: i32) -> bool { return s == 1 }\n\
             export fn f(s: i32) -> i32! {\n  if known(s) { return 1 }\n  return 0\n}",
            false,
        ),
        // A checked helper under an INFALLIBLE caller only: it wraps there, nothing to declare.
        (
            "fn sq(x: i32) -> i32 { return x * x }\nexport fn f(x: i32) -> i32 { return sq(x) }",
            false,
        ),
        // `%` only (DP-O5), through a helper.
        (
            "fn r(a: i32, b: i32) -> i32 { return a % b }\nexport fn f(a: i32, b: i32) -> i32! { return r(a, b) }",
            false,
        ),
    ] {
        let (h, pas) = declares(src);
        assert_eq!(h, pas, "the header and the unit disagree about {src:?}");
        assert_eq!(h, can, "{src:?} declares ML_ST_OVERFLOW: {h}, expected {can}");
    }
}

/// **DP-P8: only bodies somebody calls are emitted.**
///
/// A plain body that only fallible callers reached would be dead code once they call the
/// checked copy, and a copy that nothing calls is over-emission. Both are the defects that broke
/// builds under an ambient `RUSTFLAGS=-D warnings` in the prototype; `emit_robustness.rs` builds
/// that way, and this pins which bodies exist.
#[test]
fn only_called_bodies_are_emitted() {
    let rust = |src: &str| compile_to_rust(src).expect("compile");

    // Called only from a fallible body: the checked copy, and no plain body.
    let only_bang =
        rust("fn sq(x: i32) -> i32 { return x * x }\nexport fn f(x: i32) -> i32! { return sq(x) }");
    assert!(only_bang.contains("fn ml_ck_sq("), "{only_bang}");
    assert!(!only_bang.contains("fn ml_fn_sq("), "{only_bang}");

    // Called from both kinds of body: both.
    let both = rust(
        "fn sq(x: i32) -> i32 { return x * x }\nexport fn f(x: i32) -> i32! { return sq(x) }\n\
         export fn g(x: i32) -> i32 { return sq(x) }",
    );
    assert!(
        both.contains("fn ml_ck_sq(") && both.contains("fn ml_fn_sq("),
        "{both}"
    );

    // Nothing to check in the helper: no copy at all, and the fallible body calls the plain one.
    let no_ops = rust(
        "fn known(s: i32) -> bool { return s == 1 }\n\
         export fn f(s: i32) -> i32! {\n  if known(s) { return 1 }\n  return 0\n}",
    );
    assert!(!no_ops.contains("ml_ck_"), "{no_ops}");

    // An exported callee keeps its plain body for the adapter and gains a copy.
    let export = rust(
        "export fn wmul(a: i32, b: i32) -> i32 { return a * b }\n\
         export fn f(a: i32, b: i32) -> i32! { return wmul(a, b) }",
    );
    assert!(
        export.contains("fn ml_ck_wmul(") && export.contains("fn ml_fn_wmul("),
        "{export}"
    );

    // A helper nobody calls is emitted exactly as before this slice (acceptance H).
    let uncalled =
        rust("fn sq(x: i32) -> i32 { return x * x }\nexport fn f(x: i32) -> i32! { return x }");
    assert!(
        uncalled.contains("fn ml_fn_sq(") && !uncalled.contains("ml_ck_"),
        "{uncalled}"
    );
}

/// **DP-P9: the fingerprint does not see which reserved negatives a call path reaches.**
#[test]
fn propagating_the_check_does_not_move_the_fingerprint() {
    let fp = |src: &str| fingerprint(&compile_to_ir(src).expect("compile"));
    assert_eq!(
        fp("fn sq(x: i32) -> i32 { return x }\nexport fn f(x: i32) -> i32! { return sq(x) }"),
        fp("fn sq(x: i32) -> i32 { return x * x }\nexport fn f(x: i32) -> i32! { return sq(x) }"),
    );
}

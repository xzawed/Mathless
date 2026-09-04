//! `SPEC-error-prefix` — the module-prefixed error constant (Q14 / DP-Q1).
//!
//! `examples/refund.mls` exists to collide: it declares `E_NEG = 3` where
//! `examples/shapes.mls` declares `E_NEG = 1`. Two authors who never met would both reach
//! for that name, and an error name is module-scoped in the surface language, so neither is
//! wrong. Unprefixed, the two generated headers defined the same macro with different values
//! and a host including both failed to build — measured, `C4005` under `/W4 /WX`.
//!
//! The collision is proved harmless by ACCEPTANCE D, which compiles every example's header
//! in one translation unit; this file is the other half — that the module still behaves, so
//! the fixture is a real module and not a header-shaped decoy.
#![cfg(windows)]

use ml_oracle::{pe, Module};
use mlc::emit::emit_artifacts;

mod common;

#[test]
fn the_colliding_fixture_is_a_real_module_that_still_answers() {
    let src = include_str!("../../../examples/refund.mls");
    let out = common::TempOut::new("error_prefix");
    let arts = emit_artifacts(src, "refund", &out).expect("emit refund");

    let m = Module::load(arts.dll.to_str().unwrap()).expect("load refund.dll");
    // int32_t mlx_refund(double paid, int32_t days_since, double* out_value)
    let refund: extern "C" fn(f64, i32, *mut f64) -> i32 =
        unsafe { std::mem::transmute(m.symbol(b"mlx_refund\0").unwrap()) };

    // Success: 10% restocking fee.
    let mut v: f64 = -999.0;
    assert_eq!(refund(100.0, 5, &mut v as *mut f64), 0, "success status");
    assert_eq!(v, 90.0, "refund(100, 5)");

    // E_NEG = 3 here. In `shapes` the same NAME is 1 — that is the whole point of the
    // fixture, and the value that comes back is this module's, not the other one's.
    let mut neg: f64 = -999.0;
    assert_eq!(
        refund(-1.0, 0, &mut neg as *mut f64),
        3,
        "E_NEG is 3 in refund, whatever another module calls its own E_NEG"
    );
    assert_eq!(neg, -999.0, "out-param untouched on failure (DP-E3)");

    // E_TOO_LATE = 4.
    let mut late: f64 = -999.0;
    assert_eq!(refund(100.0, 31, &mut late as *mut f64), 4, "E_TOO_LATE");
    assert_eq!(late, -999.0, "out-param untouched on failure (DP-E3)");

    // The names are constants in the header, never exports: renaming them cannot change the
    // export set, which is what makes this slice ABI-neutral.
    let mut exports = pe::read_exports(&arts.dll).expect("read exports");
    exports.sort();
    assert_eq!(
        exports,
        vec![
            "ml_iface_hash".to_string(),
            "ml_module_abi_version".to_string(),
            "mlx_refund".to_string(),
        ],
        "error constants are #defines, not exports"
    );
}

/// The generated bindings carry the module in the constant's name, on both sides.
///
/// Checked on the text rather than only through the golden, so the rule is stated where a
/// reader looks for it — and so a golden re-bless cannot quietly accept a bare `ML_ERR_`.
#[test]
fn both_bindings_prefix_the_error_constant_with_the_module() {
    let src = include_str!("../../../examples/refund.mls");
    let out = common::TempOut::new("error_prefix_text");
    let arts = emit_artifacts(src, "refund", &out).expect("emit refund");

    let h = std::fs::read_to_string(&arts.header).expect("read .h");
    let pas = std::fs::read_to_string(&arts.delphi_unit).expect("read .pas");

    assert!(
        h.contains("#define ML_REFUND_ERR_E_NEG 3"),
        "the C header must prefix the error constant with the module:\n{h}"
    );
    assert!(
        pas.contains("ML_REFUND_ERR_E_NEG = 3;"),
        "the Delphi unit must use the same name — Pascal is unit-scoped and does not NEED \
         the prefix, but two bindings with different names for one constant is a second \
         thing to document and get wrong (DP-Q2):\n{pas}"
    );

    // No `#ifndef` around it, deliberately (DP-Q3): a guard would let the first header win
    // and turn a loud C4005 into a silent wrong meaning.
    assert!(
        !h.contains("#ifndef ML_REFUND_ERR_E_NEG"),
        "the error constant must NOT be #ifndef-guarded — see header.rs::error_macro"
    );

    for stale in ["ML_ERR_E_NEG", "ML_ERR_E_TOO_LATE"] {
        assert!(
            !h.contains(stale) && !pas.contains(stale),
            "a bare '{stale}' is still emitted; the prefix is what removes the collision"
        );
    }
}

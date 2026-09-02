//! D17 error-path slice — acceptance B/C (E2): the oracle loads the packaged fallible
//! module and calls both paths through the D17 ABI (i32 status + out-param). This is the
//! Rust oracle only; the same two paths are checked from a real C host in `c_host.rs`
//! (acceptance D). Delphi remains unverified.
#![cfg(windows)]

use ml_oracle::{pe, Module};
use mlc::emit::emit_artifacts;

mod common;

#[test]
fn oracle_calls_fallible_success_and_failure_paths() {
    let src = include_str!("../../../examples/safe_div.mls");
    let out = common::TempOut::new("d17");
    let arts = emit_artifacts(src, "safe_div", &out).expect("emit safe_div");

    let m = Module::load(arts.dll.to_str().unwrap()).expect("load safe_div.dll");
    let ver: extern "C" fn() -> u32 =
        unsafe { std::mem::transmute(m.symbol(b"ml_module_abi_version\0").unwrap()) };
    // int32_t mlx_safe_div(double a, double b, double* out_value)
    let safe_div: extern "C" fn(f64, f64, *mut f64) -> i32 =
        unsafe { std::mem::transmute(m.symbol(b"mlx_safe_div\0").unwrap()) };
    assert_eq!(ver(), mlc::ML_MODULE_ABI_VERSION, "abi version");

    // Success path: status 0, value written to the out-param.
    let mut out_ok: f64 = -999.0; // finite sentinel (NaN would break the unchanged check)
    let s = safe_div(6.0, 2.0, &mut out_ok as *mut f64);
    assert_eq!(s, 0, "success status");
    assert_eq!(out_ok, 3.0, "success value via out-param");

    // Failure path: status = DIV_BY_ZERO (positive), out-param left UNCHANGED (D17 contract).
    let mut out_err: f64 = -999.0;
    let s2 = safe_div(1.0, 0.0, &mut out_err as *mut f64);
    assert_eq!(s2, 1, "DIV_BY_ZERO status (positive domain error)");
    assert_eq!(out_err, -999.0, "out-param must be untouched on failure");

    // Protection: exactly the intended exports (error codes are constants, not exports).
    let mut exports = pe::read_exports(&arts.dll).expect("read exports");
    exports.sort();
    assert_eq!(
        exports,
        vec![
            "ml_iface_hash".to_string(),
            "ml_module_abi_version".to_string(),
            "mlx_safe_div".to_string(),
        ]
    );
}

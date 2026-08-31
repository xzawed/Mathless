//! out-params slice — acceptance A/B/B2/C (E2).
//!
//! Section 3-B2 is the one that matters: a declared `out` and D17's implicit `out_value` have
//! to compose. DP-O1 puts the declared one first and the return value last; DP-O3 says a
//! `fail` writes neither. Both are checked here against a real loaded module rather than
//! against the generated text.
#![cfg(windows)]

use ml_oracle::{pe, Module};
use mlc::emit::emit_artifacts;

fn build(tag: &str) -> (std::path::PathBuf, Module) {
    let src = include_str!("../../../examples/commission.mls");
    let out = std::env::temp_dir().join(format!("mlc_out_{tag}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&out);
    std::fs::create_dir_all(&out).unwrap();
    let arts = emit_artifacts(src, "commission", &out).expect("emit commission");
    let m = Module::load(arts.dll.to_str().unwrap()).expect("load commission.dll");
    (out, m)
}

#[test]
fn a_second_value_actually_comes_back() {
    let (out, m) = build("basic");
    let commission: extern "C" fn(f64, *mut i32) -> f64 =
        unsafe { std::mem::transmute(m.symbol(b"mlx_commission\0").unwrap()) };

    // The host allocates. The module writes through the pointer and returns the fee.
    //
    // The expected fee is written as the same IEEE expression rather than a hand-typed
    // decimal: 9_000_000.0 * 0.07 is 630000.0000000001 in f64, and pinning the rounded
    // decimal would be asserting something the arithmetic does not say. What is being
    // measured here is the out-param, not floating point.
    let mut tier = -1i32;
    assert_eq!(commission(500_000.0, &mut tier), 500_000.0f64 * 0.03);
    assert_eq!(tier, 1, "the bracket must come back too");

    let mut tier = -1i32;
    assert_eq!(commission(2_000_000.0, &mut tier), 2_000_000.0f64 * 0.05);
    assert_eq!(tier, 2);

    let mut tier = -1i32;
    assert_eq!(commission(9_000_000.0, &mut tier), 9_000_000.0f64 * 0.07);
    assert_eq!(tier, 3);

    drop(m);
    let _ = std::fs::remove_dir_all(&out);
}

#[test]
fn a_declared_out_composes_with_the_d17_out_value() {
    // Section 3-B2. The signature is (inputs…, declared outs…, out_value) — DP-O1.
    let (out, m) = build("compose");
    let checked: extern "C" fn(f64, *mut i32, *mut f64) -> i32 =
        unsafe { std::mem::transmute(m.symbol(b"mlx_commission_checked\0").unwrap()) };

    let mut tier = -1i32;
    let mut fee = -1.0f64;
    assert_eq!(checked(500_000.0, &mut tier, &mut fee), 0, "status 0");
    assert_eq!(tier, 1, "declared out written");
    assert_eq!(fee, 500_000.0f64 * 0.03, "out_value written");

    // DP-O3: on failure NEITHER is touched. The host is told not to read them; this measures
    // that it would also find them unchanged if it did.
    let mut tier = -7i32;
    let mut fee = -7.0f64;
    assert_eq!(checked(-1.0, &mut tier, &mut fee), 1, "E_NEGATIVE");
    assert_eq!(tier, -7, "declared out untouched on failure");
    assert_eq!(fee, -7.0, "out_value untouched on failure");

    drop(m);
    let _ = std::fs::remove_dir_all(&out);
}

#[test]
fn the_export_surface_is_unchanged_in_shape() {
    // Section 3-C: an out-param changes a signature, not the export set.
    let (out, m) = build("prot");
    drop(m);
    let dll = out.join("commission.dll");
    let mut names = pe::read_exports(&dll).expect("read exports");
    names.sort();
    assert_eq!(
        names,
        vec![
            "ml_module_abi_version".to_string(),
            "mlx_commission".to_string(),
            "mlx_commission_checked".to_string(),
        ],
        "nothing else may be exported"
    );
    let size = std::fs::metadata(&dll).unwrap().len();
    println!("commission.dll = {size} B");
    assert!(size < 60_000, "still a small stripped module: {size}");

    let _ = std::fs::remove_dir_all(&out);
}

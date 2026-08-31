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

/// DP-O3 declined to GUARANTEE that a failing call leaves the declared out untouched — it
/// chose "not enforced + a host contract" over buffering and committing atomically. This
/// measures what that actually means, because `HOST_ABI.md` used to carry a parenthetical
/// saying a failed call "writes nothing (verified by measurement)", which reads like the
/// guarantee DP-O3 refused to give.
///
/// `examples/commission.mls` happens to assign its out AFTER the failure check, so it does
/// leave the out untouched — that is a property of that program, not of the language. This
/// module writes the out FIRST and then fails, which is legal Mathless.
#[test]
fn a_failing_call_may_leave_a_declared_out_written() {
    const SRC: &str = "error E_TOO_BIG = 1\n\
                       export fn order_check(qty: i32, out offending: i32) -> i32! {\n\
                         offending = qty\n\
                         if qty > 100 { fail E_TOO_BIG }\n\
                         return 0\n\
                       }";
    let out = std::env::temp_dir().join(format!("mlc_out_o3_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&out);
    std::fs::create_dir_all(&out).unwrap();
    let arts = emit_artifacts(SRC, "order_check", &out).expect("emit order_check");
    let m = Module::load(arts.dll.to_str().unwrap()).expect("load order_check.dll");

    let f: extern "C" fn(i32, *mut i32, *mut i32) -> i32 =
        unsafe { std::mem::transmute(m.symbol(b"mlx_order_check\0").unwrap()) };

    // Success: both are written, as always.
    let (mut offending, mut value) = (-7i32, -7i32);
    assert_eq!(f(50, &mut offending, &mut value), 0);
    assert_eq!(offending, 50);
    assert_eq!(value, 0);

    // Failure: the status is the error code, `out_value` is untouched (D17 DP-E3 IS enforced,
    // because it is written only on the return path) — but the DECLARED out was already
    // assigned before `fail`, and it stays written.
    let (mut offending, mut value) = (-7i32, -7i32);
    assert_eq!(f(250, &mut offending, &mut value), 1);
    assert_eq!(
        value, -7,
        "out_value is only written on the return path, so a failure leaves it alone"
    );
    assert_eq!(
        offending, 250,
        "DP-O3: the declared out is NOT rolled back. This is why the contract is \
         'do not read on status != 0', not 'the module does not write'"
    );

    drop(m);
    let _ = std::fs::remove_dir_all(&out);
}

//! Unary-operator slice — acceptance A/B/C (E2), plus the DP-U4 overflow measurement.
//! Unary operators are expression-level, so the ABI/exports are unchanged.
#![cfg(windows)]

use ml_oracle::{pe, Module};
use mlc::emit::emit_artifacts;

#[test]
fn oracle_loads_and_calls_a_module_using_unary_operators() {
    let src = include_str!("../../../examples/negate_if.mls");
    let out = std::env::temp_dir().join(format!("mlc_unary_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&out);
    std::fs::create_dir_all(&out).unwrap();
    let arts = emit_artifacts(src, "negate_if", &out).expect("emit negate_if");

    let m = Module::load(arts.dll.to_str().unwrap()).expect("load negate_if.dll");
    let ver: extern "C" fn() -> u32 =
        unsafe { std::mem::transmute(m.symbol(b"ml_module_abi_version\0").unwrap()) };
    let negate_if: extern "C" fn(i32, bool) -> i32 =
        unsafe { std::mem::transmute(m.symbol(b"mlx_negate_if\0").unwrap()) };
    assert_eq!(ver(), mlc::ML_MODULE_ABI_VERSION, "abi version");
    assert_eq!(negate_if(7, false), 7, "!flip -> unchanged");
    assert_eq!(negate_if(7, true), -7, "unary minus");

    let mut exports = pe::read_exports(&arts.dll).expect("read exports");
    exports.sort();
    assert_eq!(
        exports,
        vec![
            "ml_module_abi_version".to_string(),
            "mlx_negate_if".to_string(),
        ]
    );

    drop(m);
    let _ = std::fs::remove_dir_all(&out);
}

#[test]
fn negating_i32_min_wraps_to_itself() {
    // DP-U4: unary `-` follows the same wrapping rule as the rest of i32 arithmetic (DP-I4).
    // Measured rather than assumed — this is the one case where `-x` cannot produce the
    // mathematically correct answer.
    let out = std::env::temp_dir().join(format!("mlc_unary_min_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&out);
    std::fs::create_dir_all(&out).unwrap();
    let arts = emit_artifacts("export fn neg(x: i32) -> i32 { return -x }", "neg", &out)
        .expect("emit neg");

    let m = Module::load(arts.dll.to_str().unwrap()).expect("load neg.dll");
    let neg: extern "C" fn(i32) -> i32 =
        unsafe { std::mem::transmute(m.symbol(b"mlx_neg\0").unwrap()) };
    assert_eq!(neg(7), -7);
    assert_eq!(neg(-7), 7);
    assert_eq!(
        neg(i32::MIN),
        i32::MIN,
        "-i32::MIN wraps to itself; it does not trap or abort"
    );

    drop(m);
    let _ = std::fs::remove_dir_all(&out);
}

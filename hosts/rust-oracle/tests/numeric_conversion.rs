//! Numeric-conversion slice — acceptance A/B/C (E2), plus the boundary measurements that
//! turn SPEC section 2.3 from a claim into a number. `f64 as i32` truncates toward zero,
//! saturates at the bounds and maps NaN to 0 — checked against a real loaded module, not
//! against the Rust documentation.
#![cfg(windows)]

use ml_oracle::{pe, Module};
use mlc::emit::emit_artifacts;

#[test]
fn oracle_loads_and_calls_a_module_using_a_cast() {
    let src = include_str!("../../../examples/line_total.mls");
    let out = std::env::temp_dir().join(format!("mlc_cast_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&out);
    std::fs::create_dir_all(&out).unwrap();
    let arts = emit_artifacts(src, "line_total", &out).expect("emit line_total");

    let m = Module::load(arts.dll.to_str().unwrap()).expect("load line_total.dll");
    let ver: extern "C" fn() -> u32 =
        unsafe { std::mem::transmute(m.symbol(b"ml_module_abi_version\0").unwrap()) };
    let line_total: extern "C" fn(f64, i32) -> f64 =
        unsafe { std::mem::transmute(m.symbol(b"mlx_line_total\0").unwrap()) };
    assert_eq!(ver(), mlc::ML_MODULE_ABI_VERSION, "abi version");
    assert_eq!(line_total(2.5, 4), 10.0, "a count finally meets a price");
    assert_eq!(line_total(2.5, 0), 0.0);

    let mut exports = pe::read_exports(&arts.dll).expect("read exports");
    exports.sort();
    assert_eq!(
        exports,
        vec![
            "ml_module_abi_version".to_string(),
            "mlx_line_total".to_string(),
        ]
    );

    drop(m);
    let _ = std::fs::remove_dir_all(&out);
}

#[test]
fn f64_to_i32_truncates_saturates_and_maps_nan_to_zero() {
    // SPEC section 2.3 is a Mathless rule, so it is measured here rather than inherited from
    // whatever the backend happens to do. `huge` and `nan` are built inside the module from
    // f64 division, which is the only way to reach infinity and NaN today.
    let out = std::env::temp_dir().join(format!("mlc_cast_edge_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&out);
    std::fs::create_dir_all(&out).unwrap();
    let arts = emit_artifacts(
        "export fn trunc(x: f64) -> i32 { return x as i32 }\n\
         export fn over(x: f64) -> i32 { return (x / 0.0) as i32 }\n\
         export fn nan_to(x: f64) -> i32 { return (0.0 / 0.0) as i32 }\n\
         export fn widen(n: i32) -> f64 { return n as f64 }",
        "edges",
        &out,
    )
    .expect("emit edges");

    let m = Module::load(arts.dll.to_str().unwrap()).expect("load edges.dll");
    let trunc: extern "C" fn(f64) -> i32 =
        unsafe { std::mem::transmute(m.symbol(b"mlx_trunc\0").unwrap()) };
    let over: extern "C" fn(f64) -> i32 =
        unsafe { std::mem::transmute(m.symbol(b"mlx_over\0").unwrap()) };
    let nan_to: extern "C" fn(f64) -> i32 =
        unsafe { std::mem::transmute(m.symbol(b"mlx_nan_to\0").unwrap()) };
    let widen: extern "C" fn(i32) -> f64 =
        unsafe { std::mem::transmute(m.symbol(b"mlx_widen\0").unwrap()) };

    // Truncation is toward zero, not floor: -3.9 becomes -3, not -4.
    assert_eq!(trunc(3.9), 3, "truncate toward zero");
    assert_eq!(trunc(-3.9), -3, "toward zero, not floor");
    assert_eq!(trunc(0.0), 0);

    // Out of range saturates instead of wrapping or trapping.
    assert_eq!(over(1.0), i32::MAX, "+inf saturates to i32::MAX");
    assert_eq!(over(-1.0), i32::MIN, "-inf saturates to i32::MIN");

    // NaN becomes 0 rather than an arbitrary bit pattern.
    assert_eq!(nan_to(0.0), 0, "NaN maps to 0");

    // Widening is exact for every i32 — f64 has 53 bits of mantissa.
    assert_eq!(widen(i32::MAX), 2147483647.0);
    assert_eq!(widen(i32::MIN), -2147483648.0);

    drop(m);
    let _ = std::fs::remove_dir_all(&out);
}

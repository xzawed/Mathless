//! Logical-operator slice — acceptance A/B/C (E2): the oracle loads a module whose loop
//! header combines two conditions, and calls it. Expression-level change, so the
//! ABI/exports are unchanged.
#![cfg(windows)]

use ml_oracle::{pe, Module};
use mlc::emit::emit_artifacts;

#[test]
fn oracle_loads_and_calls_a_module_using_logical_operators() {
    let src = include_str!("../../../examples/count_bounded.mls");
    let out = std::env::temp_dir().join(format!("mlc_logic_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&out);
    std::fs::create_dir_all(&out).unwrap();
    let arts = emit_artifacts(src, "count_bounded", &out).expect("emit count_bounded");

    let m = Module::load(arts.dll.to_str().unwrap()).expect("load count_bounded.dll");
    let ver: extern "C" fn() -> u32 =
        unsafe { std::mem::transmute(m.symbol(b"ml_module_abi_version\0").unwrap()) };
    let count: extern "C" fn(i32, i32) -> i32 =
        unsafe { std::mem::transmute(m.symbol(b"mlx_count_bounded\0").unwrap()) };
    assert_eq!(ver(), mlc::ML_MODULE_ABI_VERSION, "abi version");
    // Whichever bound is smaller stops the loop — both operands of `&&` matter.
    assert_eq!(count(10, 3), 3, "cap stops it");
    assert_eq!(count(3, 10), 3, "n stops it");
    assert_eq!(count(0, 5), 0, "body runs zero times");

    let mut exports = pe::read_exports(&arts.dll).expect("read exports");
    exports.sort();
    assert_eq!(
        exports,
        vec![
            "ml_iface_hash".to_string(),
            "ml_module_abi_version".to_string(),
            "mlx_count_bounded".to_string(),
        ]
    );

    drop(m);
    let _ = std::fs::remove_dir_all(&out);
}

#[test]
fn or_short_circuit_is_specified_but_not_observable_here() {
    // SPEC-logical-ops DP-B2 / section 2.3: `||` short-circuits because the lowering is
    // Rust's `||`. Nothing in the language can observe that yet — there are no calls and no
    // trapping operations — so this asserts the RESULT, not the evaluation. When a construct
    // arrives that can tell the difference, the real test belongs with it.
    let out = std::env::temp_dir().join(format!("mlc_logic_or_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&out);
    std::fs::create_dir_all(&out).unwrap();
    let arts = emit_artifacts(
        "export fn any(a: bool, b: bool) -> bool { return a || b }",
        "any",
        &out,
    )
    .expect("emit any");

    let m = Module::load(arts.dll.to_str().unwrap()).expect("load any.dll");
    let any: extern "C" fn(bool, bool) -> bool =
        unsafe { std::mem::transmute(m.symbol(b"mlx_any\0").unwrap()) };
    assert!(any(true, false));
    assert!(any(false, true));
    assert!(!any(false, false));
    assert!(any(true, true));

    drop(m);
    let _ = std::fs::remove_dir_all(&out);
}

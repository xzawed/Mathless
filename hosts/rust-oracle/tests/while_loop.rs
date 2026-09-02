//! `while` slice — acceptance A/B/C (E2): the oracle loads a module whose export loops, and
//! calls it. Control flow is internal, so the ABI/exports are unchanged.
#![cfg(windows)]

use ml_oracle::{pe, Module};
use mlc::emit::emit_artifacts;

#[test]
fn oracle_loads_and_calls_a_module_with_a_loop() {
    let src = include_str!("../../../examples/sum_to.mls");
    let out = std::env::temp_dir().join(format!("mlc_while_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&out);
    std::fs::create_dir_all(&out).unwrap();
    let arts = emit_artifacts(src, "sum_to", &out).expect("emit sum_to");

    let m = Module::load(arts.dll.to_str().unwrap()).expect("load sum_to.dll");
    let ver: extern "C" fn() -> u32 =
        unsafe { std::mem::transmute(m.symbol(b"ml_module_abi_version\0").unwrap()) };
    let sum_to: extern "C" fn(i32) -> i32 =
        unsafe { std::mem::transmute(m.symbol(b"mlx_sum_to\0").unwrap()) };
    assert_eq!(ver(), mlc::ML_MODULE_ABI_VERSION, "abi version");
    assert_eq!(sum_to(10), 55, "1+..+10");
    assert_eq!(sum_to(1), 1, "one iteration");
    assert_eq!(sum_to(0), 0, "the body runs zero times");
    assert_eq!(sum_to(-5), 0, "condition false from the start");

    // Control flow is internal — the export set is unchanged.
    let mut exports = pe::read_exports(&arts.dll).expect("read exports");
    exports.sort();
    assert_eq!(
        exports,
        vec![
            "ml_iface_hash".to_string(),
            "ml_module_abi_version".to_string(),
            "mlx_sum_to".to_string()
        ]
    );

    drop(m);
    let _ = std::fs::remove_dir_all(&out);
}

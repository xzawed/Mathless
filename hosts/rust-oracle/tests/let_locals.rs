//! Local-variables slice — acceptance A/B/C (E2): the oracle loads a module that uses a
//! `let` local and calls it. Locals are internal, so the ABI/exports are unchanged.
#![cfg(windows)]

use ml_oracle::{pe, Module};
use mlc::emit::emit_artifacts;

#[test]
fn oracle_loads_and_calls_a_module_using_a_local() {
    let src = include_str!("../../../examples/discount2.mls");
    let out = std::env::temp_dir().join(format!("mlc_let_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&out);
    std::fs::create_dir_all(&out).unwrap();
    let arts = emit_artifacts(src, "discount2", &out).expect("emit discount2");

    let m = Module::load(arts.dll.to_str().unwrap()).expect("load discount2.dll");
    let ver: extern "C" fn() -> u32 =
        unsafe { std::mem::transmute(m.symbol(b"ml_module_abi_version\0").unwrap()) };
    let discount2: extern "C" fn(f64, bool) -> f64 =
        unsafe { std::mem::transmute(m.symbol(b"mlx_discount2\0").unwrap()) };
    assert_eq!(ver(), mlc::ML_MODULE_ABI_VERSION, "abi version");
    assert_eq!(discount2(100.0, true), 90.0, "vip uses the `rate` local");
    assert_eq!(discount2(100.0, false), 100.0, "non-vip");

    // The local `rate` is not an export — the export set is unchanged.
    let mut exports = pe::read_exports(&arts.dll).expect("read exports");
    exports.sort();
    assert_eq!(
        exports,
        vec![
            "ml_iface_hash".to_string(),
            "ml_module_abi_version".to_string(),
            "mlx_discount2".to_string(),
        ]
    );

    drop(m);
    let _ = std::fs::remove_dir_all(&out);
}

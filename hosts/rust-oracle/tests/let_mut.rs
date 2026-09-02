//! Mutable-locals slice — acceptance A/B/C (E2): the oracle loads a module that uses a
//! `let mut` local assigned inside an `if`, and calls it. Mutable locals are internal, so
//! the ABI/exports are unchanged.
#![cfg(windows)]

use ml_oracle::{pe, Module};
use mlc::emit::emit_artifacts;

#[test]
fn oracle_loads_and_calls_a_module_using_a_mutable_local() {
    let src = include_str!("../../../examples/discount3.mls");
    let out = std::env::temp_dir().join(format!("mlc_letmut_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&out);
    std::fs::create_dir_all(&out).unwrap();
    let arts = emit_artifacts(src, "discount3", &out).expect("emit discount3");

    let m = Module::load(arts.dll.to_str().unwrap()).expect("load discount3.dll");
    let ver: extern "C" fn() -> u32 =
        unsafe { std::mem::transmute(m.symbol(b"ml_module_abi_version\0").unwrap()) };
    let discount3: extern "C" fn(f64, bool) -> f64 =
        unsafe { std::mem::transmute(m.symbol(b"mlx_discount3\0").unwrap()) };
    assert_eq!(ver(), mlc::ML_MODULE_ABI_VERSION, "abi version");
    assert_eq!(
        discount3(100.0, true),
        90.0,
        "vip: the `if` assigns the mutable sink"
    );
    assert_eq!(
        discount3(100.0, false),
        100.0,
        "non-vip: the sink keeps its initial value"
    );

    // Mutable locals are internal — the export set is unchanged.
    let mut exports = pe::read_exports(&arts.dll).expect("read exports");
    exports.sort();
    assert_eq!(
        exports,
        vec![
            "ml_iface_hash".to_string(),
            "ml_module_abi_version".to_string(),
            "mlx_discount3".to_string(),
        ]
    );

    drop(m);
    let _ = std::fs::remove_dir_all(&out);
}

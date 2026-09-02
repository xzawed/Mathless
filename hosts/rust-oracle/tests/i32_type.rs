//! Integer-type (`i32`) slice — acceptance A/B/C (E2): the oracle loads an `i32` module and
//! calls it. i32 maps to a plain C `int32_t` across the ABI.
#![cfg(windows)]

use ml_oracle::{pe, Module};
use mlc::emit::emit_artifacts;

#[test]
fn oracle_loads_and_calls_an_i32_function() {
    let src = include_str!("../../../examples/add.mls");
    let out = std::env::temp_dir().join(format!("mlc_i32_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&out);
    std::fs::create_dir_all(&out).unwrap();
    let arts = emit_artifacts(src, "add", &out).expect("emit add");

    let m = Module::load(arts.dll.to_str().unwrap()).expect("load add.dll");
    let ver: extern "C" fn() -> u32 =
        unsafe { std::mem::transmute(m.symbol(b"ml_module_abi_version\0").unwrap()) };
    let add: extern "C" fn(i32, i32) -> i32 =
        unsafe { std::mem::transmute(m.symbol(b"mlx_add\0").unwrap()) };
    assert_eq!(ver(), mlc::ML_MODULE_ABI_VERSION, "abi version");
    assert_eq!(add(2, 3), 5, "i32 add");
    assert_eq!(add(10, -4), 6, "i32 add with a negative");

    let mut exports = pe::read_exports(&arts.dll).expect("read exports");
    exports.sort();
    assert_eq!(
        exports,
        vec![
            "ml_iface_hash".to_string(),
            "ml_module_abi_version".to_string(),
            "mlx_add".to_string()
        ]
    );

    drop(m);
    let _ = std::fs::remove_dir_all(&out);
}

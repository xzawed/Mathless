//! STEP 1 E2 (Grok cross-check #3): the oracle loads the DLL that `mlc build` actually
//! packages into the output directory — not just the compiler's internal build artifact —
//! and calls the typed function through it. This proves the *packaged* module is a real,
//! loadable C-ABI module. It does NOT touch the C/Delphi host-load gate (still BLOCKED).
#![cfg(windows)]

use ml_oracle::{pe, Module};
use mlc::emit::emit_artifacts;

#[test]
fn oracle_loads_and_calls_the_packaged_module() {
    let src = include_str!("../../../examples/discount.mls");
    let out = std::env::temp_dir().join(format!("mlc_emit_oracle_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&out);
    std::fs::create_dir_all(&out).unwrap();

    let arts = emit_artifacts(src, "discount", &out).expect("emit_artifacts");

    // Load the packaged DLL (out/discount.dll) and call it — same asserts as acceptance B.
    let m = Module::load(arts.dll.to_str().unwrap()).expect("load packaged dll");
    let ver: extern "C" fn() -> u32 =
        unsafe { std::mem::transmute(m.symbol(b"ml_module_abi_version\0").unwrap()) };
    let discount: extern "C" fn(f64, bool) -> f64 =
        unsafe { std::mem::transmute(m.symbol(b"mlx_discount\0").unwrap()) };
    assert_eq!(ver(), mlc::ML_MODULE_ABI_VERSION, "abi version");
    assert_eq!(discount(100.0, true), 90.0, "vip discount");
    assert_eq!(discount(100.0, false), 100.0, "non-vip");

    // The packaged DLL carries only the intended exports (D18) — the protection property
    // survives packaging/copy.
    let mut exports = pe::read_exports(&arts.dll).expect("read exports");
    exports.sort();
    assert_eq!(
        exports,
        vec![
            "ml_module_abi_version".to_string(),
            "mlx_discount".to_string(),
        ]
    );
}

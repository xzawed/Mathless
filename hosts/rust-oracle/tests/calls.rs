//! Calls slice — acceptance A/B/C (E2). The measurement that matters here is **C**: a module
//! with an internal helper must still export exactly two symbols. That is the D04/D05 story
//! — logic moves inside without growing the surface — and it is checked against the binary,
//! not argued.
#![cfg(windows)]

use ml_oracle::{pe, Module};
use mlc::emit::emit_artifacts;

#[test]
fn an_internal_helper_never_reaches_the_export_table() {
    let src = include_str!("../../../examples/discount4.mls");
    let out = std::env::temp_dir().join(format!("mlc_calls_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&out);
    std::fs::create_dir_all(&out).unwrap();
    let arts = emit_artifacts(src, "discount4", &out).expect("emit discount4");

    let m = Module::load(arts.dll.to_str().unwrap()).expect("load discount4.dll");
    let ver: extern "C" fn() -> u32 =
        unsafe { std::mem::transmute(m.symbol(b"ml_module_abi_version\0").unwrap()) };
    let discount4: extern "C" fn(f64, bool) -> f64 =
        unsafe { std::mem::transmute(m.symbol(b"mlx_discount4\0").unwrap()) };
    assert_eq!(ver(), mlc::ML_MODULE_ABI_VERSION, "abi version");
    assert_eq!(discount4(100.0, true), 90.0, "the helper decided the rate");
    assert_eq!(discount4(100.0, false), 100.0);

    // The point of the slice, measured: `vip_rate` is nowhere in the export table.
    let mut exports = pe::read_exports(&arts.dll).expect("read exports");
    exports.sort();
    assert_eq!(
        exports,
        vec![
            "ml_iface_hash".to_string(),
            "ml_module_abi_version".to_string(),
            "mlx_discount4".to_string(),
        ],
        "an internal helper must not be exported"
    );

    // Nor in the C header a host would consume.
    let header = std::fs::read_to_string(&arts.header).expect("read header");
    assert!(
        !header.contains("vip_rate"),
        "the header must describe only the surface:\n{header}"
    );
    let unit = std::fs::read_to_string(&arts.delphi_unit).expect("read unit");
    assert!(!unit.contains("vip_rate"), "{unit}");

    drop(m);
    let _ = std::fs::remove_dir_all(&out);
}

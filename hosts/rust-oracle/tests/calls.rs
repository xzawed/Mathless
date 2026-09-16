//! Calls slice — acceptance A/B/C (E2). The measurement that matters here is **C**: a module
//! with an internal helper must still export exactly two symbols. That is the D04/D05 story
//! — logic moves inside without growing the surface — and it is checked against the binary,
//! not argued.
#![cfg(windows)]

use ml_oracle::{pe, Module};
use mlc::emit::emit_artifacts;

mod common;

#[test]
fn an_internal_helper_never_reaches_the_export_table() {
    let src = include_str!("../../../examples/discount4.mls");
    let out = common::TempOut::new("calls");
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
            // Qualified with the module name since SPEC-qualified-iface-hash — and the
            // count, which is what acceptance C is about, is unchanged at three.
            "ml_iface_hash_discount4".to_string(),
            "ml_module_abi_version".to_string(),
            "mlx_discount4".to_string(),
        ],
        "an internal helper must not be exported"
    );

    // And the LOADER says so too, which nothing had ever asked it to.
    //
    // The export table above is read by our own PE reader; this is the other way a host finds
    // out, and it is the way a host actually uses. Measured 2026-09-16: the oracle's tests call
    // `symbol()` 103 times and **not one of them looks at the Result** — every call unwraps or
    // expects. Deleting the null check from `Module::symbol` so it returns `Ok(null)` changed
    // no test outcome, because every call asks for a symbol that is there.
    //
    // What the check buys is the difference between a named refusal and an access violation:
    // without it the caller transmutes a null pointer and calls it, which on this platform also
    // takes out the process — and with it every live `TempOut` in that process (STATUS §5-5.7,
    // measured the same day). One line, so the Err branch is no longer ceremony.
    let absent = m.symbol(b"vip_rate\0");
    assert!(
        absent.is_err(),
        "the loader must refuse a symbol that is not exported, not hand back a pointer"
    );
    assert!(
        absent.unwrap_err().contains("vip_rate"),
        "the refusal has to name the symbol, or it sends the reader looking"
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
}

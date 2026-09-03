//! STEP 1 E2 (Grok cross-check #3): the oracle loads the DLL that `mlc build` actually
//! packages into the output directory — not just the compiler's internal build artifact —
//! and calls the typed function through it. This proves the *packaged* module is a real,
//! loadable C-ABI module. It is not itself acceptance D — a real C host does that in
//! `c_host.rs`, and the Delphi arm still has no host.
#![cfg(windows)]

use ml_oracle::{pe, Module};
use mlc::emit::emit_artifacts;

mod common;

/// `SPEC-linkable-bindings` §3-A — the fourth artifact.
///
/// Until this slice `mlc build` shipped a header full of plain prototypes and no import
/// library beside it, so a C host that included the header and linked the normal way broke
/// at link time. The reference host never noticed: it uses the declarations only as a
/// `_Generic` type oracle and calls through `GetProcAddress`.
///
/// `cargo` was already producing the import library — `<crate>.dll.lib` next to the DLL —
/// and packaging simply left it behind. It is published under the artifact naming the rest
/// of the set uses, `<module>.<ext>`.
#[test]
fn mlc_build_packages_an_import_library_beside_the_module() {
    let src = include_str!("../../../examples/discount.mls");
    let out = common::TempOut::new("emit_implib");

    let arts = emit_artifacts(src, "discount", &out).expect("emit_artifacts");

    assert_eq!(
        arts.import_lib,
        out.join("discount.lib"),
        "the import library is published as <module>.lib, not under cargo's internal name"
    );
    let len = std::fs::metadata(&arts.import_lib)
        .expect("import library was not packaged")
        .len();
    assert!(len > 0, "the packaged import library is empty");

    // Exactly four files land, and nothing else: the staging directory is gone, and the
    // atomic move covers the whole set rather than three of four.
    let mut names: Vec<String> = std::fs::read_dir(&out)
        .expect("read out dir")
        .map(|e| e.expect("dir entry").file_name().to_string_lossy().into())
        .collect();
    names.sort();
    assert_eq!(
        names,
        vec![
            "discount.dll".to_string(),
            "discount.h".to_string(),
            "discount.lib".to_string(),
            "discount.pas".to_string(),
        ],
        "mlc build must leave exactly the four artifacts in the output directory"
    );
}

#[test]
fn oracle_loads_and_calls_the_packaged_module() {
    let src = include_str!("../../../examples/discount.mls");
    let out = common::TempOut::new("emit_oracle");

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
            "ml_iface_hash".to_string(),
            "ml_module_abi_version".to_string(),
            "mlx_discount".to_string(),
        ]
    );
}

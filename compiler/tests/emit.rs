//! STEP 1 (Gate-D prep): `emit_artifacts` packages a `.mls` module into the three
//! consumable files on disk — `<name>.dll` + `<name>.h` + `<name>.pas`.
//!
//! What this test asserts is E2: the files exist, the `.dll` is a real PE, and the
//! `.h`/`.pas` match the text contract. It does NOT itself compile or load them. A real C
//! host does that in `hosts/rust-oracle/tests/c_host.rs` (acceptance D); the `.pas` has
//! never been compiled, because there is no `dcc64` here. Building the DLL needs `cargo`,
//! so the test is Windows-gated like the other acceptance tests.
#![cfg(windows)]

use std::path::Path;

use mlc::emit::emit_artifacts;

mod common;
use common::TempOut;

/// Hold the returned guard for the body of the test — dropping it deletes the tree.
fn fresh_out(tag: &str) -> TempOut {
    TempOut::new(&format!("emit_{tag}"))
}

#[test]
fn emit_artifacts_writes_the_four_consumable_files() {
    let src = include_str!("../../examples/discount.mls");
    let out = fresh_out("contract");

    let arts = emit_artifacts(src, "discount", &out).expect("emit_artifacts");

    // The four deliverables land in the output dir under the module name.
    assert_eq!(arts.dll, out.join("discount.dll"));
    assert_eq!(arts.header, out.join("discount.h"));
    assert_eq!(arts.delphi_unit, out.join("discount.pas"));
    assert_eq!(arts.import_lib, out.join("discount.lib"));
    for p in [&arts.dll, &arts.header, &arts.delphi_unit, &arts.import_lib] {
        assert!(p.exists(), "missing artifact: {}", p.display());
    }

    // No build litter is left in the output dir — exactly the four files.
    let count = std::fs::read_dir(&out).unwrap().count();
    assert_eq!(count, 4, "output dir must contain exactly the 4 artifacts");

    // The .dll is a real PE image (MZ magic), not an empty/placeholder file.
    let dll = std::fs::read(&arts.dll).unwrap();
    assert!(
        dll.len() > 512,
        "dll suspiciously small: {} bytes",
        dll.len()
    );
    assert_eq!(&dll[..2], b"MZ", "dll is not a PE image");

    // .h text contract: the C-ABI signature, the reserved version symbol, and a note that
    // matches reality — the C binding IS now verified against a real host (acceptance D),
    // and the note says exactly what that does and does not cover.
    let h = std::fs::read_to_string(&arts.header).unwrap();
    assert!(h.contains("mlx_discount"), "{h}");
    assert!(h.contains("ml_module_abi_version"), "{h}");
    assert!(
        h.contains("GENERATOR is verified by acceptance D") && h.contains("MSVC"),
        "the header should record what verified it: {h}"
    );
    assert!(
        h.contains("not this\n * particular file") || h.contains("particular file"),
        "…and the limits of that claim: {h}"
    );
    // The header is handed to C compilers on machines with any code page; keep it ASCII.
    // (MSVC warns C4819 otherwise, and `/WX` builds then fail — measured, acceptance D.)
    assert!(
        h.is_ascii(),
        "the generated C header must be pure ASCII: {h}"
    );

    // .pas text contract: matching unit name (Delphi requires file stem == unit), the cdecl
    // external import, and the DRAFT note — which must STAY, because no `dcc64` has ever
    // compiled this. Acceptance D closed the C arm only.
    let pas = std::fs::read_to_string(&arts.delphi_unit).unwrap();
    assert!(pas.contains("unit discount;"), "{pas}");
    assert!(pas.contains("function mlx_discount"), "{pas}");
    assert!(pas.contains("external ML_MODULE"), "{pas}");
    assert!(
        pas.contains("D14 load gate BLOCKED"),
        "the Delphi unit must stay honest — it is still unverified: {pas}"
    );
    assert!(
        pas.is_ascii(),
        "the generated Delphi unit must be ASCII: {pas}"
    );
}

#[test]
fn emit_artifacts_rejects_a_source_that_does_not_typecheck() {
    // A non-bool `if` condition fails typeck; packaging must surface that, not panic.
    let bad = "export fn f(x: f64) -> f64 { if x { return x } return x }";
    let out = fresh_out("bad");
    let err = emit_artifacts(bad, "f", &out).unwrap_err();
    // The error carries the compile failure through, and nothing was written.
    let _ = err; // exact variant asserted below via Display
    assert!(
        format!("{err}").to_lowercase().contains("type")
            || format!("{err}").to_lowercase().contains("bool"),
        "expected a type error, got: {err}"
    );
    assert!(
        !out.join("f.dll").exists(),
        "no dll should be produced for a source that fails to compile"
    );
}

// Compile-time proof the public shape is what the CLI depends on.
#[allow(dead_code)]
fn _artifacts_shape(a: &mlc::emit::Artifacts) -> (&Path, &Path, &Path) {
    (&a.dll, &a.header, &a.delphi_unit)
}

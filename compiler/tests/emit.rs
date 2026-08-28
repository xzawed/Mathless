//! STEP 1 (Gate-D prep): `emit_artifacts` packages a `.mls` module into the three
//! consumable files on disk — `<name>.dll` + `<name>.h` + `<name>.pas`.
//!
//! Honesty (Grok cross-check): what this test may assert is E2 — the files exist, the
//! `.dll` is a real PE, and the `.h`/`.pas` match the text contract. Actually *compiling
//! and loading* the `.h` from a C host or the `.pas` from Delphi stays BLOCKED (no
//! `cl`/`gcc`/`dcc64`); this test does NOT claim that. Building the DLL needs `cargo`, so
//! the test is Windows-gated like the other acceptance tests.
#![cfg(windows)]

use std::path::Path;

use mlc::emit::emit_artifacts;

fn fresh_out(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("mlc_emit_{tag}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn emit_artifacts_writes_the_three_consumable_files() {
    let src = include_str!("../../examples/discount.mls");
    let out = fresh_out("contract");

    let arts = emit_artifacts(src, "discount", &out).expect("emit_artifacts");

    // The three deliverables land in the output dir under the module name.
    assert_eq!(arts.dll, out.join("discount.dll"));
    assert_eq!(arts.header, out.join("discount.h"));
    assert_eq!(arts.delphi_unit, out.join("discount.pas"));
    for p in [&arts.dll, &arts.header, &arts.delphi_unit] {
        assert!(p.exists(), "missing artifact: {}", p.display());
    }

    // No build litter is left in the output dir — exactly the three files.
    let count = std::fs::read_dir(&out).unwrap().count();
    assert_eq!(count, 3, "output dir must contain exactly the 3 artifacts");

    // The .dll is a real PE image (MZ magic), not an empty/placeholder file.
    let dll = std::fs::read(&arts.dll).unwrap();
    assert!(
        dll.len() > 512,
        "dll suspiciously small: {} bytes",
        dll.len()
    );
    assert_eq!(&dll[..2], b"MZ", "dll is not a PE image");

    // .h text contract: the C-ABI signature + reserved version symbol + honest DRAFT note.
    let h = std::fs::read_to_string(&arts.header).unwrap();
    assert!(h.contains("mlx_discount"), "{h}");
    assert!(h.contains("ml_module_abi_version"), "{h}");
    assert!(
        h.contains("D14 load gate BLOCKED"),
        "header must stay honest: {h}"
    );

    // .pas text contract: matching unit name (Delphi requires file stem == unit), the
    // cdecl external import, and the same honest DRAFT note.
    let pas = std::fs::read_to_string(&arts.delphi_unit).unwrap();
    assert!(pas.contains("unit discount;"), "{pas}");
    assert!(pas.contains("function mlx_discount"), "{pas}");
    assert!(pas.contains("external ML_MODULE"), "{pas}");
    assert!(
        pas.contains("D14 load gate BLOCKED"),
        "unit must stay honest: {pas}"
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

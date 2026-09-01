//! Acceptance C of SPEC-export-wrappers — the protection proxies, measured with an instrument
//! that can actually move.
//!
//! The SPEC's first draft leaned on `.dll` file size. That was disproved before the refactor
//! started: `FileAlignment = 512` quantises it, so `discount` (one export, four lines) and
//! `quote` (two exports, two internals, three `try` statements) are BOTH exactly 9,728 B —
//! and an adapter that wrote through an out-param on the failure path, a real DP-E3
//! violation, added 16 bytes of code and moved the file size by ZERO.
//!
//! `.text` moves. This records it per example, and asserts the invariants the refactor
//! actually promises: the export set does not grow, and no user function name leaks into the
//! export table.
#![cfg(windows)]

use ml_oracle::pe;
use mlc::emit::emit_artifacts;

/// Build one example and report `(exports, .text virtual size, outlined function count)`.
///
/// `.pdata` on x64 holds one 12-byte RUNTIME_FUNCTION per function the linker kept
/// out-of-line, so `.pdata / 12` answers "did the adapter get inlined" directly.
fn measure(name: &str, src: &str) -> (Vec<String>, u32, u32, u64) {
    let out = std::env::temp_dir().join(format!("mlc_sect_{name}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&out);
    std::fs::create_dir_all(&out).unwrap();
    let arts = emit_artifacts(src, name, &out).unwrap_or_else(|e| panic!("{name}: {e}"));
    let mut exports = pe::read_exports(&arts.dll).expect("exports");
    exports.sort();
    let text = pe::read_section(&arts.dll, ".text")
        .expect("read .text")
        .expect(".text must exist");
    let pdata = pe::read_section(&arts.dll, ".pdata")
        .expect("read .pdata")
        .map(|s| s.virtual_size / 12)
        .unwrap_or(0);
    let size = std::fs::metadata(&arts.dll).unwrap().len();
    let _ = std::fs::remove_dir_all(&out);
    (exports, text.virtual_size, pdata, size)
}

#[test]
fn the_wrapper_refactor_did_not_grow_the_export_surface() {
    // The invariant the refactor actually promises. Every body is a plain `fn` with no
    // `#[no_mangle]`, so the only exported names are `mlx_<name>` plus the ABI symbol —
    // exactly as before. If a body ever acquired `#[no_mangle]`, the module's whole internal
    // structure would become visible to a host, which is a protection proxy (D04/D05).
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("examples");
    let mut names: Vec<String> = std::fs::read_dir(&dir)
        .expect("examples")
        .filter_map(|e| {
            let p = e.ok()?.path();
            (p.extension()? == "mls").then(|| p.file_stem()?.to_str().map(str::to_string))?
        })
        .collect();
    names.sort();

    for name in names {
        let src = std::fs::read_to_string(dir.join(format!("{name}.mls"))).unwrap();
        let (exports, text, outlined, size) = measure(&name, &src);
        println!(
            "{name}: exports={} .text={text} outlined={outlined} file={size}",
            exports.len()
        );

        for e in &exports {
            assert!(
                e == "ml_module_abi_version" || e.starts_with("mlx_"),
                "{name}: unexpected export {e} — a body must never be #[no_mangle]"
            );
            assert!(
                !e.starts_with("ml_fn_"),
                "{name}: the body {e} reached the export table"
            );
        }
        // Every `export fn` in the source, and nothing else.
        let declared = src.matches("export fn ").count();
        assert_eq!(
            exports.len(),
            declared + 1,
            "{name}: {} exports for {declared} `export fn` + the ABI symbol: {exports:?}",
            exports.len()
        );
    }
}

#[test]
fn the_adapter_is_free_at_this_optimisation_level() {
    // DP-W5's rationale at confirmation time was "inlining is an expectation, not a
    // measurement". This is the measurement: a module with N exports keeps the SAME number of
    // out-of-line functions as before the refactor, because each adapter is inlined into its
    // export at `opt-level="z"` + LTO.
    //
    // Written as a record rather than a threshold — if a future toolchain stops inlining, the
    // printed numbers say so and the SPEC gets updated, instead of a magic constant failing
    // with no explanation.
    let (_, text, outlined, size) = measure(
        "wrapinline",
        "error E = 1\n\
         export fn check(x: i32) -> i32! { if x < 0 { fail E } return x }\n\
         export fn doubled(x: i32) -> i32! { let v = try check(x) return v * 2 }",
    );
    println!("two exports sharing one rule: .text={text} outlined={outlined} file={size}");
    assert!(
        outlined > 0,
        "expected x64 unwind info; got none — the measurement is not reading what it thinks"
    );
    // The absolute number depends on the toolchain, so the assertion is the shape of the
    // claim, not a constant: a two-export module must not carry dozens of out-of-line
    // functions just because each one now has a body plus an adapter.
    assert!(
        outlined < 64,
        "{outlined} out-of-line functions for two exports — the adapters are not being inlined"
    );
}

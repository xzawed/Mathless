//! `mlc build` must not leak the generated crate's build chatter.
//!
//! The generated Rust lives in a temp directory that is deleted afterwards, so a rustc
//! diagnostic quoting `src/lib.rs:10` points at a file the user never wrote and cannot open.
//! Worse, on success it was pure noise: a `Compiling <crate> (C:\Users\...\Temp\mlc-build-…)`
//! line plus any warning about generated code. Measured with a two-value-return attempt,
//! where a declared-but-unwritable parameter produced:
//!
//! ```text
//! warning: unused variable: `out_tier`
//!   --> src\lib.rs:10:56
//! ```
//!
//! So cargo's output is captured, not inherited: dropped on success, and folded into the
//! error on failure — where the user does need it, and where it should be reachable as data
//! rather than as something that already scrolled past.

use mlc::codegen::build_cdylib;
use mlc::compile_to_rust;

fn workdir(tag: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("mlc_gco_{tag}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

#[test]
fn a_failing_generated_crate_reports_what_rustc_said() {
    // Reaching this from source is deliberately hard now — the frontend rejects the shapes
    // that used to produce invalid Rust (#67 closed the last one). So drive codegen directly,
    // which is also the only honest way to test the backend's own failure path.
    let d = workdir("fail");
    // The fingerprint export is present because `build_cdylib` refuses a source that does
    // not carry `ml_iface_hash_<crate>` (SPEC-qualified-iface-hash) — it is the join where
    // the name baked into the source is checked against the name the artifact is built
    // under. This source stands in for emitter output, so it carries what the emitter always
    // emits; what is broken about it is still the `-> i32` returning a `&str`.
    let err = build_cdylib(
        "#![no_std]\n#[no_mangle]\npub extern \"C\" fn ml_iface_hash_broken() -> u64 { 0 }\n\
         pub fn broken() -> i32 { \"not an i32\" }\n",
        "broken",
        &d,
    )
    .expect_err("invalid generated Rust must fail the build");

    let msg = format!("{err}");
    assert!(
        msg.contains("cargo build of generated crate failed"),
        "keep the summary line: {msg}"
    );
    // The point of the change: rustc's actual complaint travels WITH the error instead of
    // having been printed to the console by an inherited stdio.
    assert!(
        msg.contains("E0308") || msg.to_lowercase().contains("mismatched types"),
        "the compiler's diagnostic must be carried in the error, not just printed: {msg}"
    );
    // And it must say whose line numbers those are, since the file is generated and gone.
    // Match the specific phrase, not the bare word "generated" — the summary line already
    // contains "generated crate", so a loose check here would pass without the warning.
    assert!(
        msg.contains("GENERATED Rust"),
        "the message must warn that the positions refer to generated Rust: {msg}"
    );
    // The positions being warned about must actually be there, or the warning is decoration.
    assert!(
        msg.contains("src/lib.rs") || msg.contains("src\\lib.rs"),
        "the carried diagnostic should include the generated file's positions: {msg}"
    );

    let _ = std::fs::remove_dir_all(&d);
}

/// The module's name reaches `build_cdylib` twice — baked into the source by `emit`, and
/// passed as the crate to build — and separate arguments are how the two can disagree.
///
/// This is not hypothetical: `compile_to_rust` has no file behind it, so it emits
/// `ml_iface_hash_unnamed`, and two oracle tests were passing exactly that to `build_cdylib`
/// under a real module name. Before this refusal existed the result was a DLL exporting a
/// fingerprint that named a module which does not exist — resolved by nobody, and failing
/// far from the cause: `GetProcAddress` returning NULL in a dynamic host, `LNK2019` in a
/// linked one.
///
/// It runs no cargo: the refusal is a string comparison made before the crate is written, so
/// this costs nothing and can sit in the ungated part of the file.
#[test]
fn build_cdylib_refuses_a_source_whose_fingerprint_names_another_module() {
    let d = workdir("mismatch");
    let rust = compile_to_rust("export fn f(x: f64) -> f64 { return x }").expect("compile");
    let err = build_cdylib(&rust, "discount", &d)
        .expect_err("a source emitting ml_iface_hash_unnamed must not be buildable as `discount`");
    let msg = format!("{err}");
    assert!(
        msg.contains("ml_iface_hash_discount") && msg.contains("compile_to_rust_named"),
        "the refusal must name the symbol it wanted and the entry point that produces it: \
         {msg}"
    );
    // And the refusal happens BEFORE any crate is written, so nothing is left behind.
    assert!(
        !d.join("discount").exists(),
        "the crate directory must not be created for a source that is refused"
    );
    let _ = std::fs::remove_dir_all(&d);
}

// Windows-only, and the reason is worth stating: `build_cdylib` looks for
// `target/release/<name>.dll`, so on Linux it builds the crate fine and then fails to find
// the artifact — cargo produced `lib<name>.so`. That is not a bug to fix here. Making the
// backend platform-aware IS D22, which is deliberately unstarted (STATUS section 6), and
// starting it from a test would be the back door the Linux CI job's own comment warns about.
//
// This test is what made that assumption visible: every other build-touching test is already
// `#![cfg(windows)]`, so nothing had ever exercised `build_cdylib` on Linux before. The
// failure-path test above stays ungated — it never reaches the artifact lookup, and the error
// behaviour it pins is platform-independent.
#[cfg(windows)]
#[test]
fn a_successful_build_still_produces_a_dll() {
    // The capture must not break the happy path.
    let d = workdir("ok");
    let dll = build_cdylib(
        "#![no_std]\n#[panic_handler]\nfn p(_: &core::panic::PanicInfo) -> ! { loop {} }\n\
         #[no_mangle]\npub extern \"C\" fn ml_iface_hash_okcrate() -> u64 { 0 }\n\
         #[no_mangle]\npub extern \"C\" fn mlx_one() -> i32 { 1 }\n",
        "okcrate",
        &d,
    )
    .expect("a valid crate still builds");
    assert!(dll.dll.exists(), "{}", dll.dll.display());
    // The import library is the linker's second output for the same invocation, and
    // `build_cdylib` fails if it is missing — so a valid crate produces both.
    assert!(dll.import_lib.exists(), "{}", dll.import_lib.display());

    let _ = std::fs::remove_dir_all(&d);
}

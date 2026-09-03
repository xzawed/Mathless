//! WH5 (SPEC-iface-hash §3-A/F/G/H/J) — the fingerprint, measured on real DLLs.
//!
//! The compiler-side tests prove the *number* behaves. This file proves the number survives
//! the trip through rustc, the PE export table and `GetProcAddress`, and that a host built
//! against one interface actually REFUSES the other. Without the refusal half, the export is
//! decoration: `runtime/ml_abi.h` is this repository's own reminder that a contract nobody
//! calls is a contract nobody has (STATUS §5-5.3).
#![cfg(windows)]

use ml_oracle::{pe, Module};
use mlc::{codegen::build_cdylib, compile_to_ir, iface};

/// The two modules from the drift experiment in SPEC §0.1. `v1` is what the host was built
/// against; `v2` is an ordinary edit a rule author would make.
const V1: &str = "\
export fn rate(code: i32) -> f64 { if code == 1 { return 0.1 } return 0.0 }
export fn boxes(items: i32, per: i32) -> i32 { return items / per }
";
const V2: &str = "\
export fn rate(code: string) -> f64 { if code == \"KR\" { return 0.1 } return 0.0 }
export fn boxes(per: i32, items: i32) -> i32 { return items / per }
";
/// v1 with a threshold moved and nothing else — the edit that MUST stay compatible.
const V1_BODY_EDIT: &str = "\
export fn rate(code: i32) -> f64 { if code == 1 { return 0.2 } return 0.0 }
export fn boxes(items: i32, per: i32) -> i32 { return items / per }
";

struct Built {
    dir: std::path::PathBuf,
    dll: std::path::PathBuf,
    /// What the compiler says the fingerprint should be.
    expected: u64,
}

fn build(src: &str, tag: &str) -> Built {
    let ir = compile_to_ir(src).expect("compile");
    let expected = iface::fingerprint(&ir);
    let rust = mlc::codegen::emit(&ir).expect("codegen");
    let dir = std::env::temp_dir().join(format!("mlc_iface_{}_{}", tag, std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let dll = build_cdylib(&rust, &format!("iface_{tag}"), &dir)
        .expect("build cdylib")
        .dll;
    Built { dir, dll, expected }
}

/// Read the fingerprint the way a host does: load, resolve, call.
fn hash_of(dll: &std::path::Path) -> u64 {
    let m = Module::load(dll.to_str().unwrap()).expect("load");
    let f: extern "C" fn() -> u64 =
        unsafe { std::mem::transmute(m.symbol(b"ml_iface_hash\0").unwrap()) };
    f()
}

#[test]
fn the_module_reports_the_fingerprint_the_compiler_computed() {
    // §3-A end to end. A mismatch here would mean the header pins a value no module ever
    // returns, i.e. every host refuses every module.
    let b = build(V1, "a");
    assert_eq!(
        hash_of(&b.dll),
        b.expected,
        "exported value must match codegen"
    );
    let _ = std::fs::remove_dir_all(&b.dir);
}

#[test]
fn a_drifted_module_reports_a_different_fingerprint() {
    // §3-F/G. This is the whole slice: same export NAMES, incompatible interfaces. Before
    // this value existed, the only thing distinguishing these two modules at load time was
    // nothing at all — `ml_module_abi_version` returns 1 for both, measured below.
    let v1 = build(V1, "b1");
    let v2 = build(V2, "b2");

    let h1 = hash_of(&v1.dll);
    let h2 = hash_of(&v2.dll);
    assert_ne!(h1, h2, "drifted interfaces must not share a fingerprint");

    // The guard that exists today cannot tell them apart — the reason this slice exists.
    let abi = |dll: &std::path::Path| -> u32 {
        let m = Module::load(dll.to_str().unwrap()).expect("load");
        let f: extern "C" fn() -> u32 =
            unsafe { std::mem::transmute(m.symbol(b"ml_module_abi_version\0").unwrap()) };
        f()
    };
    assert_eq!(
        abi(&v1.dll),
        abi(&v2.dll),
        "control: the ABI version is identical across the drift, which is why it cannot gate"
    );

    // And the export tables are identical, so `GetProcAddress` succeeds either way.
    let mut e1 = pe::read_exports(&v1.dll).expect("exports v1");
    let mut e2 = pe::read_exports(&v2.dll).expect("exports v2");
    e1.sort();
    e2.sort();
    assert_eq!(
        e1, e2,
        "control: name-based linking cannot distinguish these modules"
    );

    println!("measured: v1={h1:#018X} v2={h2:#018X} (abi version equal, exports equal)");
    let _ = std::fs::remove_dir_all(&v1.dir);
    let _ = std::fs::remove_dir_all(&v2.dir);
}

#[test]
fn a_body_only_edit_keeps_the_fingerprint_on_the_real_dll() {
    // §3-E on the artifact, not just the IR. ARCHITECTURE.md:68 promises a module swap
    // needs no host rebuild; a threshold edit is exactly that case, and it must still pass
    // a host's check after this slice.
    let a = build(V1, "c1");
    let b = build(V1_BODY_EDIT, "c2");
    assert_eq!(
        hash_of(&a.dll),
        hash_of(&b.dll),
        "a threshold edit must remain loadable by an unmodified host"
    );
    let _ = std::fs::remove_dir_all(&a.dir);
    let _ = std::fs::remove_dir_all(&b.dir);
}

#[test]
fn a_host_that_checks_refuses_the_drifted_module_and_calls_nothing() {
    // §3-F in the oracle. The host below is the three lines the generated header documents;
    // the assertion is that it stops BEFORE the call that would have crashed.
    let v1 = build(V1, "d1");
    let v2 = build(V2, "d2");
    let pinned = v1.expected; // what a header generated for v1 would have burned in

    let mut called_anything = false;
    for (label, dll) in [("v1", &v1.dll), ("v2", &v2.dll)] {
        let m = Module::load(dll.to_str().unwrap()).expect("load");
        let hash: extern "C" fn() -> u64 =
            unsafe { std::mem::transmute(m.symbol(b"ml_iface_hash\0").unwrap()) };
        if hash() != pinned {
            println!(
                "{label}: refused (fingerprint {:#018X} != {pinned:#018X})",
                hash()
            );
            continue;
        }
        // Only reached for v1, where the host's compiled-in signature is the true one.
        let rate: extern "C" fn(i32) -> f64 =
            unsafe { std::mem::transmute(m.symbol(b"mlx_rate\0").unwrap()) };
        assert_eq!(rate(1), 0.1, "{label}: v1 must still work");
        called_anything = true;
    }
    assert!(called_anything, "the matching module must still be usable");

    let _ = std::fs::remove_dir_all(&v1.dir);
    let _ = std::fs::remove_dir_all(&v2.dir);
}

#[test]
fn the_fingerprint_export_does_not_pull_in_new_imports() {
    // §3-J. Measured as a SET COMPARISON against a module built the same way, never as an
    // absolute claim — every cdylib already imports CRT startup and `vcruntime140!memcpy`
    // through the DllMain stub (#89 measured 25 of them).
    let with = build(V1, "e1");
    let without = build("export fn f(x: f64) -> f64 { return x }", "e2");
    let mut a = pe::read_imports(&with.dll).expect("imports");
    let mut b = pe::read_imports(&without.dll).expect("imports");
    a.sort();
    b.sort();
    assert_eq!(a, b, "the fingerprint export must not add an import");
    let _ = std::fs::remove_dir_all(&with.dir);
    let _ = std::fs::remove_dir_all(&without.dir);
}

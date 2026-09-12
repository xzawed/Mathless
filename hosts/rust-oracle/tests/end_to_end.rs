//! Phase 1 vertical slice — acceptance A+B (SPEC §3): compile `examples/discount.mls`
//! with the compiler, then load the produced DLL from the oracle and call it.
//! This replaces the W1 hand-written fixture with the compiler's own output.
#![cfg(windows)]

use ml_oracle::Module;
use mlc::{codegen::build_cdylib, compile_to_rust_named};

#[test]
fn compiles_discount_mls_and_calls_it_via_oracle() {
    // A: source → emitted Rust → native DLL, produced by the compiler.
    let src = include_str!("../../../examples/discount.mls");
    // Named, because a DLL comes out of this: the fingerprint export carries the module name
    // (SPEC-qualified-iface-hash) and `build_cdylib` refuses a source whose name does not
    // match the crate it is asked to build.
    let rust = compile_to_rust_named(src, "discount").expect("compile discount.mls");
    // Isolate the build tree per test process (build_cdylib expects a unique workdir); a
    // fixed name would race two concurrent `cargo test` runs.
    let workdir = std::env::temp_dir().join(format!("mlc_e2e_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&workdir);
    let dll = build_cdylib(&rust, "discount", &workdir)
        .expect("build cdylib")
        .dll;
    assert!(dll.exists(), "dll not produced at {}", dll.display());

    // B: the oracle loads the compiler's output and the typed call works.
    let m = Module::load(dll.to_str().unwrap()).expect("load compiler output");

    let ver: extern "C" fn() -> u32 =
        unsafe { std::mem::transmute(m.symbol(b"ml_module_abi_version\0").unwrap()) };
    let discount: extern "C" fn(f64, bool) -> f64 =
        unsafe { std::mem::transmute(m.symbol(b"mlx_discount\0").unwrap()) };

    assert_eq!(ver(), mlc::ML_MODULE_ABI_VERSION, "abi version");
    assert_eq!(discount(100.0, true), 90.0, "vip discount");
    assert_eq!(discount(100.0, false), 100.0, "non-vip");

    // Release the loaded DLL (Windows locks it) before removing the isolated build tree.
    drop(m);
    let _ = std::fs::remove_dir_all(&workdir);
}

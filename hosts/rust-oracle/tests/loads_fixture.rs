//! W1 acceptance (SPEC §3-B): the oracle loads a module DLL and calls its exports.
//! For W1 the module is a hand-written fixture (`examples/fixture`); at W5 it is
//! replaced by the compiler's own output.
#![cfg(windows)]

use std::path::PathBuf;
use std::process::Command;

use ml_oracle::Module;

/// Build the fixture in the SAME profile as this test and return its DLL path.
fn fixture_dll() -> PathBuf {
    // The test exe lives in target/<profile>/deps/; the cdylib is in target/<profile>/.
    let exe = std::env::current_exe().expect("current_exe");
    let profile_dir = exe
        .parent()
        .and_then(|p| p.parent())
        .expect("target/<profile> dir");
    let is_release = profile_dir
        .file_name()
        .map(|n| n == "release")
        .unwrap_or(false);

    // Match the fixture build profile to this test's profile so the path lines up
    // (guards `cargo test --release`; guarantees the fixture exists when run alone).
    let mut args = vec!["build", "-p", "discount_fixture"];
    if is_release {
        args.push("--release");
    }
    let status = Command::new(env!("CARGO"))
        .args(&args)
        .status()
        .expect("spawn cargo build for fixture");
    assert!(status.success(), "fixture build failed");

    profile_dir.join("discount_fixture.dll")
}

#[test]
fn oracle_loads_fixture_and_calls_discount() {
    let dll = fixture_dll();
    assert!(dll.exists(), "fixture dll not found at {}", dll.display());

    let m = Module::load(dll.to_str().unwrap()).expect("load fixture");

    // Reserved ABI-version symbol (D18).
    let ver_p = m
        .symbol(b"ml_module_abi_version\0")
        .expect("ml_module_abi_version");
    let ver: extern "C" fn() -> u32 = unsafe { std::mem::transmute(ver_p) };
    assert_eq!(ver(), 1, "abi version");

    // User export (mlx_ prefix).
    let fn_p = m.symbol(b"mlx_discount\0").expect("mlx_discount");
    let discount: extern "C" fn(f64, bool) -> f64 = unsafe { std::mem::transmute(fn_p) };
    assert_eq!(discount(100.0, true), 90.0, "vip discount");
    assert_eq!(discount(100.0, false), 100.0, "non-vip");
}

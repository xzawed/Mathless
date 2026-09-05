//! Integer-type (`i32`) slice (SPEC docs/slices/SPEC-i32.md): signed 32-bit integers.
//! Integer literal (no `.`) is `i32`; decimal literal is `f64`; no implicit mixing. `i32 /`
//! was out of scope here (DP-I3) and the division slice added it — see
//! `compiler/tests/i32_division.rs`. The E2 load/call proof is in
//! `hosts/rust-oracle/tests/i32_type.rs`.

use mlc::{compile_to_ir, compile_to_rust};

#[test]
fn i32_function_lowers_to_i32_rust() {
    let rust =
        compile_to_rust("export fn add(a: i32, b: i32) -> i32 { let sum = a + b return sum }")
            .expect("compile add");
    assert!(
        rust.contains(r#"pub extern "C" fn mlx_add(a: i32, b: i32) -> i32"#),
        "i32 signature:\n{rust}"
    );
    // `(a).wrapping_add(b)`, not `(a + b)`: i32 arithmetic wraps by the language's rule
    // (DP-I4), and the plain operator only wraps while `overflow-checks` happens to be off.
    // See `i32_arithmetic_wraps_in_the_emitted_code_not_in_a_build_flag` for the measurement
    // that changed this line.
    assert!(
        rust.contains("let sum = (a).wrapping_add(b);"),
        "i32 arithmetic:\n{rust}"
    );
}

#[test]
fn an_integer_literal_is_i32_and_a_decimal_is_f64() {
    let ri = compile_to_rust("export fn one() -> i32 { return 1 }").expect("compile i32");
    assert!(ri.contains("-> i32"), "{ri}");
    assert!(ri.contains("return 1i32;"), "integer literal → i32:\n{ri}");

    let rf = compile_to_rust("export fn p() -> f64 { return 0.9 }").expect("compile f64");
    assert!(rf.contains("-> f64"), "{rf}");
    assert!(rf.contains("0.9f64"), "decimal literal → f64:\n{rf}");
}

#[test]
fn mixing_i32_and_f64_is_rejected() {
    let err = compile_to_ir("export fn f(a: i32) -> i32 { return a + 1.0 }").unwrap_err();
    assert!(format!("{err:?}").to_lowercase().contains("i32"), "{err:?}");
}

#[test]
fn i32_division_is_no_longer_rejected() {
    // DP-I3 deferred `i32 /` out of the i32 slice, and this test used to pin that rejection.
    // The division slice supersedes it (SPEC-i32-division): `/` and `%` are now total on i32.
    // Kept as a positive test rather than deleted, so the supersession is visible from here —
    // the negative form would otherwise just vanish from history.
    compile_to_rust("export fn f(a: i32) -> i32 { return a / 2 }").expect("i32 / now compiles");
}

#[test]
fn returning_a_decimal_from_an_i32_function_is_rejected() {
    let err = compile_to_ir("export fn f() -> i32 { return 1.0 }").unwrap_err();
    assert!(
        format!("{err:?}").to_lowercase().contains("mismatch"),
        "{err:?}"
    );
}

#[test]
fn the_f64_path_is_unchanged() {
    let rust =
        compile_to_rust("export fn g(x: f64) -> f64 { return x * 0.9 }").expect("compile f64");
    assert!(rust.contains(r#"-> f64"#), "{rust}");
    assert!(
        !rust.contains("i32"),
        "no i32 leaks into an f64 fn:\n{rust}"
    );
}

#[test]
fn i32_maps_to_int32_t_and_integer_in_the_bindings() {
    let ir = compile_to_ir("export fn add(a: i32, b: i32) -> i32 { let s = a + b return s }")
        .expect("compile");
    let h = mlc::header::emit_c_header(&ir, "add");
    assert!(h.contains("int32_t mlx_add(int32_t a, int32_t b);"), "{h}");
    let p = mlc::header::emit_delphi_unit(&ir, "add", "add");
    assert!(
        p.contains("a: Integer; b: Integer") && p.contains("): Integer;"),
        "{p}"
    );
}

/// i32 arithmetic wraps because the emitted code says so, not because a default happens to be
/// off.
///
/// `ir.rs` states the contract — "Overflow wraps, same rule as the rest of i32 arithmetic
/// (DP-I4), so `-i32::MIN == i32::MIN`" — and codegen emitted Rust's plain `+ - *` and unary
/// `-`, which wrap only while `overflow-checks` is off. The generated `[profile.release]`
/// pinned `panic`, `strip`, `lto` and `opt-level`, and not that one.
///
/// Measured, same `.mls` and same compiler, one environment variable apart. A C host loading
/// the built `.dll` and calling `bump(2147483647)`:
///
/// | build | result |
/// |---|---|
/// | `mlc build` | `bump(2147483647) = -2147483648` — the documented wrap |
/// | `CARGO_PROFILE_RELEASE_OVERFLOW_CHECKS=true mlc build` | **timed out at 8s**; the host thread never returned |
///
/// The hang is the documented `ml_panic` behaviour (STATUS §5-4): in `no_std` the panic
/// handler IS the panic runtime, `panic = "abort"` only drops unwinding tables, so a panic
/// lands in `loop {}` and spins with the process still up. Reachable from an environment
/// variable that appears nowhere in the source.
///
/// The assertions below are on the emitted text rather than on a call, deliberately: the
/// failure mode is a hang, and a test that reproduces it would hang CI rather than fail it.
/// The runtime column above is the measurement; this is the regression guard.
#[test]
fn i32_arithmetic_wraps_in_the_emitted_code_not_in_a_build_flag() {
    let rust = compile_to_rust(
        "export fn bump(x: i32) -> i32 { return x + 1 }\n\
         export fn drop1(x: i32) -> i32 { return x - 1 }\n\
         export fn twice(x: i32) -> i32 { return x * 2 }\n\
         export fn neg(x: i32) -> i32 { return -x }",
    )
    .expect("compile");
    for method in [
        "wrapping_add",
        "wrapping_sub",
        "wrapping_mul",
        "wrapping_neg",
    ] {
        assert!(
            rust.contains(method),
            "i32 arithmetic must wrap in the emitted code:\n{rust}"
        );
    }

    // f64 has no wrapping_* and needs none — this must not become "every operator gets a
    // method call".
    let f = compile_to_rust("export fn add(a: f64, b: f64) -> f64 { return a + b }")
        .expect("compile f64");
    assert!(
        !f.contains("wrapping_"),
        "f64 arithmetic must stay a plain operator:\n{f}"
    );
}

/// The generated profile pins the flag too — belt and braces for the emitted HELPERS, which
/// are hand-written Rust (`ml_slen`'s `n += 1`, `ml_wint`'s index arithmetic) and are not
/// covered by the lowering above.
#[test]
fn the_generated_profile_pins_overflow_checks() {
    let manifest = mlc::codegen::CARGO_TOML_PROFILE;
    assert!(
        manifest.contains("overflow-checks = false"),
        "the emitted [profile.release] must say what it relies on:\n{manifest}"
    );
}

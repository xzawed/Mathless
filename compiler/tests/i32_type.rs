//! Integer-type (`i32`) slice (SPEC docs/slices/SPEC-i32.md): signed 32-bit integers.
//! Integer literal (no `.`) is `i32`; decimal literal is `f64`; no implicit mixing; `i32 /`
//! is rejected this slice. The E2 load/call proof is in `hosts/rust-oracle/tests/i32_type.rs`.

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
    assert!(
        rust.contains("let sum = (a + b);"),
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
fn i32_division_is_rejected_this_slice() {
    // `i32 /` is a typecheck error for now (DP-I3). Two things the message must get right:
    // it must NOT claim the module aborts — a panic in a generated module spins forever in
    // `ml_panic`'s `loop {}` (STATUS 5-4) — and if it suggests the f64 round-trip it must
    // carry that workaround's own trap: a zero divisor silently yields i32::MAX/MIN/0
    // instead of failing (measured on a loaded module).
    let err = compile_to_ir("export fn f(a: i32) -> i32 { return a / 2 }").unwrap_err();
    let msg = format!("{err:?}").to_lowercase();
    assert!(msg.contains("division") || msg.contains("i32"), "{err:?}");
    assert!(!msg.contains("abort"), "the abort claim is false: {err:?}");
    assert!(
        msg.contains("i32::max") || msg.contains("guard"),
        "the suggested f64 workaround must carry its /0 caveat: {err:?}"
    );
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

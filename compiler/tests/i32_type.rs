//! Integer-type (`i32`) slice (SPEC docs/phase1/SPEC-i32.md): signed 32-bit integers.
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
    // i32 `/0` would abort the no_std cdylib, so `i32 /` is a typecheck error for now.
    let err = compile_to_ir("export fn f(a: i32) -> i32 { return a / 2 }").unwrap_err();
    assert!(
        format!("{err:?}").to_lowercase().contains("division")
            || format!("{err:?}").to_lowercase().contains("i32"),
        "{err:?}"
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

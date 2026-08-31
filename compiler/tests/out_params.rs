//! Explicit scalar `out` parameters (SPEC docs/slices/SPEC-out-params.md).
//!
//! DP-O1 = declared outs in declaration order, D17's implicit `out_value` always LAST.
//! DP-O2 = every normal-return path must assign every out.
//! The load/call proof is in `hosts/rust-oracle/tests/out_params.rs`; this file pins the
//! frontend and the emitted shape.

use mlc::header::{emit_c_header, emit_delphi_unit};
use mlc::{compile_to_ir, compile_to_rust};

#[test]
fn an_out_parameter_lowers_to_a_raw_pointer() {
    // The hole this slice closes: before it, a second value could be DECLARED but not
    // written — `out_tier: i32` was a by-value input that silently did nothing.
    let rust = compile_to_rust("export fn f(a: f64, out t: i32) -> f64 { t = 1 return a }")
        .expect("compile");
    assert!(rust.contains("t: *mut i32"), "{rust}");
    assert!(
        rust.contains("*t ="),
        "the assignment must write through the pointer:\n{rust}"
    );
}

#[test]
fn out_value_is_always_last() {
    // DP-O1. `-> T!` already owned the trailing slot; declared outs go before it so the rule
    // stays one sentence — "the return value comes last".
    let rust = compile_to_rust(
        "error E = 1\n\
         export fn g(a: f64, out t: i32) -> f64! { t = 2 if a < 0.0 { fail E } return a }",
    )
    .expect("compile");
    let sig = rust
        .lines()
        .find(|l| l.contains("mlx_g"))
        .expect("the export signature")
        .to_string();
    let t_at = sig.find("t: *mut i32").expect("declared out present");
    let v_at = sig
        .find("out_value: *mut f64")
        .expect("implicit out present");
    assert!(t_at < v_at, "declared out must precede out_value: {sig}");
}

#[test]
fn several_outs_keep_declaration_order() {
    let rust = compile_to_rust(
        "export fn f(out a: i32, out b: f64, c: i32) -> i32 { a = 1 b = 2.0 return c }",
    )
    .expect("compile");
    let sig = rust
        .lines()
        .find(|l| l.contains("mlx_f"))
        .unwrap()
        .to_string();
    assert!(
        sig.find("a: *mut i32").unwrap() < sig.find("b: *mut f64").unwrap(),
        "{sig}"
    );
    assert!(
        sig.find("b: *mut f64").unwrap() < sig.find("c: i32").unwrap(),
        "{sig}"
    );
}

#[test]
fn a_normal_return_path_must_assign_every_out() {
    // DP-O2. Same analysis as "all paths return"; skipping it means the host reads whatever
    // was in its stack variable.
    let err = compile_to_ir(
        "export fn f(a: f64, out t: i32) -> f64 { if a < 100.0 { return 0.0 } t = 1 return a }",
    )
    .unwrap_err();
    let msg = format!("{err:?}").to_lowercase();
    assert!(msg.contains("out"), "{err:?}");
    assert!(
        msg.contains('t'),
        "the message should name the parameter: {err:?}"
    );
}

#[test]
fn assigning_on_every_path_is_accepted() {
    compile_to_rust(
        "export fn f(a: f64, out t: i32) -> f64 { if a < 100.0 { t = 0 return 0.0 } t = 1 return a }",
    )
    .expect("every path assigns");
}

#[test]
fn a_fail_path_need_not_assign() {
    // DP-O3: on failure the host reads nothing, so requiring the write would be ceremony.
    compile_to_rust(
        "error E = 1\n\
         export fn g(a: f64, out t: i32) -> f64! { if a < 0.0 { fail E } t = 1 return a }",
    )
    .expect("a fail path is exempt");
}

#[test]
fn reading_an_out_parameter_is_rejected() {
    // DP-O4: the host may have passed uninitialised memory. Write-only.
    for src in [
        "export fn f(out t: i32) -> i32 { t = 1 return t }",
        "export fn f(a: i32, out t: i32) -> i32 { t = a + t return a }",
        "export fn f(out t: bool) -> i32 { t = true if t { return 1 } return 0 }",
    ] {
        let err = compile_to_ir(src).unwrap_err();
        let msg = format!("{err:?}").to_lowercase();
        assert!(
            msg.contains("read") || msg.contains("write-only") || msg.contains("out"),
            "{src} -> {err:?}"
        );
    }
}

#[test]
fn an_internal_function_may_not_have_out_parameters() {
    // DP-O5: a call expression has no syntax for passing a pointer. Same reason `-> T!` is
    // export-only (#67); lifting it later is additive.
    let err = compile_to_ir(
        "fn h(out t: i32) -> i32 { t = 1 return 1 }\nexport fn f() -> i32 { return 1 }",
    )
    .unwrap_err();
    let msg = format!("{err:?}").to_lowercase();
    assert!(msg.contains("out"), "{err:?}");
    assert!(msg.contains("export"), "the fix must be named: {err:?}");
}

#[test]
fn out_value_stays_reserved_in_a_fallible_function() {
    // The implicit name must not be claimable as a declared out, or the two collide.
    let err = compile_to_ir("error E = 1\nexport fn g(out out_value: f64) -> f64! { fail E }")
        .unwrap_err();
    assert!(
        format!("{err:?}").to_lowercase().contains("out_value"),
        "{err:?}"
    );
}

#[test]
fn an_out_parameter_is_still_subject_to_the_existing_name_rules() {
    // Reserved words and duplicates were already checked; `out` does not create an escape.
    for src in [
        "export fn f(out type: i32) -> i32 { type = 1 return 1 }",
        "export fn f(out t: i32, out t: i32) -> i32 { t = 1 return 1 }",
    ] {
        assert!(compile_to_ir(src).is_err(), "{src} must be rejected");
    }
}

#[test]
fn the_bindings_declare_a_pointer() {
    // Section 3-F. C gets `T*`; Delphi gets the keyword it already emits for `out_value`.
    let ir = compile_to_ir("export fn f(a: f64, out t: i32) -> f64 { t = 1 return a }")
        .expect("compile");
    let h = emit_c_header(&ir, "m");
    let pas = emit_delphi_unit(&ir, "Mlx_M", "m");
    assert!(h.contains("int32_t* t"), "{h}");
    assert!(pas.contains("out t: Integer"), "{pas}");
}

#[test]
fn the_fallible_signature_pins_dp_o1_exactly() {
    // DP-O1 is a rule about ORDER, and order is exactly what drifts silently. Pin the whole
    // line rather than its pieces: inputs, then declared outs in source order, then the
    // implicit return-value out-param last.
    let ir = compile_to_ir(
        "error E = 1\n\
         export fn g(a: f64, out t: i32) -> f64! { t = 1 if a < 0.0 { fail E } return a }",
    )
    .expect("compile");
    let h = emit_c_header(&ir, "m");
    assert!(
        h.contains("int32_t mlx_g(double a, int32_t* t, double* out_value);"),
        "{h}"
    );
    let pas = emit_delphi_unit(&ir, "Mlx_M", "m");
    assert!(
        pas.contains("function mlx_g(a: Double; out t: Integer; out out_value: Double): Integer;"),
        "{pas}"
    );
}

//! String RETURN into a caller-allocated buffer (SPEC docs/slices/SPEC-string-return.md).
//!
//! The ABI is not invented here — `HOST_ABI.md`'s Q12 sketch already fixed it:
//! `int32_t mlx_f(<params>, char* buf, int32_t cap, int32_t* needed)`, truncation is a
//! negative status, and the module never allocates. This file pins the frontend, the scope
//! rules and the emitted shape; `hosts/rust-oracle/tests/string_return.rs` measures the
//! protocol's edges against a loaded module, which is where this slice's value actually is.

use mlc::{compile_to_ir, compile_to_rust};

fn err_of(src: &str) -> String {
    compile_to_ir(src).unwrap_err().to_string().to_lowercase()
}

const CARRIER: &str = "error E_UNKNOWN = 1\n\
                       export fn name_of(scac: string) -> string! {\n\
                         if scac == \"UPSN\" { return \"UPS Ground\" }\n\
                         fail E_UNKNOWN\n\
                       }";

#[test]
fn a_function_may_return_a_string_when_it_is_fallible() {
    compile_to_rust(CARRIER).expect("the rule from SPEC section 1 must compile");
}

#[test]
fn the_return_lowers_to_the_q12_buffer_triple() {
    // DP-T1: the triple IS the return value, so it comes last (DP-O1 unchanged), and the
    // C-level return is the i32 status.
    let rust = compile_to_rust(CARRIER).expect("compile");
    assert!(
        rust.contains("scac: *const u8, ml_buf: *mut u8, ml_cap: i32, ml_needed: *mut i32"),
        "{rust}"
    );
    assert!(
        rust.contains("-> i32"),
        "the status is the C return:\n{rust}"
    );
}

#[test]
fn a_declared_out_still_comes_before_the_return_value() {
    // DP-O1 survives verbatim: declared outs in source order, then the return value — which
    // is now three slots wide instead of one.
    let rust = compile_to_rust(
        "export fn label(scac: string, out tier: i32) -> string! { tier = 1 return \"x\" }",
    )
    .expect("compile");
    assert!(
        rust.contains(
            "scac: *const u8, tier: *mut i32, ml_buf: *mut u8, ml_cap: i32, ml_needed: *mut i32"
        ),
        "{rust}"
    );
}

#[test]
fn nothing_is_allocated() {
    // Q12's whole point. The module never touches an allocator, so the generated crate stays
    // `no_std` with zero dependencies.
    let rust = compile_to_rust(CARRIER).expect("compile");
    assert!(!rust.contains("alloc"), "{rust}");
    assert!(!rust.contains("String"), "{rust}");
    assert!(!rust.contains("Vec"), "{rust}");
}

#[test]
fn the_copy_does_not_call_the_c_runtime_by_name() {
    // Same stance as the string-input slice: no `strcpy`/`memcpy` written by us. (The import
    // set is measured against a control module in the oracle — rustc may still lower a byte
    // loop to memcpy, which is exactly why that measurement is a comparison, not an absolute.)
    let rust = compile_to_rust(CARRIER).expect("compile");
    for banned in ["strcpy", "strncpy", "memcpy", "strlen"] {
        assert!(!rust.contains(banned), "{banned} in:\n{rust}");
    }
}

#[test]
fn a_bare_string_return_is_still_rejected_and_names_the_fix() {
    // DP-T1: `!` is the surface's only mark for "this returns a status you must check", and
    // truncation makes every string return fallible. The message must teach, not just refuse.
    let msg = err_of("export fn f(s: string) -> string { return s }");
    assert!(msg.contains("string!"), "must name the fix: {msg}");
}

#[test]
fn an_internal_function_may_not_return_a_string() {
    // Same reason `-> T!` is export-only (DP-O5): D17 is a host-boundary convention and
    // there is no internal calling convention for the buffer triple.
    let msg = err_of(
        "fn helper(s: string) -> string! { return s }\n\
         export fn f(s: string) -> bool { return true }",
    );
    assert!(msg.contains("export"), "{msg}");
}

#[test]
fn only_a_literal_or_a_parameter_may_be_returned() {
    // DP-T5. Every returned byte must be an ASCII source literal or a byte the host handed
    // in — that is what keeps DP-S2 ("opaque bytes") literally true for this slice.
    compile_to_rust("export fn f(s: string) -> string! { return \"KR\" }").expect("literal");
    compile_to_rust("export fn f(s: string) -> string! { return s }").expect("parameter echo");

    // SPEC-string-concat opened the built form: a concatenation, and `i32 as string`. Every
    // OTHER way of producing a string is still rejected, and a type mismatch is still a type
    // mismatch — `return n` where `n` is an i32 does not quietly format anything (DP-K1).
    compile_to_rust("export fn f(a: string, b: string) -> string! { return a + b }")
        .expect("concatenation, since SPEC-string-concat");
    compile_to_rust("export fn f(n: i32) -> string! { return n as string }").expect("digits");

    for src in [
        "export fn f(n: i32) -> string! { return n }",
        "export fn f(b: bool) -> string! { return b }",
        // Still no implicit conversion, which is the whole of DP-K1.
        "export fn f(a: string, n: i32) -> string! { return a + n }",
    ] {
        assert!(compile_to_ir(src).is_err(), "{src} must be rejected");
    }
}

#[test]
fn the_synthesized_names_cannot_be_shadowed() {
    // `ml_buf`/`ml_cap`/`ml_needed` are injected into the same scope as the user's own names.
    // The `ml_` prefix is already reserved for exactly this reason (#85), so this test is a
    // guard on that guarantee rather than a new rule — if the prefix rule were ever relaxed,
    // a parameter named `ml_cap` would silently shadow the capacity and every call would
    // truncate or overrun. Same shape as the `__d` defect.
    for name in ["ml_buf", "ml_cap", "ml_needed"] {
        let msg = err_of(&format!(
            "export fn f({name}: i32, s: string) -> string! {{ return s }}"
        ));
        assert!(msg.contains("ml_"), "{name} must be rejected: {msg}");
    }
}

#[test]
fn the_c_header_declares_the_triple_and_the_truncation_constant() {
    // Section 3-F. DP-T6: the truncation status sits OUTSIDE the module's error namespace —
    // it is a runtime-wide band with the same value everywhere — and is `#ifndef`-guarded so
    // two generated headers can be included in one translation unit. The error namespace
    // takes the opposite rule and is deliberately NOT guarded (Q14 / DP-Q3).
    let ir = compile_to_ir(CARRIER).expect("compile");
    let h = mlc::header::emit_c_header(&ir, "carrier");
    assert!(
        h.contains("int32_t mlx_name_of(const char* scac, char* ml_buf, int32_t ml_cap, int32_t* ml_needed);"),
        "{h}"
    );
    assert!(h.contains("#ifndef ML_ST_INSUFFICIENT_BUFFER"), "{h}");
    assert!(h.contains("#define ML_ST_INSUFFICIENT_BUFFER (-1)"), "{h}");
    // Q14 renamed error constants to ML_<MODULE>_ERR_<NAME>, which quietly neutered this
    // assertion: a bare `ML_ERR_TRUNCATED` can no longer be emitted by any path, so the
    // check could never fail again. Written against the shape the emitter actually uses, it
    // has teeth once more — `carrier` is the module here.
    assert!(
        !h.contains("ML_CARRIER_ERR_TRUNCATED") && !h.contains("ML_ERR_TRUNCATED"),
        "the truncation status must not live in the module's error namespace:\n{h}"
    );
}

#[test]
fn the_delphi_unit_uses_pbyte_and_ships_no_executable_code() {
    // DP-T3: `PByte` makes both UnicodeString misspellings compile ERRORS in Delphi, instead
    // of the silent wrong answer the string-input slice could only warn about.
    // DP-T3b: no `dcc64` exists here, so no Pascal statement may ship — a logic error in a
    // generated wrapper would be worse than a comment, because hosts would trust it.
    let ir = compile_to_ir(CARRIER).expect("compile");
    let pas = mlc::header::emit_delphi_unit(&ir, "Mlx_Carrier", "carrier");
    assert!(pas.contains("ml_buf: PByte"), "{pas}");
    assert!(
        !pas.contains("out ml_buf"),
        "`out ml_buf` would be PAnsiChar* — the module would overwrite the host's pointer:\n{pas}"
    );
    assert!(pas.contains("ml_cap: Integer"), "{pas}");
    assert!(pas.contains("out ml_needed: Integer"), "{pas}");
    // No executable Pascal: the unit has exactly one `implementation` and nothing after it
    // but `end.`.
    let body = pas.split("implementation").nth(1).unwrap_or("");
    assert_eq!(
        body.split_whitespace().collect::<Vec<_>>(),
        vec!["end."],
        "the implementation section must stay empty (DP-T3b):\n{body}"
    );
}

/// The Delphi unit has to name the truncation status too.
///
/// Measured, for `export fn label(a: string, b: string) -> string!`:
///
/// - the `.h` defines `ML_ST_INSUFFICIENT_BUFFER (-1)` and explains the Q12 protocol above it;
/// - the `.pas` declared the same function with the same buffer triple and said **nothing** —
///   no constant, no note.
///
/// So a Delphi host had to retype `-1`, which is exactly what `hosts/c-host/host.c` refuses to
/// do for the C side ("the error constant comes from the header too … not from a number
/// retyped here"). The error codes already reach the unit as `ML_<MODULE>_ERR_<NAME>`; the one
/// status a string-returning call can ALWAYS produce did not.
///
/// DP-T3b still holds: this is a `const` and a comment, not a statement. No `dcc64` exists
/// here, so no Pascal code ships.
#[test]
fn the_delphi_unit_declares_the_truncation_status_like_the_header_does() {
    let ir = compile_to_ir(CARRIER).expect("compile");
    let pas = mlc::header::emit_delphi_unit(&ir, "Mlx_Carrier", "carrier");
    assert!(
        pas.contains("ML_ST_INSUFFICIENT_BUFFER = -1;"),
        "a Delphi host needs the name, not the number:\n{pas}"
    );
    assert!(
        pas.to_lowercase().contains("truncation"),
        "and the rule that makes it a failure rather than a short success:\n{pas}"
    );
    // Outside the module's error namespace, exactly as in the header (DP-T6).
    assert!(
        !pas.contains("ML_CARRIER_ERR_TRUNCATED"),
        "the truncation status must not join the module's error namespace:\n{pas}"
    );

    // ...and only for a module that can produce it. A module with no string return must not
    // carry a constant for a status none of its functions can return.
    let plain = compile_to_ir("export fn f(x: f64) -> f64 { return x }").expect("compile");
    let plain_pas = mlc::header::emit_delphi_unit(&plain, "Mlx_M", "m");
    assert!(
        !plain_pas.contains("ML_ST_INSUFFICIENT_BUFFER"),
        "an unrelated module must not carry it:\n{plain_pas}"
    );
}

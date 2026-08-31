//! String INPUT parameters and comparison (SPEC docs/slices/SPEC-string-input.md).
//!
//! DP-S1 = NUL-terminated `const char*`. DP-S2 = opaque bytes; `==` is byte equality.
//! The values are measured against a loaded module in `hosts/rust-oracle/tests/string_input.rs`;
//! this file pins the frontend, the scope restriction, and the emitted shape.
//!
//! The scope restriction is the load-bearing part. A string can only be a PARAMETER — never a
//! return type, a local, or an `out`. All three would ask where the bytes live, and returning
//! one is the first user of the Q12 protocol, which is a separate slice (SPEC section 5.1).

use mlc::{compile_to_ir, compile_to_rust};

fn err_of(src: &str) -> String {
    format!("{:?}", compile_to_ir(src).unwrap_err()).to_lowercase()
}

#[test]
fn a_string_parameter_can_be_compared_against_a_literal() {
    // The measured business rule: the host supplies a code and the module branches on it.
    compile_to_rust(
        "export fn vat_rate(country: string) -> f64 {\n\
           if country == \"KR\" { return 0.1 }\n\
           if country == \"JP\" { return 0.08 }\n\
           return 0.0\n\
         }",
    )
    .expect("the rule from SPEC section 1.1 must compile");
}

#[test]
fn two_string_parameters_can_be_compared() {
    compile_to_rust("export fn same(a: string, b: string) -> bool { return a == b }")
        .expect("string == string");
    compile_to_rust("export fn diff(a: string, b: string) -> bool { return a != b }")
        .expect("string != string");
}

#[test]
fn a_string_lowers_to_a_borrowed_byte_pointer() {
    // DP-S1. The parameter is borrowed for the call (D16 rule 1) — no allocation anywhere.
    let rust =
        compile_to_rust("export fn f(s: string) -> bool { return s == \"x\" }").expect("compile");
    assert!(rust.contains("s: *const u8"), "{rust}");
    assert!(
        !rust.contains("String"),
        "nothing owned may appear:\n{rust}"
    );
    assert!(!rust.contains("alloc"), "no allocation:\n{rust}");
}

#[test]
fn a_literal_lowers_to_a_static_nul_terminated_array() {
    let rust =
        compile_to_rust("export fn f(s: string) -> bool { return s == \"KR\" }").expect("compile");
    // `b"KR\0"` — static storage, NUL-terminated so the comparison agrees with the C side.
    assert!(rust.contains(r#"b"KR\0""#), "{rust}");
}

#[test]
fn the_comparison_does_not_call_the_c_runtime() {
    // SPEC section 2.3: calling `strcmp` would add an import, and the import set is a
    // protection proxy (D04/D05). A byte loop costs nothing at the boundary.
    let rust =
        compile_to_rust("export fn f(s: string) -> bool { return s == \"x\" }").expect("compile");
    assert!(!rust.contains("strcmp"), "{rust}");
    assert!(!rust.contains("memcmp"), "{rust}");
    assert!(
        rust.contains("ml_streq"),
        "the emitted helper is used:\n{rust}"
    );
}

#[test]
fn the_helper_is_only_emitted_when_a_comparison_exists() {
    let plain = compile_to_rust("export fn f(s: string) -> bool { return true }")
        .expect("a string parameter that is never compared still compiles");
    assert!(
        !plain.contains("ml_streq"),
        "unused helper emitted:\n{plain}"
    );
}

#[test]
fn a_string_may_only_be_a_parameter() {
    // SPEC section 2.5. Each of these asks where the bytes live, and a return is the first
    // user of the Q12 protocol — a separate slice.
    for src in [
        "export fn f(s: string) -> string { return s }",
        "export fn f(s: string) -> bool { let t = s return t == s }",
        "export fn f(s: string, out t: string) -> bool { return true }",
    ] {
        let msg = err_of(src);
        assert!(
            msg.contains("string"),
            "{src} must be rejected naming the type: {msg}"
        );
    }
    // An INTERNAL function may take one. SPEC section 2.5 restricts the POSITION (parameter),
    // not the visibility — a borrowed pointer passed to a helper raises no new question.
    compile_to_rust(
        "fn is_kr(s: string) -> bool { return s == \"KR\" }\n\
         export fn f(c: string) -> bool { return is_kr(c) }",
    )
    .expect("an internal helper may take a string parameter");
}

#[test]
fn only_equality_is_defined_on_strings() {
    // DP-S3: the three measured rules needed `==` and nothing else.
    for src in [
        "export fn f(a: string, b: string) -> bool { return a < b }",
        "export fn f(a: string, b: string) -> bool { return a > b }",
        "export fn f(a: string) -> f64 { return a + a }",
    ] {
        assert!(compile_to_ir(src).is_err(), "{src} must be rejected");
    }
}

#[test]
fn a_string_may_not_be_compared_with_another_type() {
    // DP-I2's stance is unchanged: no implicit anything.
    for src in [
        "export fn f(a: string, b: i32) -> bool { return a == b }",
        "export fn f(a: string) -> bool { return a == 1 }",
        "export fn f(a: string) -> bool { return a == true }",
        "export fn f(a: i32) -> bool { return a == \"x\" }",
    ] {
        assert!(compile_to_ir(src).is_err(), "{src} must be rejected");
    }
}

#[test]
fn a_literal_must_be_plain_ascii_and_unescaped() {
    // DP-S4 keeps STATUS section 6's existing rule narrow rather than widening it; escapes are
    // rejected loudly instead of silently meaning something else.
    let non_ascii = err_of("export fn f(s: string) -> bool { return s == \"한글\" }");
    assert!(
        non_ascii.contains("ascii"),
        "the reason must be named: {non_ascii}"
    );
    let escaped = err_of("export fn f(s: string) -> bool { return s == \"a\\nb\" }");
    assert!(
        escaped.contains("escape") || escaped.contains("\\"),
        "{escaped}"
    );
    let unterminated = format!(
        "{:?}",
        mlc::parse("export fn f(s: string) -> bool { return s == \"oops }").unwrap_err()
    )
    .to_lowercase();
    assert!(
        unterminated.contains("string") || unterminated.contains("unterminated"),
        "{unterminated}"
    );
}

#[test]
fn the_bindings_say_const_char_and_pansichar() {
    // Section 3-F. The Delphi side carries the warning too — a UnicodeString passed here
    // compiles, does not crash, and matches nothing (SPEC section 5.2).
    let ir = compile_to_ir("export fn vat_rate(country: string) -> f64 { return 0.0 }")
        .expect("compile");
    let h = mlc::header::emit_c_header(&ir, "vat");
    let pas = mlc::header::emit_delphi_unit(&ir, "Mlx_Vat", "vat");
    assert!(h.contains("const char* country"), "{h}");
    assert!(pas.contains("country: PAnsiChar"), "{pas}");
    assert!(
        pas.to_lowercase().contains("unicodestring"),
        "the Delphi unit must warn about UnicodeString:\n{pas}"
    );
}

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

    // The gate learned to look inside `Concat` (see `compares_strings`), and a widened gate
    // can over-emit as easily as a narrow one under-emits. A module that concatenates but
    // never compares must still not carry the helper — otherwise every string-building module
    // grows dead code, and dead code in the module is dead code in the shipped `.dll`.
    let concat = compile_to_rust("export fn f(a: string, b: string) -> string! { return a + b }")
        .expect("a concatenation without a comparison compiles");
    assert!(
        !concat.contains("ml_streq"),
        "the widened gate emitted the comparison helper for a module that never compares:\n\
         {concat}"
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

/// Every `ml_*` helper the generated crate CALLS must also be DEFINED in it.
///
/// Each helper is emitted behind a gate that walks the IR looking for a reason to emit it, and
/// two of those walkers (`compares_strings`, `builds_strings`) ended their `match` with
/// `_ => false`. A variant nobody thought about therefore reads as "no reason found" instead
/// of failing to compile — and `IrExprKind::Concat` was exactly that variant for
/// `compares_strings`.
///
/// Measured before the fix, with `mlc build` on
///
///     fn score(b: bool) -> i32 { if b { return 1 }  return 0 }
///     export fn label(a: string, b: string) -> string! { return "eq=" + score(a == b) as string }
///
///     mlc: codegen error: cargo build of generated crate failed ...
///     error[E0425]: cannot find function `ml_streq` in this scope
///       --> src\lib.rs:87:32
///        |
///     87 |     __n += ml_ilen(ml_fn_score(ml_streq(a, b)));
///
/// A valid program, rejected with a rustc error inside code the user never wrote.
///
/// This test is deliberately not "does `ml_streq` appear for this one input" — that is the
/// assertion that was already there, and it passed throughout. It asserts the invariant the
/// gates exist to keep, so a future helper with a future gate is covered without editing it.
#[test]
fn every_helper_the_generated_crate_calls_is_also_defined_in_it() {
    let corpus = [
        // The reproduction: the comparison is reachable only through a concat piece.
        "fn score(b: bool) -> i32 { if b { return 1 }  return 0 }\n\
         export fn label(a: string, b: string) -> string! { return \"eq=\" + score(a == b) as string }",
        // The same shape one level deeper, through a cast as well as a call.
        "fn score(b: bool) -> i32 { if b { return 1 }  return 0 }\n\
         export fn label(a: string, b: string) -> string! { return \"n=\" + (score(a == b) + 1) as string }",
        // A rounder reachable only through a concat piece — the walker that already got
        // this right, kept here so the three stay tested as one class.
        "export fn label(x: f64) -> string! { return \"n=\" + round(x) as i32 as string }",
        // A comparison in the ordinary place, so the test still fails if emission breaks
        // outright rather than only in the nested case.
        "export fn f(s: string) -> bool { return s == \"x\" }",
    ];
    for src in corpus {
        let rust = compile_to_rust(src).expect("should compile");
        // Call sites look like `ml_name(`; definitions like `fn ml_name(`.
        let mut missing = Vec::new();
        for (i, _) in rust.match_indices("ml_") {
            let rest = &rust[i..];
            let Some(open) = rest.find('(') else { continue };
            let name = &rest[..open];
            if !name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_') {
                continue;
            }
            // `ml_fn_*` and `mlx_*` are the user's own functions, emitted with their bodies.
            if name.starts_with("ml_fn_") || rust.contains(&format!("fn {name}(")) {
                continue;
            }
            if !missing.contains(&name.to_string()) {
                missing.push(name.to_string());
            }
        }
        assert!(
            missing.is_empty(),
            "the generated crate calls {missing:?} but defines none of them — \
             a helper gate did not see the use.\nsource:\n{src}\n\ngenerated:\n{rust}"
        );
    }
}

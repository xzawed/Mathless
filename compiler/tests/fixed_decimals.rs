//! `fixed(x, places)` — the frontend and the emitted shape (`SPEC-fixed-decimals`).
//!
//! The values live in `hosts/rust-oracle/tests/fixed_decimals.rs`, because the defect this
//! slice closes is a VALUE defect: `STATUS.md` §9-53 measured the documented in-module
//! workaround answering `"1234.5"` for 1234.05, `"0.-7"` for -0.07, and `"21474836.47"` for
//! fifty million — every one of them status 0, with no warning.

use mlc::{compile_to_ir, compile_to_rust};

#[test]
fn fixed_takes_an_f64_and_a_place_count() {
    compile_to_rust("export fn amount(won: f64) -> string! { return fixed(won, 2) }")
        .expect("the rule that motivated this slice must compile");
    compile_to_rust("export fn whole(x: f64) -> string! { return fixed(x, 0) }")
        .expect("zero places is a legal request: no point at all");
    // It composes with concatenation, like any other returned string.
    compile_to_rust("export fn tagged(x: f64) -> string! { return fixed(x, 2) + \" KRW\" }")
        .expect("a formatted number is a piece like any other");
}

/// `places` is bounded, and a literal is checked where the diagnostic is good.
///
/// Ten is the bound because an f64 cannot carry a tenth decimal place meaningfully; the
/// refusal says so rather than truncating silently, which is the whole theme of this slice.
#[test]
fn a_literal_place_count_outside_the_range_is_refused_while_compiling() {
    for bad in ["10", "-1"] {
        let src = format!("export fn f(x: f64) -> string! {{ return fixed(x, {bad}) }}");
        let err = compile_to_ir(&src).unwrap_err().to_string().to_lowercase();
        assert!(
            !err.contains("unknown function"),
            "the builtin is not wired up: {err}"
        );
        assert!(
            err.contains(bad) && err.contains("places"),
            "the refusal must show the count and name what it is: {err}"
        );
    }

    // The two ends of the range are legal, and a non-literal count is a run-time question.
    compile_to_rust("export fn f(x: f64) -> string! { return fixed(x, 0) }").expect("0");
    compile_to_rust("export fn f(x: f64) -> string! { return fixed(x, 9) }").expect("9");
    compile_to_rust("export fn f(x: f64, n: i32) -> string! { return fixed(x, n) }")
        .expect("a variable count cannot be checked here, so it is checked at run time");
}

/// The shapes that are not a number and a count.
#[test]
fn fixed_refuses_the_wrong_shapes() {
    for (src, needle) in [
        (
            "export fn f(s: string) -> string! { return fixed(s, 2) }",
            "f64",
        ),
        (
            "export fn f(x: f64) -> string! { return fixed(x, 2.0) }",
            "i32",
        ),
        ("export fn f(x: f64) -> string! { return fixed(x) }", "two"),
    ] {
        let err = compile_to_ir(src)
            .expect_err("must be refused")
            .to_string()
            .to_lowercase();
        assert!(
            !err.contains("unknown function"),
            "the builtin is not wired up; this would pass on the wrong diagnostic: {err}"
        );
        assert!(
            err.contains(needle),
            "the refusal must say what it wanted: {err}"
        );
    }
}

/// A user function may not shadow it — the check `byte_len` was missing and `byte_slice`
/// learned to include from the start.
#[test]
fn a_user_function_may_not_shadow_fixed() {
    let err = compile_to_ir("export fn fixed(x: f64) -> f64 { return x }")
        .expect_err("`fixed` is a builtin")
        .to_string();
    assert!(
        err.contains("fixed") && err.contains("built-in"),
        "the refusal must name the builtin it collides with: {err}"
    );
}

/// **Acceptance I, half of it** — nothing from the C runtime is called to format a number.
///
/// The import table is measured in the oracle; this pins the generated source, because a
/// `format!` or a `{:.2}` would be visible here long before it showed up as an import.
#[test]
fn formatting_does_not_reach_for_the_runtime() {
    let rust =
        compile_to_rust("export fn f(x: f64) -> string! { return fixed(x, 2) }").expect("compile");

    // The banned idioms are looked for in CODE, with comment lines removed first.
    //
    // Not a loophole — the opposite. The emitted helpers carry their reasoning into the
    // generated crate, and one of those notes has to NAME the idiom it rejects: `ml_fixscale`
    // says it does not use `floor(s + 0.5)`, because that answers 1 for 0.49999999999999994.
    // Checking the raw text made that sentence indistinguishable from the defect it warns
    // about, and the way to keep the sentence was to read the code instead of the prose.
    let code: String = rust
        .lines()
        .filter(|l| !l.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");
    for banned in ["format!", "write!", "core::fmt", "{:.", "snprintf"] {
        assert!(
            !code.contains(banned),
            "`{banned}` appears — the digits are produced by integer arithmetic:\n{rust}"
        );
    }
    // All three, named: scale once, size from the scaled value, fill from the same size.
    // Naming each of them rather than one shared prefix is deliberate — a single needle would
    // stay green if two of the three were folded into one function that counts twice, and
    // "the two passes agree" is the property this trio exists to hold (`ml_wint`'s rule).
    for helper in ["ml_fixscale", "ml_fixlen", "ml_wfix"] {
        assert!(rust.contains(helper), "`{helper}` is missing:\n{rust}");
    }

    // …and it must reuse the rounding arithmetic rather than carry a second copy. The repo
    // measured why: `floor(x + 0.5)` answers 1 for 0.49999999999999994, so `ml_round` computes
    // the fractional part exactly instead. Two copies of that are two chances to get it wrong.
    assert!(
        rust.contains("ml_trunc_raw"),
        "the shared exact-truncation helper must be present:\n{rust}"
    );
    assert!(
        !code.contains("+ 0.5"),
        "the rejected idiom must not appear:\n{rust}"
    );

    // A module that uses neither `fixed` nor a rounder carries none of it.
    let plain = compile_to_rust("export fn f(x: f64) -> f64 { return x * 2.0 }").expect("compile");
    assert!(!plain.contains("ml_trunc_raw"), "{plain}");
    assert!(!plain.contains("ml_fixscale"), "{plain}");

    // …and a module that uses `fixed` but no rounder does not get the named rounders.
    let only_fixed =
        compile_to_rust("export fn f(x: f64) -> string! { return fixed(x, 1) }").expect("compile");
    assert!(
        !only_fixed.contains("fn ml_round"),
        "the named rounders are a separate gate (§9-47's lesson):\n{only_fixed}"
    );
}

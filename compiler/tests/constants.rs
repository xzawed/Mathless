//! Constant declarations — the frontend, the bindings and the fingerprint (`SPEC-constants`).
//!
//! The values live in `hosts/rust-oracle/tests/constants.rs`. What is here is what the SPEC
//! decides without loading a module: which form crosses the boundary (DP-K1), how it is
//! spelled there (DP-K3, DP-K7), that a constant is checked like its literal (DP-K5), and
//! what is refused (DP-K4, DP-K6).

use mlc::header::{emit_c_header, emit_delphi_unit};
use mlc::iface::{fingerprint, manifest};
use mlc::{compile_to_ir, compile_to_rust};

fn refused(src: &str) -> String {
    compile_to_ir(src)
        .expect_err("this must be refused")
        .to_string()
}

/// **Both forms compile, in every position a literal can take.**
#[test]
fn both_forms_compile_where_their_literal_would() {
    let src = "
        const RATE = 0.1
        const STRICT = true
        const LIMIT = 3
        export const PAID = 2
        export const NONE = -1
        export fn f(x: i32, b: bool, y: f64) -> f64 {
          if x == PAID && (b || STRICT) { return y * RATE }
          if x < LIMIT && x != NONE { return y }
          return 0.0
        }";
    compile_to_rust(src).expect("the forms the SPEC shows must compile");

    // Order does not matter: a constant declared after the function that uses it.
    compile_to_rust("export fn f() -> i32 { return LATE }\nexport const LATE = 7")
        .expect("constants are module-scoped like `error`, not declared-before-use");
}

/// **`const` still works as a function name** — it only means something at the top level.
#[test]
fn const_is_still_a_legal_function_name() {
    compile_to_rust("fn const() -> i32 { return 1 }\nexport fn f() -> i32 { return const() }")
        .expect("`fn const()` compiled before this slice and must keep compiling");
    compile_to_rust(
        "const K = 2\nfn const() -> i32 { return K }\nexport fn f() -> i32 { return const() }",
    )
    .expect("a constant and a function called `const` can coexist");
}

/// **DP-K1, DP-K3: only `export const` reaches the bindings, as `ML_<MODULE>_CONST_<NAME>`.**
#[test]
fn only_export_const_reaches_the_header_and_the_unit() {
    let ir = compile_to_ir(
        "const RATE = 0.1
         const LIMIT = 3
         export const PAID = 2
         export const NONE = -1
         export const LOW = -2147483648
         export const HIGH = 2147483647
         export fn f(x: i32) -> i32 { return x + PAID + NONE + LOW + HIGH + LIMIT }",
    )
    .expect("compile");

    let h = emit_c_header(&ir, "m");
    for line in [
        "#define ML_M_CONST_PAID 2\n",
        "#define ML_M_CONST_NONE (-1)\n",
        // `-2147483648` in C is the negation of a long long literal, not an int.
        "#define ML_M_CONST_LOW (-2147483647 - 1)\n",
        "#define ML_M_CONST_HIGH 2147483647\n",
    ] {
        assert!(h.contains(line), "the header lacks {line:?}:\n{h}");
    }
    let pas = emit_delphi_unit(&ir, "m");
    for line in [
        "  ML_M_CONST_PAID = 2;\n",
        "  ML_M_CONST_NONE = -1;\n",
        "  ML_M_CONST_LOW = -2147483648;\n",
        "  ML_M_CONST_HIGH = 2147483647;\n",
    ] {
        assert!(pas.contains(line), "the unit lacks {line:?}:\n{pas}");
    }
    for internal in ["RATE", "LIMIT"] {
        assert!(
            !h.contains(internal) && !pas.contains(internal),
            "the internal constant {internal} reached a binding"
        );
    }
}

/// **DP-K7: an exported value is in the fingerprint; an internal one is body.**
#[test]
fn the_fingerprint_moves_with_an_exported_value_and_only_then() {
    let fp = |src: &str| fingerprint(&compile_to_ir(src).expect("compile"));
    let body = "export fn f(x: i32) -> i32 { return x + A }";

    assert_ne!(
        fp(&format!("export const A = 2\n{body}")),
        fp(&format!("export const A = 7\n{body}")),
        "renumbering an exported constant is the drift this slice exists to catch"
    );
    assert_eq!(
        fp(&format!("const A = 2\n{body}")),
        fp(&format!("const A = 7\n{body}")),
        "an internal constant is body: a threshold edit ships by replacing the file"
    );
    // Naming a literal with an internal constant changes nothing a host can see.
    assert_eq!(
        fp("export fn f(x: i32) -> i32 { return x * 3 }"),
        fp("const K = 3\nexport fn f(x: i32) -> i32 { return x * K }"),
    );
}

/// **DP-K7: the manifest spells values in plain decimal and sorts by name bytes.**
#[test]
fn manifest_lines_are_plain_decimal_sorted_by_name() {
    let ir = compile_to_ir(
        "export const B_10 = 1
         export const B_1 = -2147483648
         export fn f() -> i32 { return B_1 + B_10 }",
    )
    .expect("compile");
    let m = manifest(&ir);
    // Sorting the rendered line would put `B_10` first: '0' sorts below '='.
    assert!(
        m.contains("const B_1=-2147483648\nconst B_10=1\n"),
        "expected name-byte order and plain decimal:\n{m}"
    );

    // A module with no exported constant gets no `const` line — its fingerprint is what it
    // was before this slice (the golden headers pin every example's value).
    let plain = manifest(&compile_to_ir("const K = 3\nexport fn f() -> i32 { return K }").unwrap());
    assert!(
        !plain.contains("const "),
        "an internal constant leaked:\n{plain}"
    );
}

/// **DP-K5: a constant is refused wherever its literal would be.**
#[test]
fn a_constant_is_checked_like_its_literal() {
    let e = refused("const ZERO = 0\nexport fn f(x: i32) -> i32 { return x / ZERO }");
    assert!(
        e.contains("dividing by the literal 0 is rejected"),
        "`x / ZERO` must meet the literal-zero check, not compile to a total division: {e}"
    );
    let e = refused("const NEG = -1\nexport fn f(xs: [i32]) -> i32! { return xs[NEG] }");
    assert!(
        e.contains("the index -1 is negative"),
        "`xs[NEG]` must meet the negative-index check: {e}"
    );
    let e = refused("const P = 12\nexport fn f(x: f64) -> string! { return fixed(x, P) }");
    assert!(
        e.contains("places") && e.contains("0..="),
        "`fixed(x, P)` must meet the places range check: {e}"
    );
}

/// **DP-K2, DP-K4: what a constant's value may be.**
#[test]
fn values_that_are_refused() {
    for (src, needle) in [
        (
            "export const R = 0.5\nexport fn f() -> i32 { return 1 }",
            "export const",
        ),
        (
            "export const T = true\nexport fn f() -> i32 { return 1 }",
            "export const",
        ),
        (
            "const A = 1 + 1\nexport fn f() -> i32 { return A }",
            "single literal",
        ),
        (
            "const A = 1\nconst B = A\nexport fn f() -> i32 { return B }",
            "single literal",
        ),
        (
            "const B = -true\nexport fn f() -> i32 { return 1 }",
            "single literal",
        ),
        // Unused, so only the DECLARATION can be what refuses them — a use would meet the
        // literal range check inside the function and pass this needle for the wrong reason.
        (
            "const C = 2147483648\nexport fn f() -> i32 { return 1 }",
            "does not fit in i32",
        ),
        (
            "const C = -2147483649\nexport fn f() -> i32 { return 1 }",
            "does not fit in i32",
        ),
    ] {
        let e = refused(src);
        assert!(
            e.contains(needle),
            "{src:?} was refused, but not for its reason: {e}"
        );
    }
    // An exported constant must be i32, and the refusal says which types are allowed where.
    let e = refused("export const R = 0.5\nexport fn f() -> i32 { return 1 }");
    assert!(e.contains("i32"), "{e}");
}

/// **DP-K6: one name, one meaning.**
#[test]
fn names_that_are_refused() {
    for (src, needle) in [
        (
            "const A = 1\nconst a = 2\nexport fn f() -> i32 { return A }",
            "case-insensitively",
        ),
        (
            "error E_X = 1\nconst e_x = 2\nexport fn f() -> i32 { return e_x }",
            "has the name of error",
        ),
        (
            "const PAID = 2\nexport fn f(PAID: i32) -> i32 { return PAID }",
            "hides the constant",
        ),
        (
            "const PAID = 2\nexport fn f(x: i32) -> i32 { let PAID = x  return PAID }",
            "hides the constant",
        ),
        (
            "const PAID = 2\nexport fn f(x: i32) -> i32 { let mut y = x  PAID = 3  return y }",
            "cannot be assigned",
        ),
        (
            "const result = 1\nexport fn f() -> i32 { return 1 }",
            "`result`",
        ),
    ] {
        let e = refused(src);
        assert!(
            e.contains(needle),
            "{src:?} was refused, but not for its reason: {e}"
        );
    }
}

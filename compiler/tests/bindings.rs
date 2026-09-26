//! W7 acceptance (WBS W7): generate a C header + Delphi import unit that match the
//! module's D18 ABI. These tests check the generated *text*. The C header is separately
//! compiled and used for real by `hosts/rust-oracle/tests/c_host.rs` (acceptance D); the
//! Delphi unit still has no host — there is no `dcc64` here.

use mlc::compile_to_ir;
use mlc::header::{emit_c_header, emit_delphi_unit};
use mlc::ir::IrModule;

fn discount_ir() -> IrModule {
    compile_to_ir(include_str!("../../examples/discount.mls")).unwrap()
}

#[test]
fn c_header_matches_module_abi() {
    let h = emit_c_header(&discount_ir(), "discount");
    assert!(h.contains("#ifndef ML_DISCOUNT_H"), "{h}");
    assert!(h.contains("#include <stdbool.h>"), "{h}");
    assert!(h.contains(r#"extern "C""#), "{h}");
    assert!(h.contains("uint32_t ml_module_abi_version(void);"), "{h}");
    assert!(
        h.contains("double mlx_discount(double /* price */, bool /* vip */);"),
        "{h}"
    );
}

#[test]
fn delphi_unit_matches_module_abi() {
    let u = emit_delphi_unit(&discount_ir(), "discount");
    // The unit name IS the module name, because Object Pascal requires it to match the
    // file `mlc build` writes. This used to assert `Mlx_Discount`, a name the tool has
    // never emitted, which is how the golden came to freeze one too.
    assert!(u.contains("unit discount;"), "{u}");
    assert!(u.contains("ML_MODULE = 'discount.dll';"), "{u}");
    assert!(
        u.contains("function ml_module_abi_version: LongWord; cdecl; external ML_MODULE;"),
        "{u}"
    );
    assert!(
        u.contains(
            "function mlx_discount(price: Double; vip: Boolean): Double; cdecl; external ML_MODULE;"
        ),
        "{u}"
    );
    assert!(u.trim_end().ends_with("end."), "{u}");
}

/// **Every conditional block in both binding generators, both branches, one table.**
///
/// `header.rs` decides **twelve** things by inspecting the module — eleven when this test was
/// written; `ML_ST_OVERFLOW` (`SPEC-checked-arithmetic`) is the twelfth — and the count is the first
/// thing this test got wrong: the first draft said eight, covered seven of them, and claimed
/// to cover all. Review counted again. The `.pas` has its own copies of both status constants
/// under their own conditions, and the UTF-8 notice is two emissions, not one.
///
/// Two of the eleven had both branches pinned — `ML_ST_INDEX_OUT_OF_RANGE` (#238) and the
/// UTF-8 notice, in `string_input.rs` — and the rest had **neither**. They lived only in the
/// goldens, which capture the text a module produces but say nothing about the CONDITION:
/// widening one so it fires for every module moves no golden as long as some corpus module
/// already triggers it.
///
/// That is §9-47's rule — a widened gate over-emits as easily as a narrow one under-emits —
/// applied to the binding generators, where it had not been. The failure it guards against is
/// #238's in the other direction: a host reading a header that promises a protocol the module
/// does not implement, or omits one it does.
///
/// The matrix was measured before it was written down (`STATUS.md` §9-58) and every cell was
/// already right; this is what keeps it right. A `.` is as much a claim as a `Y`.
///
/// **What this test does NOT cover, and where it is covered.** Exactly one condition is
/// per-FUNCTION rather than per-module, so it does not fit a module-shaped matrix: the
/// `/* may fail with: … */` provenance line. `fallible_calls.rs` has it both ways — the
/// present form, the `(no domain error)` form, and `!h.contains("may fail with")` for an
/// infallible module.
///
/// A draft of this paragraph also listed the error constants as per-function. They are not:
/// `!module.errors.is_empty()` is a module property, so that row is in the matrix. Reviewing
/// the same claim twice is what moved it — the scope of a test is itself a claim, and this
/// one was restated wrongly after being restated wrongly (`STATUS.md` §9-58.1).
#[test]
fn each_conditional_binding_block_fires_for_exactly_the_shapes_that_need_it() {
    // (source, what the .h must contain, what the .pas must contain)
    //
    // Markers are the first line of each block rather than a word from inside it, so a
    // reworded explanation does not fail this test while a removed block does.
    const H_Q12_STR: &str = "Q12 caller-allocates";
    const H_Q12_ARR: &str = "Q12 for an ARRAY return";
    const H_INSUF: &str = "ML_ST_INSUFFICIENT_BUFFER";
    const H_OOR: &str = "ML_ST_INDEX_OUT_OF_RANGE";
    const H_OVF: &str = "#define ML_ST_OVERFLOW";
    const H_UTF8: &str = "UTF-8";
    // The error-constant block. Module-level (`!module.errors.is_empty()`), so it belongs in
    // this matrix — the first draft of the comment below called it per-function and was wrong.
    const H_ERR: &str = "#define ML_M_ERR_E_BAD";
    const P_ARR_RET: &str = "An ARRAY return";
    const P_ARR_PAR: &str = "An array parameter is TWO";
    const P_UNICODE: &str = "UnicodeString";
    // The `.pas` carries its own copies of both statuses, each under its OWN condition —
    // `returns_str || returns_array` and `can_report_out_of_range`. The first draft of this
    // test checked them only in the `.h`, so a Delphi-side condition could have been widened
    // or dropped without a word. Matched on the constant's DECLARATION (`NAME = value;`)
    // rather than the prose above it, so a reworded comment does not fail this.
    //
    // Built from `abi.rs` rather than typed, for the reason `doc_claims.rs` gives about
    // documents: a value written by hand is a second source of truth, and this one caught a
    // real one — the Pascal emitter was writing `-1` while the C header beside it formatted
    // the constant. Leaking these as `String` rather than `&'static str` is the whole cost.
    let p_insuf = format!(
        "ML_ST_INSUFFICIENT_BUFFER = {};",
        mlc::abi::ML_ST_INSUFFICIENT_BUFFER
    );
    let p_oor = format!(
        "ML_ST_INDEX_OUT_OF_RANGE = {};",
        mlc::abi::ML_ST_INDEX_OUT_OF_RANGE
    );
    let p_ovf = format!("ML_ST_OVERFLOW = {};", mlc::abi::ML_ST_OVERFLOW);
    let (pas_insuf, pas_oor, pas_ovf) = (p_insuf.as_str(), p_oor.as_str(), p_ovf.as_str());
    // Nested inside the `returns_str || returns_array` block: only an ARRAY return gets it.
    const P_ELEMENTS: &str = "count ELEMENTS, not bytes, so the";
    const P_UTF8: &str = "UTF-8";
    const P_ERR: &str = "ML_M_ERR_E_BAD";

    let all = [H_Q12_STR, H_Q12_ARR, H_INSUF, H_OOR, H_OVF, H_UTF8, H_ERR];
    let all_pas = [
        P_ARR_RET, P_ARR_PAR, P_UNICODE, pas_insuf, pas_oor, pas_ovf, P_ELEMENTS, P_UTF8, P_ERR,
    ];

    let cases: &[(&str, &str, &[&str], &[&str])] = &[
        // A scalar function reaches none of it. The most important row: it is the one that
        // fails if any condition is widened to "always".
        (
            "scalar",
            "export fn f(x: f64) -> f64 { return x * 2.0 }",
            &[],
            &[],
        ),
        // A borrowed string return: the string half of Q12, and the Delphi cast trap because
        // there is a string PARAMETER. No index status — nothing here can be out of range.
        (
            "string return",
            "export fn f(s: string) -> string! { return s }",
            &[H_Q12_STR, H_INSUF],
            &[P_UNICODE, pas_insuf],
        ),
        // A BUILT string is the same contract: concatenation does not add a status.
        (
            "built string",
            "export fn f(s: string) -> string! { return s + \"!\" }",
            &[H_Q12_STR, H_INSUF],
            &[P_UNICODE, pas_insuf],
        ),
        // A span CAN be out of range, so this row is #238's rule inside the table: the same
        // return type as the two above, one more constant.
        (
            "span return",
            "export fn f(s: string) -> string! { return byte_slice(s, 0, 2) }",
            &[H_Q12_STR, H_INSUF, H_OOR],
            &[P_UNICODE, pas_insuf, pas_oor],
        ),
        // An array return uses the ELEMENT-counting half of Q12 and must NOT carry the string
        // half — the two protocols share `ml_cap`'s name and differ in its unit, which is the
        // confusion `SPEC-array-return` §2.6 exists to prevent.
        (
            "array return",
            "export fn f(n: i32) -> [i32]! {\n result n\n result[0] = 7\n}",
            &[H_Q12_ARR, H_INSUF, H_OOR],
            &[P_ARR_RET, pas_insuf, pas_oor, P_ELEMENTS],
        ),
        // An array PARAMETER borrows: no buffer, so no truncation status. Indexing can still
        // fail, so the index status stays.
        (
            "array param",
            "export fn f(xs: [i32]) -> i32! { return xs[0] }",
            &[H_OOR],
            &[P_ARR_PAR, pas_oor],
        ),
        // A returned non-ASCII literal: the one-line UTF-8 notice, in BOTH bindings. It rides
        // on the string-return rows' conditions as well, which is why it is its own case —
        // the notice is two emissions under one predicate, and the `.pas` copy had no test.
        (
            "utf-8 return",
            // `\u{..}` is interpreted by RUST, so what reaches the compiler is the two
            // characters themselves — the language refuses escapes inside a literal, and this
            // file stays ASCII either way.
            "export fn f() -> string! { return \"\u{c2b9}\u{c778}\" }",
            &[H_Q12_STR, H_INSUF, H_UTF8],
            &[pas_insuf, P_UTF8],
        ),
        // A declared domain error: the constants appear in both bindings. Every other row
        // declares none, so they carry the absent half of this condition.
        (
            "domain error",
            "error E_BAD = 1\nexport fn f(x: f64) -> f64! {\n if x < 0.0 { fail E_BAD }\n return x\n}",
            &[H_ERR],
            &[P_ERR],
        ),
        // i32 arithmetic in a FALLIBLE body can have no i32 value, so the overflow status is
        // declared. Every other row does arithmetic on nothing, on f64, or in no fallible body,
        // and carries the absent half; which operators count is `checked_arithmetic.rs`'s table.
        (
            "checked arithmetic",
            "export fn f(a: i32) -> i32! { return a + 1 }",
            &[H_OVF],
            &[pas_ovf],
        ),
        // The same status reached ONLY through a helper: the fallible body does no arithmetic
        // of its own. The predicate that looked at that body alone said "absent" here while
        // the module returned -3 (SPEC-helper-check-propagation §2.5).
        (
            "helper overflow",
            "fn sq(x: i32) -> i32 { return x * x }\nexport fn f(x: i32) -> i32! { return sq(x) }",
            &[H_OVF],
            &[pas_ovf],
        ),
        // A string parameter with a scalar return: only the Delphi note, because only Delphi
        // has a way to get this wrong silently (measured 2026-09-10, DP-S2).
        (
            "string param",
            "export fn f(s: string) -> bool { return s == \"a\" }",
            &[],
            &[P_UNICODE],
        ),
    ];

    for (label, src, want_h, want_pas) in cases {
        let ir = compile_to_ir(src).unwrap_or_else(|e| panic!("{label}: {e}"));
        let h = emit_c_header(&ir, "m");
        let pas = emit_delphi_unit(&ir, "m");

        for marker in all {
            let expected = want_h.contains(&marker);
            assert_eq!(
                h.contains(marker),
                expected,
                "{label}: the .h {} contain {marker:?}\n{h}",
                if expected { "must" } else { "must NOT" }
            );
        }
        for marker in all_pas {
            let expected = want_pas.contains(&marker);
            assert_eq!(
                pas.contains(marker),
                expected,
                "{label}: the .pas {} contain {marker:?}\n{pas}",
                if expected { "must" } else { "must NOT" }
            );
        }
    }
}

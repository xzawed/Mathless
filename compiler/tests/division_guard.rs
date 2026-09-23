//! `SPEC-division-guard-operands` — acceptance E (evaluation order).
//!
//! The order cannot be measured by VALUE today. The only failure an expression can produce is
//! `ML_ST_INDEX_OUT_OF_RANGE`, so a program whose two operands both fail returns the same
//! status whichever one ran first. `§3-E` of the SPEC says so rather than inventing a test
//! that would not have measured it — `STATUS.md` §7, *"없는 측정을 테스트로 만들지 않는다"*.
//!
//! What is checkable is the emitted text, and the golden file already freezes it. This exists
//! because a golden is re-blessed with `MATHLESS_BLESS=1`, and a re-bless accepts whatever it
//! is handed: someone could reverse the two bindings, bless it, and see a green suite. This
//! asserts the property itself, so the reversal has to be argued with rather than blessed.
//!
//! Frontend-only — no module is built, so it runs on both CI jobs.

use mlc::compile_to_rust_named;

/// The dividend is bound before the divisor, and the dividend is not inside the `else`.
///
/// Both halves matter and they are different claims. Binding order is what makes evaluation
/// follow source order, the same as the `+ - *` arm where the left operand is the receiver.
/// Being outside the `else` is what makes the left operand evaluated AT ALL when the divisor
/// is zero — which is the defect `STATUS.md` §5-6 recorded: `xs[999] / 0` returned status 0
/// because the bounds check sat in a branch that was not taken.
#[test]
fn the_dividend_is_bound_first_and_outside_the_else() {
    for (op, method) in [("/", "wrapping_div"), ("%", "wrapping_rem")] {
        let src = format!("export fn f(a: i32, b: i32) -> i32 {{ return a {op} b }}");
        let rust = compile_to_rust_named(&src, "f").expect("compile");

        let line = rust
            .lines()
            .find(|l| l.contains(method))
            .unwrap_or_else(|| panic!("no `{method}` in the emitted Rust for `a {op} b`:\n{rust}"));

        let l_at = line
            .find("let __l =")
            .unwrap_or_else(|| panic!("`a {op} b` does not bind the dividend:\n  {line}"));
        let d_at = line
            .find("let __d =")
            .unwrap_or_else(|| panic!("`a {op} b` does not bind the divisor:\n  {line}"));
        assert!(
            l_at < d_at,
            "`a {op} b` binds the divisor before the dividend. Operands evaluate left to \
             right — that is already true of the `+ - *` arm, where the left operand is the \
             receiver, and `/` and `%` were the only ones that differed. Nobody chose that; \
             `__d` was bound first for the zero test.\n  {line}"
        );

        let else_at = line
            .find("else")
            .unwrap_or_else(|| panic!("`a {op} b` has no zero guard at all:\n  {line}"));
        assert!(
            l_at < else_at,
            "`a {op} b` binds the dividend inside the `else`, so a zero divisor skips it. The \
             left operand can early-return — `xs[i]` reports ML_ST_INDEX_OUT_OF_RANGE from \
             expression position — and skipping it is how `xs[999] / 0` came back as status 0 \
             with value 0 (STATUS.md §5-6).\n  {line}"
        );

        // The guard itself is still there: DP-N4 says a zero divisor is a defined `0i32`, not
        // a panic. Without this, deleting the guard would satisfy both assertions above.
        assert!(
            line.contains("if __d == 0 { 0i32 }"),
            "`a {op} b` no longer yields `0i32` for a zero divisor. DP-N4 chose a defined \
             result over a fallible operation, and this slice does not reopen it.\n  {line}"
        );
    }
}

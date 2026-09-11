//! Array **return** — AR1, the frontend (SPEC `docs/slices/SPEC-array-return.md`).
//!
//! This file is written BEFORE the parser understands any of it, so every test here fails
//! on the commit that adds it. That is the point: AR1 is done when they pass and not before.
//!
//! **What AR1 owns.** The surface: `-> [T]!` in return position, the `result <expr>`
//! statement, and `result[i] = e`. Lowering, the ABI triple, the capacity check and the
//! zero fill are AR3's, and the values are AR4's — nothing here loads a module.
//!
//! **The one invariant AR1 exists to protect** is SPEC §2.4: the capacity check has to run
//! before the first element write, on every path. That is why `result` must DOMINATE the
//! writes rather than merely precede them in source order — `if b { result n }` satisfies
//! "exactly one, first" and still skips the check when `b` is false. The launch review
//! found that hole in the draft; these tests are what keep it closed.

use mlc::compile_to_ir;

/// The parser's fallback when it does not recognise a statement at all.
///
/// **Every one of these tests passed vacuously against it on first run.** Today `result` is
/// an unknown identifier, so the fallback quotes it — `found Ident("result")` — and a needle
/// of `"result"` was satisfied by the compiler ECHOING THE SOURCE rather than by any rule
/// being enforced. Eight refusals were green while nothing was implemented.
///
/// That is the shape #203 was about: a guard that pins a phrase instead of a fact. Naming the
/// fallback and refusing it is what makes these tests red until each rule really exists.
const GENERIC_FALLBACK: &str = "expected statement (if|while|return|fail|let|assignment)";

/// Every rejection must name the rule it is enforcing, not just fail.
///
/// `needle` is a word the message has to contain. It is deliberately a SINGLE word or short
/// phrase naming the concept — never a sentence — because `language_gaps.rs` records why
/// pinning diagnostic text makes an improvement look like a failure.
fn refused(label: &str, src: &str, needle: &str) {
    let err = compile_to_ir(src)
        .err()
        .unwrap_or_else(|| panic!("'{label}' compiles, but the SPEC refuses it:\n{src}"));
    let msg = err.to_string();
    assert!(
        !msg.contains(GENERIC_FALLBACK),
        "'{label}' is refused only because nothing parses `result` yet. That is the feature \
         being absent, not the rule being enforced — the author gets no idea which rule they \
         hit. SPEC §3.1 requires every refusal to speak by name:\n{msg}"
    );
    assert!(
        msg.contains(needle),
        "'{label}' is refused but the message never mentions '{needle}', so the author is \
         not told which rule they hit:\n{msg}"
    );
    // The same formatting invariant `language_gaps.rs` guards: a multi-line literal without
    // its trailing backslash prints source indentation at the user. Nine diagnostics shipped
    // that way from the array INPUT slice (#204), so the return slice checks its own from
    // the first commit rather than after.
    assert!(
        !msg.contains("   "),
        "'{label}' prints a run of 3+ spaces — a string literal is missing its `\\`:\n{msg}"
    );
    println!("  {label:<34} {msg}");
}

// ---------------------------------------------------------------- the shape that must work

/// SPEC §1 and §2.4 — the installment schedule, one of the two rules that wanted this slice.
#[test]
fn a_declared_length_then_element_writes_compiles() {
    compile_to_ir(
        "export fn schedule(principal: i32, months: i32) -> [i32]! {\n\
         \x20 result months\n\
         \x20 let mut i = 0\n\
         \x20 while i < months {\n\
         \x20   result[i] = principal / months\n\
         \x20   i = i + 1\n\
         \x20 }\n\
         }",
    )
    .expect("the SPEC's own example must compile");
}

/// §2.6 — the element set is the one array input already uses. If these diverge, an author
/// has to remember two tables.
#[test]
fn all_three_scalar_element_types_are_accepted() {
    for elem in ["i32", "f64", "bool"] {
        let src = format!(
            "export fn f(n: i32) -> [{elem}]! {{\n\
             \x20 result n\n\
             }}"
        );
        compile_to_ir(&src).unwrap_or_else(|e| panic!("[{elem}] return rejected: {e}"));
    }
}

/// §2.3 — a declared scalar `out` still comes first and the return triple last (DP-O1).
/// AR1 only has to accept the shape; AR3 measures the order in the emitted signature.
#[test]
fn a_declared_out_coexists_with_an_array_return() {
    compile_to_ir(
        "export fn f(n: i32, out count: i32) -> [i32]! {\n\
         \x20 result n\n\
         \x20 count = n\n\
         }",
    )
    .expect("an array return must coexist with a declared out-param");
}

// ---------------------------------------------------------------- SPEC §3.1, the rejections

/// §2.3 — truncation is possible on every call, so the status is not optional.
#[test]
fn an_array_return_without_the_bang_is_refused() {
    refused(
        "-> [i32] without !",
        "export fn f(n: i32) -> [i32] { result n }",
        "!",
    );
}

/// §2.3 — same reason `-> T!` and `-> string!` are export-only.
#[test]
fn an_internal_fn_may_not_return_an_array() {
    refused(
        "internal fn -> [i32]!",
        "fn f(n: i32) -> [i32]! { result n }\n\
         export fn g(n: i32) -> i32 { return n }",
        "export",
    );
}

/// §2.4 — without a declared length there is no capacity check, so there is nothing to write
/// into safely.
#[test]
fn a_missing_result_statement_is_refused() {
    refused(
        "no result statement",
        "export fn f(n: i32) -> [i32]! { }",
        "declare",
    );
}

/// §2.4 — two lengths means two capacity checks and an ambiguous `*ml_needed`.
#[test]
fn two_result_statements_are_refused() {
    refused(
        "two result statements",
        "export fn f(n: i32) -> [i32]! {\n\
         \x20 result n\n\
         \x20 result n\n\
         }",
        "exactly one",
    );
}

/// §2.4 — source order is not enough, and this is the case that proves it.
///
/// `if b { result n }` has exactly one `result` and it precedes every write. It still skips
/// the capacity check whenever `b` is false, and then the module writes into a host buffer
/// whose size it never checked. THIS is the test the launch review bought.
#[test]
fn a_result_statement_inside_a_branch_is_refused() {
    refused(
        "result inside if",
        "export fn f(b: bool, n: i32) -> [i32]! {\n\
         \x20 if b { result n }\n\
         \x20 result[0] = 1\n\
         }",
        "top level",
    );
    refused(
        "result inside while",
        "export fn f(n: i32) -> [i32]! {\n\
         \x20 let mut i = 0\n\
         \x20 while i < n { result n  i = i + 1 }\n\
         }",
        "top level",
    );
}

/// §2.4 — writing before the length is declared is writing before the capacity check.
#[test]
fn writing_before_the_result_statement_is_refused() {
    refused(
        "write before result",
        "export fn f(n: i32) -> [i32]! {\n\
         \x20 result[0] = 1\n\
         \x20 result n\n\
         }",
        "before",
    );
}

/// §2.4b (DP-R9 = a′) — once the length is declared, `*ml_needed` is written and the buffer
/// is zero-filled, so a later failure cannot leave them untouched. Rather than weaken the
/// Q12 contract, the compiler refuses the only failures it can see statically.
#[test]
fn failing_after_the_result_statement_is_refused() {
    refused(
        "fail after result",
        "error E_X = 1\n\
         export fn f(n: i32) -> [i32]! {\n\
         \x20 result n\n\
         \x20 fail E_X\n\
         }",
        "after",
    );
    refused(
        "try after result",
        "fn g(a: i32) -> i32! { return a }\n\
         export fn f(n: i32) -> [i32]! {\n\
         \x20 result n\n\
         \x20 let mut v = 0\n\
         \x20 v = try g(n)\n\
         }",
        "after",
    );
}

/// §2.4 — `result` is the host's buffer. There is nothing to read back out of it, exactly as
/// an `out` parameter is write-only.
#[test]
fn reading_result_is_refused() {
    refused(
        "read result as a value",
        "export fn f(n: i32) -> [i32]! {\n\
         \x20 result n\n\
         \x20 let a = result\n\
         }",
        "write-only",
    );
    refused(
        "read result through an index",
        "export fn f(n: i32) -> [i32]! {\n\
         \x20 result n\n\
         \x20 let a = result[0]\n\
         }",
        "write-only",
    );
}

/// §2.4 — `result` is a name that only exists in a function returning an array, the way
/// `len` is a builtin signature rather than a keyword (DP-R7).
#[test]
fn result_outside_an_array_returning_function_is_refused() {
    refused(
        "result in a scalar function",
        "export fn f(n: i32) -> i32 { result n  return n }",
        "[T]!",
    );
}

/// DP-R7 — because it is not a keyword, this has to keep working.
#[test]
fn result_is_still_a_legal_local_name_elsewhere() {
    compile_to_ir("export fn f(a: i32) -> i32 { let result = a  return result }")
        .expect("`result` is not a keyword (DP-R7); a scalar function may still use the name");
}

/// §2.6 — same refusals as array input, and they must say the same thing.
#[test]
fn non_scalar_element_types_are_refused() {
    refused(
        "[string] return",
        "export fn f(n: i32) -> [string]! { result n }",
        "scalar",
    );
    refused(
        "[[i32]] return",
        "export fn f(n: i32) -> [[i32]]! { result n }",
        "scalar",
    );
}

/// §2.4 table — an `out` array is still not the mechanism; the return is.
#[test]
fn an_out_array_parameter_is_still_refused() {
    refused(
        "out array parameter",
        "export fn f(out xs: [i32]) -> i32 { return 0 }",
        "out",
    );
}

// ------------------------------------------------------------ the seam between AR1 and AR3

/// AR1 ends at the IR. Lowering is AR3, and the gap must SAY so.
///
/// A feature that parses and typechecks but does not lower is exactly what STATUS §5-5.2
/// warns about: half implemented, and nothing notices. This pins the half, so the day AR3
/// lands this test fails and somebody has to delete it on purpose.
///
/// The message matters as much as the refusal. Before the guard in `codegen::emit`, this
/// program hit the all-paths-return safety net and the author was told "function 'f' may not
/// return on all paths" — true of the lowered shape, and wrong about their source, which is
/// correct. §9-12 records the same failure: a true sentence pointing at the wrong thing.
#[test]
fn the_backend_refuses_by_name_until_ar3_lands() {
    let ir = compile_to_ir(
        "export fn f(n: i32) -> [i32]! {
           result n
           result[0] = 1
         }",
    )
    .expect("AR1 owns the frontend, so this must typecheck");
    let _ = ir;

    let err = mlc::compile_to_rust(
        "export fn f(n: i32) -> [i32]! {
           result n
           result[0] = 1
         }",
    )
    .expect_err("lowering is AR3; until then the backend must refuse");
    let msg = err.to_string();
    assert!(
        msg.contains("does not lower it yet"),
        "the refusal must say the BACKEND is unfinished: {msg}"
    );
    assert!(
        msg.contains("Nothing is wrong with the source"),
        "and it must say the source is fine, or the author goes looking for a bug they do          not have: {msg}"
    );
    assert!(
        !msg.contains("may not return on all paths"),
        "that is the all-paths safety net firing -- a true sentence about the wrong thing          (§9-12). The guard in codegen::emit exists to get ahead of it: {msg}"
    );
}

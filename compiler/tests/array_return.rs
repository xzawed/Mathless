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

// ------------------------------------------------------------------- AR3b, the lowering

/// The seam test that stood here is gone, and that is the point of having had it.
///
/// It pinned the state where the frontend accepted `result` and the backend refused to lower
/// it, and it was written to FAIL the day AR3 landed so that deleting it would be a decision
/// rather than an oversight. It failed on exactly that commit. What replaces it is this: the
/// order of the lowered steps, which is the whole reason Q12 survives a value that has to be
/// computed.
///
/// The VALUES are measured in `hosts/rust-oracle/tests/array_return.rs`, through a loaded
/// module and a real buffer. This one only reads the text, and STATUS §7 is explicit that
/// text alone proves little — it is here because the ORDER is what a reader has to check, and
/// a reordering that still passes every value test would still be a protocol violation.
#[test]
fn the_capacity_check_precedes_every_write() {
    let rust = mlc::compile_to_rust(
        "export fn f(n: i32) -> [i32]! {
           result n
           result[0] = 1
         }",
    )
    .expect("AR3b lowers this");

    let needed = rust.find("*ml_needed =").expect("*ml_needed is written");
    let check = rust
        .find("if __n > __cap")
        .expect("the capacity is checked");
    let fill = rust
        .find("*ml_buf.add(__z")
        .expect("the elements are zeroed");
    let write = rust
        .find("*ml_buf.add(__i")
        .expect("the author's element write");

    assert!(
        needed < check,
        "*ml_needed must be written BEFORE the capacity test: Q12's truncation row says the          host learns the size it needs, and returning first would deny it that"
    );
    assert!(
        check < fill,
        "the capacity test must come BEFORE the zero fill, or a truncated call has already          written into a buffer it just decided was too small"
    );
    assert!(fill < write, "the fill precedes the author's writes");
    assert!(
        rust.contains("if __n < 0 { 0 }") && rust.contains("if ml_cap < 0 { 0 }"),
        "both negatives are clamped to zero rather than read as huge unsigned values          (§2.7, the _snprintf(count < 0) hazard):
{rust}"
    );
}

// ---------------------------------------------------------------------------- AR3, the ABI

/// SPEC §2.1 and §2.2 — the Q12 triple, with the element unit.
///
/// The triple comes LAST (DP-O1 unchanged): declared `out` parameters first, the return value
/// after them. `ml_cap` and `*ml_needed` count ELEMENTS here, which is NOT the unit the string
/// return uses — that one counts bytes, NUL included. §2.2 chose consistency with array input
/// over consistency with string return, and wrote down what that costs.
#[test]
fn an_array_return_lowers_to_the_q12_triple() {
    let h = mlc::header::emit_c_header(
        &compile_to_ir(
            "export fn schedule(principal: i32, months: i32) -> [i32]! {
               result months
               result[0] = principal
             }",
        )
        .expect("compile"),
        "schedule",
    );
    let decl = h
        .lines()
        .find(|l| l.contains("mlx_schedule"))
        .unwrap_or_else(|| {
            panic!(
                "no declaration in:
{h}"
            )
        });
    // Built by concatenation rather than a continued literal. Twice now a `\`-continued
    // string in this work has silently kept its source indentation (#204, and again while
    // writing AR1's messages), and an expected value carrying that defect fails for a reason
    // that has nothing to do with the code under test.
    // The author's parameter NAMES are commented out on purpose (see `c_param_name`): a name
    // this project does not control must not sit where a preprocessor macro could eat it. The
    // triple's names are not -- those are ours, and the header documents them.
    let want = String::new()
        + "int32_t mlx_schedule(int32_t /* principal */, int32_t /* months */, "
        + "int32_t* ml_buf, int32_t ml_cap, int32_t* ml_needed);";
    assert_eq!(
        decl.trim(),
        want,
        "the triple must be int32_t* for an [i32] return, and it must come last"
    );
}

/// §2.2 — the element type reaches the buffer pointer, so `[f64]` is `double*`.
///
/// This is the case where the unit choice bites: a host that allocates `*ml_needed` BYTES and
/// passes that as `ml_cap` promises eight times the room it has. The header has to say so.
#[test]
fn the_buffer_pointer_carries_the_element_type() {
    for (elem, ctype) in [("i32", "int32_t"), ("f64", "double"), ("bool", "bool")] {
        let src = format!(
            "export fn f(n: i32) -> [{elem}]! {{
               result n
             }}"
        );
        let h = mlc::header::emit_c_header(&compile_to_ir(&src).expect("compile"), "m");
        let decl = h.lines().find(|l| l.contains("mlx_f")).expect("decl");
        assert!(
            decl.contains(&format!("{ctype}* ml_buf")),
            "[{elem}] must hand back a {ctype}* buffer: {decl}"
        );
    }
}

/// §2.2 — the unit difference is a trap, so the header says it in words AND shows the
/// multiplication a host has to do. A comment is the only place this can live: `ml_cap` is the
/// host's promise, and the module has no way to check it.
#[test]
fn the_header_states_the_element_unit_and_the_allocation_size() {
    let h = mlc::header::emit_c_header(
        &compile_to_ir("export fn f(n: i32) -> [f64]! { result n }").expect("compile"),
        "m",
    );
    let low = h.to_lowercase();
    assert!(
        low.contains("element"),
        "the header must say ml_cap counts ELEMENTS:
{h}"
    );
    assert!(
        h.contains("sizeof"),
        "and it must show the allocation size, because copying the string idiom          (malloc(*ml_needed)) under-allocates by sizeof(T):
{h}"
    );
}

/// §2.3 — a declared `out` still precedes the return triple (DP-O1).
#[test]
fn a_declared_out_precedes_the_return_triple() {
    let h = mlc::header::emit_c_header(
        &compile_to_ir(
            "export fn f(n: i32, out count: i32) -> [i32]! {
               result n
               count = n
             }",
        )
        .expect("compile"),
        "m",
    );
    let decl = h.lines().find(|l| l.contains("mlx_f")).expect("decl");
    let out_at = decl.find("count").expect("the declared out");
    let buf_at = decl.find("ml_buf").expect("the buffer");
    assert!(
        out_at < buf_at,
        "the declared out must come before the return triple: {decl}"
    );
}

/// §2.2 — the Delphi unit has to carry the same two facts, in its own spelling.
#[test]
fn the_delphi_unit_declares_the_triple() {
    let unit = mlc::header::emit_delphi_unit(
        &compile_to_ir("export fn f(n: i32) -> [i32]! { result n }").expect("compile"),
        "m",
    );
    assert!(unit.contains("ml_buf"), "{unit}");
    assert!(unit.contains("ml_cap"), "{unit}");
    assert!(unit.contains("ml_needed"), "{unit}");
    assert!(
        unit.to_lowercase().contains("element"),
        "the unit must state the element unit too -- a Delphi host allocating by bytes          over-promises exactly as a C one does:
{unit}"
    );
}

/// The truncation status is a CONSTANT in the unit, for an array return too.
///
/// The C header declares `ML_ST_INSUFFICIENT_BUFFER` for a string return **and** for an
/// array return; the Pascal unit declared it only for the string one. So a Delphi host of
/// an array-returning module had to retype `-1` — the exact thing `hosts/c-host/host.c`
/// refuses to do on the C side ("the error constant comes from the header too … not from a
/// number retyped here"), and the exact asymmetry `SPEC-string-return` DP-T6 closed for
/// strings.
///
/// Found while writing acceptance E of `SPEC-array-return`: `examples/allocate.mls` returns
/// arrays and no strings, and its generated `.h` had the constant while its `.pas` did not.
#[test]
fn the_delphi_unit_declares_the_truncation_status_for_an_array_return() {
    let unit = mlc::header::emit_delphi_unit(
        &compile_to_ir("export fn f(n: i32) -> [i32]! { result n }").expect("compile"),
        "m",
    );
    assert!(
        unit.contains("ML_ST_INSUFFICIENT_BUFFER = -1;"),
        "a module that can truncate must declare the status its host compares against:\n{unit}"
    );
    // And only once, for a module that returns BOTH shapes — a Pascal const block that
    // declares the same identifier twice does not compile.
    let both = mlc::header::emit_delphi_unit(
        &compile_to_ir(
            "export fn a(n: i32) -> [i32]! { result n }\n\
             export fn b(s: string) -> string! { return s }",
        )
        .expect("compile"),
        "m",
    );
    assert_eq!(
        both.matches("ML_ST_INSUFFICIENT_BUFFER = -1;").count(),
        1,
        "declared twice in one const block, which is a compile error in Pascal:\n{both}"
    );
}

/// **The scalar safety net does not reach array ELEMENTS, and the zero-fill makes that quiet.**
///
/// Found by re-running the repository's own section-7 method on 2026-09-12: ten business
/// rules written naturally, compiled, and called from a real C host. All ten worked — but
/// one of them was written with two `if`s and no `else`, because Mathless has no `else`, and
/// that means the author writes the COMPLEMENT CONDITION BY HAND.
///
/// Get that complement wrong and the two shapes diverge:
///
/// | shape | a gap in the conditions |
/// |---|---|
/// | scalar `-> i32` | **compile error** — `may not return on all paths` |
/// | array `-> [i32]!` | **compiles**, and the uncovered index reads back as the zero-fill |
///
/// Measured on a linked C host, with `need <= 0` and `need > 1` leaving `need == 1`
/// uncovered: `status=0 needed=3 result=[5,0,0]` where the answer is `[5,0,1]`.
///
/// **Neither half is a bug.** `typeck.rs` skips the return-path check for an array return on
/// purpose — the value IS the host's buffer and there is no `return` to demand — and DP-R4
/// already declared per-index coverage undecidable, which is why the fill exists at all. The
/// fill is a real gain: it turns an undefined hole into a defined value.
///
/// What this pins is the ASYMMETRY, because an author who has learned "the compiler catches
/// my missing branch" is wrong exactly where the consequence is a plausible number rather
/// than a crash. It is a documented hazard now (`SPEC-array-return` §5.2, `LANGUAGE.md`), and
/// a documented hazard that nothing checks is how the wording drifts.
#[test]
fn a_gap_in_the_conditions_is_caught_for_a_scalar_and_not_for_an_array() {
    // `x == 1` is covered by neither branch.
    let scalar = "export fn band(x: i32) -> i32 {\n\
                  \x20 if x <= 0 { return 0 }\n\
                  \x20 if x > 1 { return 2 }\n\
                  }";
    let err = compile_to_ir(scalar).expect_err("a scalar return must demand every path");
    assert!(
        err.to_string().contains("may not return on all paths"),
        "the scalar half must be refused for the COVERAGE reason, not some other one: {err}"
    );

    // The same gap, one index at a time. Nothing refuses it, and nothing can: whether every
    // index is assigned depends on the loop, which is arbitrary (DP-R4).
    let array = "export fn reorder(on_hand: [i32], target: i32) -> [i32]! {\n\
                 \x20 result len(on_hand)\n\
                 \x20 let mut i = 0\n\
                 \x20 while i < len(on_hand) {\n\
                 \x20   let need = target - on_hand[i]\n\
                 \x20   if need <= 0 { result[i] = 0 }\n\
                 \x20   if need > 1 { result[i] = need }\n\
                 \x20   i = i + 1\n\
                 \x20 }\n\
                 }";
    compile_to_ir(array).expect(
        "the array half must still compile — if this starts failing, per-index coverage has \
         become checkable and SPEC-array-return DP-R4 plus the note in section 5.2 are now \
         wrong, which is a bigger change than this test",
    );
}

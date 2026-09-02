//! STATUS §5-5.2 — every hole in the surface, pinned as rejected.
//!
//! **Why this file exists.** `LANGUAGE.md` lists what the language does not have yet. Nothing
//! held that list to the compiler, so a feature could be *half* implemented — parsed but not
//! checked, or checked but not lowered — and no test would notice. Each case below fails
//! today; the day one starts compiling, this file fails and somebody has to decide whether
//! that was intended.
//!
//! **It is a list, not a judgement.** These are not "bad" constructs. Most are ordinary and
//! will arrive as slices. The test asserts only that today's compiler says no.
//!
//! **When a gap closes**, delete its case here and update `LANGUAGE.md`'s "아직 아님" block in
//! the same PR. The two are a pair: one is the claim, the other is the check.
//!
//! **Diagnostic quality is measured here, not asserted.** §5-5.2 also records that these are
//! the worst messages in the suite — most die as `expected statement …, found Ident`, which
//! does not tell a user the feature is missing rather than mistyped. Pinning the exact text
//! would freeze that; `--nocapture` prints it instead, so an improvement shows up as better
//! output rather than a failing test.

use mlc::compile_to_ir;

/// Assert the source is rejected, and report what the compiler actually said.
fn rejected(label: &str, src: &str) {
    match compile_to_ir(src) {
        Ok(_) => panic!(
            "'{label}' COMPILES now. If that was intended, remove this case and update \
             LANGUAGE.md's \"아직 아님\" block in the same change:\n{src}"
        ),
        Err(e) => println!("  {label:<22} {e}"),
    }
}

// ------------------------------------------------------------------ control flow

#[test]
fn control_flow_that_does_not_exist_yet() {
    rejected(
        "else",
        "export fn f(a: f64, b: f64) -> f64 { if a > b { return a } else { return b } }",
    );
    rejected(
        "break",
        "export fn f(a: f64, b: f64) -> f64 { while a > b { break }  return a }",
    );
    rejected(
        "continue",
        "export fn f(a: f64, b: f64) -> f64 { while a > b { continue }  return a }",
    );
    rejected(
        "for",
        "export fn f(a: f64) -> f64 { for i in 0..3 { }  return a }",
    );
}

#[test]
fn recursion_is_rejected_by_design_not_by_omission() {
    // This one is different from its neighbours: SPEC-calls §5.1 REFUSES it deliberately,
    // because it is statically decidable and the consequence is the process. Relaxing it is
    // an additive change that needs a spec, not a bug fix (STATUS §5-3).
    rejected(
        "recursion",
        "fn r(n: i32) -> i32 { return r(n) }\nexport fn f(a: i32) -> i32 { return r(a) }",
    );
}

// ------------------------------------------------------------------ operators

#[test]
fn operators_that_do_not_exist_yet() {
    rejected(
        "f64 %",
        "export fn f(a: f64, b: f64) -> f64 { return a % b }",
    );
    rejected(
        "compound assign",
        "export fn f(a: f64, b: f64) -> f64 { let mut x = a  x += b  return x }",
    );
    rejected(
        "bitwise &",
        "export fn f(a: i32, b: i32) -> i32 { return a & b }",
    );
    rejected(
        "bitwise |",
        "export fn f(a: i32, b: i32) -> i32 { return a | b }",
    );
    rejected(
        "shift <<",
        "export fn f(a: i32, b: i32) -> i32 { return a << b }",
    );
}

// ------------------------------------------------------------------ strings

#[test]
fn string_operations_that_do_not_exist_yet() {
    // Concatenation and `i32 as string` DO exist since #108 — deliberately absent from this
    // list, and their own tests live in `string_concat.rs`.
    rejected(
        "string ordering",
        "export fn f(s: string, t: string) -> bool { return s < t }",
    );
    rejected(
        "string length",
        "export fn f(s: string) -> i32 { return s.len }",
    );
    rejected(
        "f64 as string",
        "export fn f(x: f64) -> string! { return x as string }",
    );
    rejected(
        "bool as string",
        "export fn f(b: bool) -> string! { return b as string }",
    );
    rejected(
        "string local",
        "export fn f(s: string) -> string! { let t = s  return t }",
    );
    rejected(
        "out string",
        "export fn f(out s: string) -> i32 { return 0 }",
    );
    rejected(
        "string as i32",
        "export fn f(s: string) -> i32 { return s as i32 }",
    );
}

// ------------------------------------------------------------------ data and declarations

#[test]
fn data_shapes_that_do_not_exist_yet() {
    rejected(
        "array type",
        "export fn f(xs: i32[]) -> i32 { return xs[0] }",
    );
    rejected(
        "struct declaration",
        "struct P { x: i32 }\nexport fn f(a: i32) -> i32 { return a }",
    );
    rejected(
        "const declaration",
        "const K = 3\nexport fn f(a: i32) -> i32 { return a }",
    );
    rejected(
        "host fn import",
        "import fn host_log(x: i32)\nexport fn f(a: i32) -> i32 { return a }",
    );
}

// ------------------------------------------------------------------ the list itself

#[test]
fn this_file_covers_the_language_md_gap_list() {
    // A rejection list that drifts from the document it mirrors is worse than none: it looks
    // like coverage. This checks the two are still talking about the same things, by name.
    let language_md = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("docs")
            .join("LANGUAGE.md"),
    )
    .expect("docs/LANGUAGE.md");

    // Each term must appear in LANGUAGE.md somewhere. Loose on purpose — the document is
    // Korean prose, so this catches a gap being DELETED from the doc while still rejected
    // here (or the reverse), not every wording change.
    for term in ["else", "break", "for", "struct", "배열", "재귀", "import"] {
        assert!(
            language_md.contains(term),
            "LANGUAGE.md no longer mentions '{term}', but this file still pins it as rejected \
             — one of the two is out of date"
        );
    }
}

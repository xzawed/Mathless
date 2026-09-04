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
    // The gap block lists 부분문자열 beside 길이 and 순서 비교; only the other two were pinned
    // (found by re-reading the block against this file — STATUS §9-A A1).
    rejected(
        "substring",
        "export fn f(s: string) -> string! { return s[0..2] }",
    );
}

// ------------------------------------------------------------------ conversions

#[test]
fn conversions_that_do_not_exist_yet() {
    // `bool 변환` is the last item of the gap block's final bullet, and nothing pinned it:
    // `as` accepts f64 and i32 only, in both directions.
    rejected(
        "bool as i32",
        "export fn f(b: bool) -> i32 { return b as i32 }",
    );
    rejected(
        "i32 as bool",
        "export fn f(a: i32) -> bool { return a as bool }",
    );
    // 체크드 오버플로 is in the same bullet and is deliberately NOT here: it is a property of
    // arithmetic, not a construct one can write, so there is no source text to reject.
    // `-i32::MIN == i32::MIN` is measured in unary.rs instead.
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
    // "null 안전 또는 option" is a whole bullet of the gap block, and neither half was pinned.
    rejected("option type", "export fn f(a: f64?) -> f64 { return a }");
    rejected("null literal", "export fn f(a: f64) -> f64 { return null }");
}

// ------------------------------------------------------------------ the list itself

/// Each gap named in `LANGUAGE.md`'s block, and the `rejected(…)` label that pins it here.
///
/// The right column is not decoration: it is checked against the actual call sites below, so
/// a row cannot name a case that does not exist.
///
/// One item of the block is deliberately absent — 체크드 오버플로. It is a property of
/// arithmetic rather than a construct, so there is no source text to reject; see
/// `conversions_that_do_not_exist_yet`.
const GAPS: &[(&str, &str)] = &[
    ("부분문자열", "substring"),
    ("순서 비교", "string ordering"),
    ("길이", "string length"),
    ("포맷", "f64 as string"),
    ("지역 변수", "string local"),
    ("struct", "struct declaration"),
    ("배열", "array type"),
    ("option", "option type"),
    ("for", "for"),
    ("else", "else"),
    ("break", "break"),
    ("복합 대입", "compound assign"),
    ("비트", "bitwise &"),
    ("상수 선언", "const declaration"),
    ("import", "host fn import"),
    ("재귀", "recursion"),
    ("나머지", "f64 %"),
    ("bool` 변환", "bool as i32"),
];

/// The label of every `rejected(…)` call in this file, read out of its own source.
///
/// Reading the call sites — not the table above — is what stops the check being circular.
/// A table entry naming a case nobody wrote would otherwise satisfy a search of this file,
/// because the table is *in* this file.
fn pinned_labels() -> Vec<String> {
    let src = include_str!("language_gaps.rs");
    let mut out = Vec::new();
    let mut rest = src;
    while let Some(i) = rest.find("rejected(") {
        rest = &rest[i + "rejected(".len()..];
        let after = rest.trim_start();
        let Some(stripped) = after.strip_prefix('"') else {
            continue; // a call whose label is not a literal — nothing to record
        };
        if let Some(end) = stripped.find('"') {
            out.push(stripped[..end].to_string());
        }
    }
    out
}

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

    // Matched inside the GAP BLOCK, not the whole document. Searching the whole file was the
    // defect (STATUS §9-A A1): every term also occurs in the "현재 구현된 표면" list above it
    // — `else` in "**`else` 없음**", `재귀` in "**재귀는 금지**", `배열` in "정적 NUL 종료
    // 바이트 배열" — so the entire block could be DELETED and this test stayed green. It was
    // asserting that the document mentions the words, and a document saying the opposite
    // mentions them too.
    let anchor = "아직 아님";
    assert_eq!(
        language_md.matches(anchor).count(),
        1,
        "LANGUAGE.md's gap-block anchor '{anchor}' is missing or no longer unique, so this \
         test can no longer find the block it mirrors"
    );
    let from = language_md.find(anchor).expect("anchor counted above");
    let rest = &language_md[from..];
    let block = &rest[..rest.find("\n## ").unwrap_or(rest.len())];
    let bullets = block
        .lines()
        .filter(|l| l.trim_start().starts_with("- "))
        .count();
    assert!(
        bullets >= 5,
        "the \"아직 아님\" block is down to {bullets} bullets — it was probably truncated, and \
         a truncated block is exactly what this test used to miss:\n{block}"
    );

    let labels = pinned_labels();
    for (term, label) in GAPS {
        assert!(
            block.contains(term),
            "LANGUAGE.md's \"아직 아님\" block no longer lists '{term}', but this file still \
             pins it as rejected (case '{label}') — one of the two is out of date"
        );
        assert!(
            labels.iter().any(|l| l == label),
            "GAPS maps '{term}' to a case '{label}' that no rejected(…) call in this file \
             declares. Add the case, or fix the row"
        );
    }

    // What this does NOT do: catch a gap ADDED to the block with no case here. Matching Korean
    // prose bullet-by-bullet was tried and rejected — a bullet lists several gaps separated by
    // `·` and `,`, so a per-bullet check passes on its first recognised term and leaves the
    // rest unpinned, which is how 부분문자열, option, and `bool` 변환 sat unpinned under a
    // guard that claimed to cover the list. They are pinned now; a NEW item still needs a
    // human to add its row above.
}

//! `mlprobe` measures VALUES, so its own test measures values — not that it exits 0.
//!
//! `STATUS.md` §7 says the absence of this tool is what keeps measurement cheap: `mlc build`
//! is one line, "so what is the answer?" is fifteen lines of loader, and that asymmetry let
//! four "compiles, plausible, wrong" defects through. A tool that ran without printing the
//! right number would recreate the same gap while looking like it had closed it.
//!
//! Every case below therefore asserts the printed ANSWER, and each covers a different ABI
//! shape, because the shape is what the generated harness has to get right.
#![cfg(windows)]

use std::process::Command;

fn probe(args: &[&str]) -> String {
    let out = Command::new(env!("CARGO_BIN_EXE_mlprobe"))
        .args(args)
        .output()
        .expect("run mlprobe");
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    assert!(
        out.status.success(),
        "mlprobe failed:\nstdout: {stdout}\nstderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    stdout
}

fn probe_err(args: &[&str]) -> String {
    let out = Command::new(env!("CARGO_BIN_EXE_mlprobe"))
        .args(args)
        .output()
        .expect("run mlprobe");
    assert!(!out.status.success(), "expected a refusal");
    String::from_utf8_lossy(&out.stderr).into_owned()
}

/// A plain scalar return — the simplest shape, and the one §9-46's R1 used.
#[test]
fn it_prints_the_value_of_a_scalar_function() {
    let out = probe(&[
        "--src",
        "export fn copay(amount: f64) -> f64 {\n\
         \x20 if amount < 30000.0 { return amount * 0.30 }\n\
         \x20 if amount < 100000.0 { return amount * 0.25 }\n\
         \x20 return amount * 0.20\n\
         }",
        "copay",
        "50000",
    ]);
    assert!(out.contains("value  = 12500.0"), "{out}");
}

/// A borrowed array plus the length the compiler appends, and a fallible return.
#[test]
fn it_passes_an_array_and_reads_the_out_value() {
    let src = "export fn max_item(xs: [f64]) -> f64! {\n\
               \x20 let mut best = 0.0\n\
               \x20 let mut i = 0\n\
               \x20 while i < len(xs) {\n\
               \x20   if xs[i] > best { best = xs[i] }\n\
               \x20   i = i + 1\n\
               \x20 }\n\
               \x20 return best\n\
               }";
    let out = probe(&["--src", src, "max_item", "3,17.5,9"]);
    assert!(out.contains("status = 0"), "{out}");
    assert!(out.contains("value  = 17.5"), "{out}");

    // `-` is the empty array. The answer is 0 with status 0, which is a property of THIS
    // program rather than of the language — and seeing it is the point of the tool.
    let empty = probe(&["--src", src, "max_item", "-"]);
    assert!(
        empty.contains("status = 0") && empty.contains("value  = 0.0"),
        "{empty}"
    );
}

/// The Q12 caller-allocates protocol: a string in, a string out, and `*ml_needed`.
#[test]
fn it_reads_a_string_return_through_the_q12_triple() {
    let out = probe(&[
        "--src",
        "export fn status_label(code: string) -> string! {\n\
         \x20 if code == \"AP\" { return \"approved\" }\n\
         \x20 return \"in review\"\n\
         }",
        "status_label",
        "AP",
    ]);
    assert!(out.contains("needed = 9"), "8 bytes plus the NUL: {out}");
    assert!(out.contains("value  = \"approved\""), "{out}");
}

/// A declared `out` before D17's `out_value` — DP-O1's order, seen from the outside.
#[test]
fn it_shows_a_declared_out_and_the_return_value_separately() {
    // Relative to the CRATE root, which is where cargo runs a test binary from — not the
    // workspace root. The file path form is exercised here on purpose: it is how the tool is
    // actually used, and it is also what derives the module name.
    let out = probe(&["../examples/commission.mls", "commission_checked", "500000"]);
    assert!(out.contains("out tier = 1"), "{out}");
    assert!(out.contains("value  = 15000.0"), "{out}");
}

/// A negative status is the answer, not a failure of the tool.
#[test]
fn a_negative_status_is_reported_rather_than_hidden() {
    let out = probe(&[
        "--src",
        "export fn is_kookmin(acct: string) -> bool! { return byte_slice(acct, 0, 3) == \"004\" }",
        "is_kookmin",
        "00",
    ]);
    assert!(
        out.contains("status = -2"),
        "an out-of-range span is ML_ST_INDEX_OUT_OF_RANGE, and the host has to see it: {out}"
    );
    assert!(
        !out.contains("value"),
        "nothing may be printed as a value when the call failed: {out}"
    );
}

/// The refusals, because a tool that guesses is worse than one that stops.
#[test]
fn it_refuses_what_it_cannot_answer() {
    let src = "export fn f(a: f64, b: f64) -> f64 { return a + b }";

    let wrong_arity = probe_err(&["--src", src, "f", "1"]);
    assert!(
        wrong_arity.contains("takes 2 argument"),
        "the refusal must say how many it wanted: {wrong_arity}"
    );

    let unknown = probe_err(&["--src", src, "nope"]);
    assert!(unknown.contains("no function 'nope'"), "{unknown}");
    assert!(
        unknown.contains("\"f\""),
        "…and list what this module does export: {unknown}"
    );

    let not_a_number = probe_err(&["--src", src, "f", "x", "2"]);
    assert!(not_a_number.contains("not an f64"), "{not_a_number}");

    // An internal function has no export-table entry, so there is nothing to call.
    let internal = probe_err(&[
        "--src",
        "fn helper(x: f64) -> f64 { return x }\n\
         export fn g(x: f64) -> f64 { return helper(x) }",
        "helper",
        "1",
    ]);
    assert!(internal.contains("internal"), "{internal}");
}

/// The three values that are not finite, in the ARGUMENT direction.
///
/// `f64_literal` exists only for these. `{:?}` round-trips every finite f64, so the tool used
/// it directly until `fixed` needed `NaN`, `inf` and `-inf` — which `{:?}` renders as bare
/// `NaN`/`inf`, became `NaNf64` and `inff64` in the generated harness, and did not compile
/// (STATUS §9-54.4). The fix landed with the slice and **no test came with it**, so for two
/// weeks the one known defect class of the measuring instrument was guarded by nothing.
///
/// These are also exactly the inputs `SPEC-fixed-decimals` §2.4 requires to answer `-2`, so a
/// probe that cannot express them cannot measure that acceptance criterion at all.
#[test]
fn it_can_express_the_three_values_that_are_not_finite() {
    for spelling in ["NaN", "inf", "-inf"] {
        let out = probe(&[
            "--src",
            "export fn amount(won: f64) -> string! { return fixed(won, 2) }",
            "amount",
            spelling,
        ]);
        assert!(
            out.contains("status = -2"),
            "`{spelling}` must reach the module and come back as ML_ST_INDEX_OUT_OF_RANGE: {out}"
        );
    }

    // The control: a finite argument through the same path still answers, so the loop above is
    // not passing because the tool refuses every f64.
    let finite = probe(&[
        "--src",
        "export fn amount(won: f64) -> string! { return fixed(won, 2) }",
        "amount",
        "1234.5",
    ]);
    assert!(finite.contains("\"1234.50\""), "{finite}");

    // The three must stay DISTINCT, which `-2` cannot show: `fixed` refuses all of them, so a
    // `f64_literal` that collapsed the two infinities into `f64::NAN` would pass the loop
    // above unchanged. Verify caught that. An identity function makes the argument observable
    // on the way out, so each spelling has to survive the round trip as itself.
    let id = "export fn id(x: f64) -> f64 { return x }";
    for (spelling, want) in [("NaN", "NaN"), ("inf", "inf"), ("-inf", "-inf")] {
        let out = probe(&["--src", id, "id", spelling]);
        assert!(
            out.contains(&format!("value  = {want}")),
            "`{spelling}` must arrive as itself, not as another non-finite value: {out}"
        );
    }
}

/// The same three values in the RESULT direction, which is a different code path: arguments go
/// through `f64_literal` into the harness source, results come back through `{:?}` at run time.
///
/// STATUS §9-61.3 published a table of what `f64` division hands a host — `inf`, `-inf`, `NaN`
/// — and every row of it was read through this path. The table was shipped before anything
/// asserted the path could print those three.
#[test]
fn it_prints_a_non_finite_result_rather_than_a_number() {
    let src = "export fn ratio(a: f64, b: f64) -> f64 { return a / b }";

    for (a, b, want) in [("1", "0", "inf"), ("-1", "0", "-inf"), ("0", "0", "NaN")] {
        let out = probe(&["--src", src, "ratio", a, b]);
        assert!(
            out.contains(&format!("value  = {want}")),
            "{a} / {b} is {want} and the tool has to say so, not round it to a number: {out}"
        );
    }

    // The control again: the same function on a finite pair prints an ordinary number.
    let out = probe(&["--src", src, "ratio", "3", "2"]);
    assert!(out.contains("value  = 1.5"), "{out}");
}

/// The ARRAY RETURN protocol, which had no test at all.
///
/// The generator has a separate branch for `-> [T]!`: its own buffer, its own `needed`, and a
/// print that slices `&buf[..needed]`. Nothing exercised it. Measured the way STATUS §9-58
/// measures a conditional — plant a defect and see whether anything goes red. Replacing that
/// slice with `&buf[..0]` makes a four-element answer print as `[]`, **and all eight tests
/// stayed green**. A silent wrong answer from the instrument that reports wrong answers.
///
/// All three element types, because the zero used to fill the buffer and the Rust element type
/// are chosen per element (`0.0f64` / `0i32` / `false`) and each of those is its own arm.
#[test]
fn it_prints_an_array_return_element_by_element() {
    let i32s = probe(&[
        "--src",
        "export fn evens(n: i32) -> [i32]! {\n result n\n let mut i = 0\n \
         while i < n { result[i] = i * 2\n i = i + 1 }\n}",
        "evens",
        "4",
    ]);
    assert!(i32s.contains("value  = [0, 2, 4, 6]"), "{i32s}");
    assert!(
        i32s.contains("needed = 4"),
        "the length is the host's, too: {i32s}"
    );

    let f64s = probe(&[
        "--src",
        "export fn halves(xs: [f64]) -> [f64]! {\n result len(xs)\n let mut i = 0\n \
         while i < len(xs) { result[i] = xs[i] / 2.0\n i = i + 1 }\n}",
        "halves",
        "10,7,3",
    ]);
    assert!(f64s.contains("value  = [5.0, 3.5, 1.5]"), "{f64s}");

    // `[bool]` is the one whose ELEMENT WIDTH already cost this repository a silent wrong
    // answer on a Delphi host (SPEC-array-return acceptance E), so the tool printing it
    // element-by-element is worth pinning rather than assuming.
    let bools = probe(&[
        "--src",
        "export fn positive(xs: [i32]) -> [bool]! {\n result len(xs)\n let mut i = 0\n \
         while i < len(xs) { result[i] = xs[i] > 0\n i = i + 1 }\n}",
        "positive",
        "3,-1,0",
    ]);
    assert!(bools.contains("value  = [true, false, false]"), "{bools}");
}

/// The scalar and element types nothing had passed or returned.
///
/// Before this, every argument in this file was an `f64`, a `string` or an `[f64]`, and every
/// return was an `f64`, a `bool!` or a `string!`. The generator has a distinct arm for each
/// type in both directions, and STATUS §9-58's rule applies to a tool as much as to a binding
/// generator: an arm nothing runs is an arm nothing checks.
///
/// Planted to confirm it: making the `i32` argument arm emit `0i32` regardless of the value
/// turns `twice(21)` into `0`, and the eight tests that existed before stayed green.
#[test]
fn every_scalar_and_element_type_survives_the_round_trip() {
    let n = probe(&[
        "--src",
        "export fn twice(n: i32) -> i32 { return n * 2 }",
        "twice",
        "21",
    ]);
    assert!(n.contains("value  = 42"), "an i32 argument and return: {n}");

    let b = probe(&[
        "--src",
        "export fn flip(b: bool) -> bool { return !b }",
        "flip",
        "true",
    ]);
    assert!(
        b.contains("value  = false"),
        "a bool argument and return: {b}"
    );

    let ints = probe(&[
        "--src",
        "export fn total(xs: [i32]) -> i32! {\n let mut s = 0\n let mut i = 0\n \
         while i < len(xs) { s = s + xs[i]\n i = i + 1 }\n return s\n}",
        "total",
        "3,4,5",
    ]);
    assert!(
        ints.contains("value  = 12"),
        "an [i32] argument, i32! return: {ints}"
    );

    let flags = probe(&[
        "--src",
        "export fn any_true(xs: [bool]) -> bool! {\n let mut i = 0\n \
         while i < len(xs) { if xs[i] { return true }\n i = i + 1 }\n return false\n}",
        "any_true",
        "false,true,false",
    ]);
    assert!(
        flags.contains("value  = true"),
        "a [bool] argument: {flags}"
    );
}

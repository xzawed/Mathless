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

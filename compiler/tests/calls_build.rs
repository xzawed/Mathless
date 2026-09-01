//! Programs that the frontend accepts must also BUILD.
//!
//! This file exists because of a specific escape. `calls.rs` asserted DP-C3 — "an export is
//! just a function that is additionally visible outside, so it can be called" — using
//! `compile_to_rust`, which stops before rustc. The assertion passed for four slices while the
//! feature was broken: an export is emitted as `mlx_<name>`, the call site emitted the bare
//! name, and the generated crate failed with `E0425: cannot find function`. The user saw a
//! rustc diagnostic quoting a `src/lib.rs` line in a temp directory they never wrote.
//!
//! The lesson generalises past that one bug: a test that stops at the emitted TEXT cannot see
//! whether the text is valid Rust. So this file takes call-shaped programs and runs the real
//! build. It is slow on purpose — each case is a `cargo build` — so it stays small and covers
//! shapes rather than values. Values are the oracle's job.
#![cfg(windows)]

use mlc::emit::emit_artifacts;

fn builds(tag: &str, src: &str) {
    let out = std::env::temp_dir().join(format!("mlc_cb_{tag}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&out);
    std::fs::create_dir_all(&out).unwrap();
    let r = emit_artifacts(src, tag, &out);
    let _ = std::fs::remove_dir_all(&out);
    if let Err(e) = r {
        panic!("this program type-checks but does not build:\n{src}\n\n{e}");
    }
}

#[test]
fn an_internal_callee_builds() {
    builds(
        "internal",
        "fn helper(x: i32) -> i32 { return x + 1 }\n\
         export fn f(x: i32) -> i32 { return helper(x) }",
    );
}

#[test]
fn an_exported_callee_builds() {
    // The one that was broken. DP-C3 promised it; nothing checked it.
    builds(
        "exported",
        "export fn helper(x: i32) -> i32 { return x + 1 }\n\
         export fn f(x: i32) -> i32 { return helper(x) }",
    );
}

#[test]
fn a_builtin_and_a_user_call_compose() {
    // The builtins are emitted as `ml_<name>` while user functions are not, so a program that
    // mixes the two exercises both spellings of the same syntax.
    builds(
        "mixed",
        "fn half(x: f64) -> f64 { return x * 0.5 }\n\
         export fn f(x: f64) -> f64 { return floor(half(x)) + ceil(x) }",
    );
}

#[test]
fn a_call_inside_control_flow_builds() {
    builds(
        "control",
        "fn step(n: i32) -> i32 { return n + 1 }\n\
         export fn f(n: i32) -> i32 {\n\
           let mut i = 0\n\
           while i < n { i = step(i) }\n\
           if i > 100 { return 100 }\n\
           return i\n\
         }",
    );
}

#[test]
fn an_exported_callee_that_takes_a_string_builds() {
    // A string parameter is `*const u8` in the generated Rust; passing one along a call is the
    // shape most likely to produce a type error only rustc can see.
    builds(
        "strarg",
        "export fn is_kr(s: string) -> bool { return s == \"KR\" }\n\
         export fn f(s: string) -> f64 {\n\
           if is_kr(s) { return 0.1 }\n\
           return 0.0\n\
         }",
    );
}

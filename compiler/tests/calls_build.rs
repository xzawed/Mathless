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

#[test]
fn a_target_keyword_as_a_function_name_builds() {
    // DP-W4 removed the frontend rejection of `fn match`, on the ground that no user function
    // name reaches the generated Rust any more. That ground is only true if the emission
    // really is `ml_fn_<name>` everywhere — declaration AND call site — so this runs rustc
    // rather than trusting the reasoning.
    builds(
        "kw_internal",
        "fn match(x: i32) -> i32 { return x + 1 }\n\
         fn type(x: i32) -> i32 { return match(x) }\n\
         export fn f(x: i32) -> i32 { return type(x) }",
    );
}

#[test]
fn a_target_keyword_as_an_exported_name_still_builds() {
    // This compiled before the refactor too, because the export was emitted as `mlx_type`.
    // It is here as a REGRESSION guard: naming the body with the raw user name would have
    // broken it, which is the measurement that decided DP-W1.
    builds(
        "kw_export",
        "export fn type(x: i32) -> i32 { return x + 1 }\n\
         export fn match(x: i32) -> i32 { return type(x) }",
    );
}

#[test]
fn a_generated_prefix_as_a_function_name_builds() {
    // `fn ml_panic` used to collide with the emitted panic handler; it is now
    // `ml_fn_ml_panic`. `export fn mlx_foo` beside `fn mlx_foo` used to be a duplicate.
    builds(
        "prefixes",
        "fn ml_panic(x: i32) -> i32 { return x }\n\
         fn mlx_foo(x: i32) -> i32 { return ml_panic(x) }\n\
         export fn foo(x: i32) -> i32 { return mlx_foo(x) }",
    );
}

#[test]
fn an_exported_fallible_callee_builds() {
    // DP-F5, unlocked by the refactor: an export is `try`-callable because its body is the
    // same Rust-native shape as any other. Before this, typeck rejected it outright.
    builds(
        "exported_try",
        "error E = 1\n\
         export fn check(x: i32) -> i32! { if x < 0 { fail E } return x }\n\
         export fn use_it(x: i32) -> i32! { let y = try check(x) return y * 2 }",
    );
}

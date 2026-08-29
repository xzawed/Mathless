//! `while` slice (SPEC docs/slices/SPEC-while.md): `while cond { … }`. Internal control flow
//! — no ABI change. The E2 load/call proof is in `hosts/rust-oracle/tests/while_loop.rs` and
//! the C host (`hosts/c-host/host.c`).

use mlc::{compile_to_ir, compile_to_rust};

const SUM_TO: &str = include_str!("../../examples/sum_to.mls");

#[test]
fn while_lowers_to_a_rust_while() {
    let rust = compile_to_rust(SUM_TO).expect("compile sum_to");
    assert!(
        rust.lines().any(|l| l.trim().starts_with("while ")),
        "a while statement should be emitted:\n{rust}"
    );
    assert!(rust.contains("mlx_sum_to"), "{rust}");
    // The loop body mutates the OUTER bindings — one `let mut` each, no re-binding.
    assert_eq!(rust.matches("let mut total").count(), 1, "{rust}");
    assert_eq!(rust.matches("let mut i ").count(), 1, "{rust}");
}

#[test]
fn a_non_bool_condition_is_rejected() {
    let err = compile_to_ir("export fn f() -> i32 { while 1 { } return 0 }").unwrap_err();
    let msg = format!("{err:?}").to_lowercase();
    assert!(msg.contains("bool"), "{err:?}");
}

#[test]
fn a_body_ending_in_while_does_not_count_as_returning() {
    // A `while` can run zero times, so it is not a terminator — same as an `else`-less `if`.
    let err =
        compile_to_ir("export fn f() -> i32 { let mut x = 0 while x < 1 { x = 1 } }").unwrap_err();
    assert!(
        format!("{err:?}").to_lowercase().contains("return"),
        "{err:?}"
    );
}

#[test]
fn while_is_a_statement_not_an_expression() {
    let err = compile_to_ir("export fn f() -> i32 { return while true { } }").unwrap_err();
    assert!(
        matches!(err, mlc::CompileError::Parse(_)),
        "expected a parse error, got {err:?}"
    );
}

#[test]
fn a_local_declared_in_the_loop_body_does_not_leak_out() {
    let err = compile_to_ir("export fn f(b: bool) -> i32 { while b { let r = 1 } return r }")
        .unwrap_err();
    assert!(
        format!("{err:?}").to_lowercase().contains("unknown"),
        "{err:?}"
    );
}

#[test]
fn the_loop_body_can_assign_an_outer_mutable_local() {
    // The whole point of the slice: repetition that accumulates into an outer binding.
    let rust = compile_to_rust(SUM_TO).expect("compile");
    assert!(
        rust.lines().any(|l| l.trim().starts_with("total = ")),
        "assignment inside the loop targets the outer binding:\n{rust}"
    );
}

#[test]
fn assigning_an_immutable_local_inside_a_loop_is_still_rejected() {
    let err = compile_to_ir("export fn f(b: bool) -> i32 { let x = 0 while b { x = 1 } return x }")
        .unwrap_err();
    let msg = format!("{err:?}").to_lowercase();
    assert!(msg.contains("immutable") || msg.contains("mut"), "{err:?}");
}

#[test]
fn a_fallible_fn_may_use_a_loop() {
    let rust = compile_to_rust(
        "error E = 1\nexport fn g(n: i32) -> i32! { let mut i = 0 while i < n { i = i + 1 } if n < 0 { fail E } return i }",
    )
    .expect("compile fallible-with-while");
    assert!(
        rust.lines().any(|l| l.trim().starts_with("while ")),
        "{rust}"
    );
    assert!(
        rust.contains("out_value"),
        "still the fallible ABI:\n{rust}"
    );
}

#[test]
fn a_nested_loop_compiles() {
    let rust = compile_to_rust(
        "export fn f(n: i32) -> i32 { let mut a = 0 let mut i = 0 while i < n { let mut j = 0 while j < n { a = a + 1 j = j + 1 } i = i + 1 } return a }",
    )
    .expect("compile nested");
    assert_eq!(
        rust.lines()
            .filter(|l| l.trim().starts_with("while "))
            .count(),
        2,
        "{rust}"
    );
}

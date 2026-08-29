//! Mutable-locals (`let mut`) + assignment slice (SPEC docs/phase1/SPEC-let-mut.md):
//! `let mut NAME = EXPR` declares a mutable local, `NAME = EXPR` reassigns one. Internal
//! only — no ABI change. The E2 load/call proof is in `hosts/rust-oracle/tests/let_mut.rs`.

use mlc::{compile_to_ir, compile_to_rust};

const DISCOUNT3: &str = include_str!("../../examples/discount3.mls");

#[test]
fn let_mut_and_assignment_lower_to_rust() {
    let rust = compile_to_rust(DISCOUNT3).expect("compile discount3");
    assert!(
        rust.contains("let mut result = price;"),
        "`let mut` lowers to a mutable Rust binding:\n{rust}"
    );
    // An assignment line, not a second binding. (The emitter parenthesises binary
    // expressions — assert the statement shape, not that formatting detail.)
    assert!(
        rust.lines().any(|l| {
            let l = l.trim();
            l.starts_with("result = ") && l.contains("0.9f64") && l.ends_with(';')
        }),
        "assignment lowers to a Rust assignment statement:\n{rust}"
    );
    assert!(rust.contains("mlx_discount3"), "{rust}");
}

#[test]
fn assigning_inside_an_if_targets_the_outer_binding() {
    // The assignment sits inside the `if` body but must resolve to the OUTER `let mut`
    // (block scope keeps the parent binding visible, and Rust's `x = e;` mutates it).
    let rust = compile_to_rust(DISCOUNT3).expect("compile discount3");
    let mut_pos = rust.find("let mut result").expect("binding emitted");
    let if_pos = rust.find("if vip").expect("if emitted");
    assert!(
        mut_pos < if_pos,
        "the binding is declared outside the if:\n{rust}"
    );
    // Exactly one binding — the assignment must not introduce a second `let`.
    assert_eq!(
        rust.matches("let mut result").count(),
        1,
        "assignment must not re-bind:\n{rust}"
    );
}

#[test]
fn reassigning_an_immutable_let_is_rejected() {
    let err = compile_to_ir("export fn f() -> i32 { let x = 1 x = 2 return x }").unwrap_err();
    let msg = format!("{err:?}").to_lowercase();
    assert!(
        msg.contains("immutable") || msg.contains("mut"),
        "should say the target is not mutable: {err:?}"
    );
}

#[test]
fn reassigning_a_parameter_is_rejected() {
    // D16: an argument is a borrow for the duration of the call — parameters are immutable.
    let err = compile_to_ir("export fn f(a: i32) -> i32 { a = 1 return a }").unwrap_err();
    let msg = format!("{err:?}").to_lowercase();
    assert!(
        msg.contains("parameter") || msg.contains("immutable") || msg.contains("mut"),
        "should reject assigning to a parameter: {err:?}"
    );
}

#[test]
fn assigning_an_undeclared_name_is_rejected() {
    let err = compile_to_ir("export fn f() -> i32 { y = 1 return 0 }").unwrap_err();
    assert!(
        format!("{err:?}").to_lowercase().contains("unknown"),
        "{err:?}"
    );
}

#[test]
fn assigning_a_mismatched_type_is_rejected() {
    let err = compile_to_ir("export fn f() -> i32 { let mut x = 1 x = 1.0 return x }").unwrap_err();
    let msg = format!("{err:?}").to_lowercase();
    assert!(msg.contains("mismatch") || msg.contains("type"), "{err:?}");
}

#[test]
fn a_block_ending_in_an_assignment_is_rejected() {
    // An assignment is not a terminator — "all paths return" still applies.
    let err = compile_to_ir("export fn f() -> i32 { let mut x = 0 x = 1 }").unwrap_err();
    assert!(
        format!("{err:?}").to_lowercase().contains("return"),
        "{err:?}"
    );
}

#[test]
fn assignment_is_a_statement_not_an_expression() {
    // DP-M3: `a = b = c` and `return x = 1` must not parse.
    let err = compile_to_ir("export fn f() -> i32 { let mut x = 0 return x = 1 }").unwrap_err();
    assert!(
        matches!(err, mlc::CompileError::Parse(_)),
        "expected a parse error, got {err:?}"
    );
}

#[test]
fn a_reserved_mutable_local_name_is_rejected() {
    let err = compile_to_ir("export fn f() -> f64 { let mut type = 1.0 return type }").unwrap_err();
    assert!(
        format!("{err:?}").to_lowercase().contains("reserved"),
        "{err:?}"
    );
}

#[test]
fn a_mutable_local_named_out_value_in_a_fallible_fn_is_rejected() {
    let err = compile_to_ir("export fn f() -> f64! { let mut out_value = 1.0 return out_value }")
        .unwrap_err();
    assert!(format!("{err:?}").contains("out_value"), "{err:?}");
}

#[test]
fn a_mutable_local_shadowing_a_parameter_is_rejected() {
    let err = compile_to_ir("export fn f(x: f64) -> f64 { let mut x = 1.0 return x }").unwrap_err();
    assert!(
        format!("{err:?}").to_lowercase().contains("scope"),
        "{err:?}"
    );
}

#[test]
fn a_mutable_local_declared_in_an_if_does_not_leak_out() {
    let err =
        compile_to_ir("export fn f(b: bool) -> f64 { if b { let mut r = 1.0 return r } return r }")
            .unwrap_err();
    assert!(
        format!("{err:?}").to_lowercase().contains("unknown"),
        "{err:?}"
    );
}

#[test]
fn a_fallible_fn_may_use_a_mutable_local() {
    // Positive coverage: `let mut` + assignment interact correctly with the D17 lowering.
    let rust = compile_to_rust(
        "error E = 1\nexport fn g(a: f64) -> f64! { let mut s = a if a < 0.0 { fail E } s = s * 2.0 return s }",
    )
    .expect("compile fallible-with-let-mut");
    assert!(rust.contains("let mut s = a;"), "{rust}");
    assert!(
        rust.lines()
            .any(|l| l.trim().starts_with("s = ") && l.contains("2.0f64")),
        "{rust}"
    );
    assert!(
        rust.contains("out_value"),
        "still the fallible ABI:\n{rust}"
    );
}

//! Local-variables (`let`) slice (SPEC docs/slices/SPEC-let-locals.md): `let NAME = EXPR`
//! block-scoped, immutable, inferred-typed locals. Internal only — no ABI change. The E2
//! load/call proof is in `hosts/rust-oracle/tests/let_locals.rs`.

use mlc::{compile_to_ir, compile_to_rust};

const DISCOUNT2: &str = "\
export fn discount2(price: f64, vip: bool) -> f64 {
  let rate = 0.9
  if vip { return price * rate }
  return price
}
";

#[test]
fn let_binding_lowers_to_a_rust_let() {
    let rust = compile_to_rust(DISCOUNT2).expect("compile discount2");
    assert!(
        rust.contains("let rate = 0.9f64;"),
        "let lowers to a Rust let:\n{rust}"
    );
    assert!(rust.contains("mlx_discount2"), "{rust}");
}

#[test]
fn use_before_definition_is_rejected() {
    // RHS is checked before the binding is added, so `x` is unknown in `let x = x`.
    let err = compile_to_ir("export fn f() -> f64 { let x = x return x }").unwrap_err();
    assert!(
        format!("{err:?}").to_lowercase().contains("unknown"),
        "{err:?}"
    );
}

#[test]
fn redeclaration_in_the_same_block_is_rejected() {
    let err =
        compile_to_ir("export fn f() -> f64 { let x = 1.0 let x = 2.0 return x }").unwrap_err();
    assert!(
        format!("{err:?}").to_lowercase().contains("scope"),
        "{err:?}"
    );
}

#[test]
fn a_local_shadowing_a_parameter_is_rejected() {
    let err = compile_to_ir("export fn f(x: f64) -> f64 { let x = 1.0 return x }").unwrap_err();
    assert!(
        format!("{err:?}").to_lowercase().contains("scope"),
        "{err:?}"
    );
}

#[test]
fn a_reserved_local_name_is_rejected() {
    let err = compile_to_ir("export fn f() -> f64 { let type = 1.0 return type }").unwrap_err();
    assert!(
        format!("{err:?}").to_lowercase().contains("reserved"),
        "{err:?}"
    );
}

#[test]
fn a_local_named_out_value_in_a_fallible_fn_is_rejected() {
    // `out_value` is the synthesized D17 out-param; a local of that name would collide.
    let err = compile_to_ir("export fn f() -> f64! { let out_value = 1.0 return out_value }")
        .unwrap_err();
    assert!(format!("{err:?}").contains("out_value"), "{err:?}");
}

#[test]
fn a_local_declared_in_an_if_does_not_leak_out() {
    // `r` is block-scoped to the `if`; the outer `return r` must be an unknown variable.
    let err =
        compile_to_ir("export fn f(b: bool) -> f64 { if b { let r = 1.0 return r } return r }")
            .unwrap_err();
    assert!(
        format!("{err:?}").to_lowercase().contains("unknown"),
        "{err:?}"
    );
}

#[test]
fn a_fallible_fn_may_use_a_local() {
    // Positive coverage: `let` interacts correctly with the D17 status+out-param lowering.
    let rust = compile_to_rust(
        "error E = 1\nexport fn g(a: f64) -> f64! { let two = 2.0 if a < 0.0 { fail E } return a * two }",
    )
    .expect("compile fallible-with-let");
    assert!(rust.contains("let two = 2.0f64;"), "{rust}");
    assert!(
        rust.contains("out_value"),
        "still the fallible ABI:\n{rust}"
    );
}

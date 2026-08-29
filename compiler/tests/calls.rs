//! Internal-functions-and-calls slice (SPEC docs/slices/SPEC-calls.md): non-exported `fn`
//! declarations and call expressions. Internal helpers must NOT reach the export table —
//! that is measured in `hosts/rust-oracle/tests/calls.rs`.

use mlc::{compile_to_ir, compile_to_rust};

const DISCOUNT4: &str = include_str!("../../examples/discount4.mls");

#[test]
fn an_internal_helper_can_be_declared_and_called() {
    let rust = compile_to_rust(DISCOUNT4).expect("compile discount4");
    assert!(rust.contains("vip_rate"), "the helper is emitted:\n{rust}");
    assert!(rust.contains("mlx_discount4"), "{rust}");
}

#[test]
fn the_internal_helper_is_not_exported() {
    // The point of the slice: logic moves inside without growing the export surface.
    let rust = compile_to_rust(DISCOUNT4).expect("compile");
    assert!(
        !rust.contains("mlx_vip_rate"),
        "an internal fn must not get the export prefix:\n{rust}"
    );
    // Exactly one `#[no_mangle]` per exported item: mlx_discount4 and ml_module_abi_version.
    assert_eq!(
        rust.matches("#[no_mangle]").count(),
        2,
        "only the ABI symbol and the one export may be no_mangle:\n{rust}"
    );
}

#[test]
fn declaration_order_does_not_matter() {
    // Two-pass name resolution (DP-C4): the callee is declared after the caller.
    compile_to_rust(
        "export fn f(x: i32) -> i32 { return g(x) }\nfn g(x: i32) -> i32 { return x + 1 }",
    )
    .expect("forward reference should resolve");
}

#[test]
fn direct_recursion_is_rejected() {
    let err =
        compile_to_ir("fn f(x: i32) -> i32 { return f(x) }\nexport fn g() -> i32 { return f(1) }")
            .unwrap_err();
    let msg = format!("{err:?}").to_lowercase();
    assert!(msg.contains("recursi") || msg.contains("cycle"), "{err:?}");
}

#[test]
fn mutual_recursion_is_rejected() {
    let err = compile_to_ir(
        "fn a(x: i32) -> i32 { return b(x) }\nfn b(x: i32) -> i32 { return a(x) }\nexport fn e() -> i32 { return a(1) }",
    )
    .unwrap_err();
    let msg = format!("{err:?}").to_lowercase();
    assert!(msg.contains("recursi") || msg.contains("cycle"), "{err:?}");
    // The message should name the functions involved, not just say "cycle".
    assert!(
        format!("{err:?}").contains('a') && format!("{err:?}").contains('b'),
        "{err:?}"
    );
}

#[test]
fn a_diamond_call_graph_is_fine() {
    // Not a cycle: two callers sharing a callee must still compile.
    compile_to_rust(
        "fn leaf(x: i32) -> i32 { return x + 1 }\nfn a(x: i32) -> i32 { return leaf(x) }\nfn b(x: i32) -> i32 { return leaf(x) }\nexport fn top(x: i32) -> i32 { return a(x) + b(x) }",
    )
    .expect("a diamond is acyclic");
}

#[test]
fn calling_an_unknown_function_is_rejected() {
    let err = compile_to_ir("export fn f() -> i32 { return nope(1) }").unwrap_err();
    assert!(
        format!("{err:?}").to_lowercase().contains("unknown"),
        "{err:?}"
    );
}

#[test]
fn argument_count_and_types_must_match() {
    for src in [
        "fn g(a: i32, b: i32) -> i32 { return a }\nexport fn f() -> i32 { return g(1) }",
        "fn g(a: i32) -> i32 { return a }\nexport fn f() -> i32 { return g(1, 2) }",
        "fn g(a: i32) -> i32 { return a }\nexport fn f() -> i32 { return g(1.5) }",
    ] {
        let err = compile_to_ir(src).unwrap_err();
        let msg = format!("{err:?}").to_lowercase();
        assert!(
            msg.contains("argument") || msg.contains("expects") || msg.contains("type"),
            "{src} -> {err:?}"
        );
    }
}

#[test]
fn calling_a_fallible_function_is_rejected_for_now() {
    // DP-C1: `-> T!` lowers to status + out-param, so it is not a value.
    let err = compile_to_ir(
        "error E = 1\nfn g(x: i32) -> i32! { if x < 0 { fail E } return x }\nexport fn f() -> i32 { return g(1) }",
    )
    .unwrap_err();
    let msg = format!("{err:?}").to_lowercase();
    assert!(msg.contains("fallible"), "{err:?}");
}

#[test]
fn an_exported_function_may_also_be_called() {
    // DP-C3: an export is just a function that is additionally visible outside.
    compile_to_rust(
        "export fn helper(x: i32) -> i32 { return x + 1 }\nexport fn f(x: i32) -> i32 { return helper(x) }",
    )
    .expect("calling an exported fn is allowed");
}

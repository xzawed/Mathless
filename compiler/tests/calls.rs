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
    //
    // The callee must be `export`ed for this test to reach the CALL SITE at all: an
    // internal `-> T!` is now rejected at its declaration, so the original internal-`g`
    // form would still fail this assertion while testing the other rule entirely.
    let err = compile_to_ir(
        "error E = 1\nexport fn g(x: i32) -> i32! { if x < 0 { fail E } return x }\nexport fn f() -> i32 { return g(1) }",
    )
    .unwrap_err();
    let msg = format!("{err:?}").to_lowercase();
    assert!(msg.contains("fallible"), "{err:?}");
    assert!(
        !msg.contains("internal function"),
        "must be the call-site rule, not the declaration rule: {err:?}"
    );
}

#[test]
fn an_exported_function_may_also_be_called() {
    // DP-C3: an export is just a function that is additionally visible outside.
    //
    // This test used to stop at `compile_to_rust`, which never invokes rustc — so it passed
    // while the feature it asserts was broken: an export is emitted as `mlx_<name>`, the call
    // site emitted the bare name, and the generated crate failed to build with a rustc error
    // quoting a file the user never wrote. Asserting the emitted name is what a unit test can
    // do; `compiler/tests/calls_build.rs` runs the real build.
    let rust = compile_to_rust(
        "export fn helper(x: i32) -> i32 { return x + 1 }\nexport fn f(x: i32) -> i32 { return helper(x) }",
    )
    .expect("calling an exported fn is allowed");
    assert!(
        rust.contains("mlx_helper(x)"),
        "an exported callee must be called by its emitted name:\n{rust}"
    );
}

#[test]
fn a_callee_with_an_out_parameter_is_rejected() {
    // An `out` is a pointer, and a call expression has no syntax for taking an address.
    // `Sig` did not model out-ness, so this type-checked against a signature that does not
    // exist and then failed inside the generated crate, with no source position. Today the
    // missing-name error masked it; fixing that one exposed this one.
    let err = compile_to_ir(
        "export fn c(a: f64, out tier: i32) -> f64 { tier = 1 return a }\n\
         export fn f(a: f64) -> f64 { return c(a, 7) }",
    )
    .unwrap_err()
    .to_string();
    assert!(err.contains("its parameter 'tier' is an `out`"), "{err}");
}

#[test]
fn an_internal_function_name_must_be_safe_in_the_target() {
    // Internal names are emitted RAW into the generated Rust — the `mlx_` prefix that made
    // exported names safe does not apply to them. Without a frontend check these die as
    // "cargo build of generated crate failed", which says nothing about the cause.
    for src in [
        "fn type(x: i32) -> i32 { return x }\nexport fn f() -> i32 { return type(1) }",
        "fn match(x: i32) -> i32 { return x }\nexport fn f() -> i32 { return match(1) }",
    ] {
        let err = compile_to_ir(src).unwrap_err();
        assert!(
            format!("{err:?}").to_lowercase().contains("reserved"),
            "{src} -> {err:?}"
        );
    }
}

#[test]
fn an_internal_function_may_not_squat_the_reserved_prefixes() {
    // `ml_*` is the runtime namespace and `mlx_*` is what exports are emitted as (D18), so an
    // internal function using either can collide with a generated symbol. `fn mlx_foo` beside
    // `export fn foo` is a duplicate definition; `fn ml_panic` collides with the emitted
    // panic handler.
    for src in [
        "fn mlx_foo(x: i32) -> i32 { return x }\nexport fn foo(x: i32) -> i32 { return mlx_foo(x) }",
        "fn ml_module_abi_version(x: i32) -> i32 { return x }\nexport fn f() -> i32 { return ml_module_abi_version(1) }",
        "fn ml_panic(x: i32) -> i32 { return x }\nexport fn f() -> i32 { return ml_panic(1) }",
    ] {
        let err = compile_to_ir(src).unwrap_err();
        let msg = format!("{err:?}").to_lowercase();
        assert!(
            msg.contains("ml_") || msg.contains("reserved") || msg.contains("prefix"),
            "{src} -> {err:?}"
        );
    }
}

#[test]
fn an_exported_name_may_still_look_like_a_prefix() {
    // Exports are emitted as `mlx_<name>`, so `export fn mlx_foo` becomes `mlx_mlx_foo` —
    // ugly but not a collision. Don't reject what isn't broken.
    compile_to_rust("export fn mlx_foo(x: i32) -> i32 { return x }").expect("no collision");
}

#[test]
fn an_internal_function_may_be_fallible_now() {
    // #67 rejected this, and SPEC-calls section 5.3 recorded why: codegen had no shape for an
    // internal `-> T!` (it emitted `fallible = false`, so `fail E` became a plain
    // `return <code>` — the error code returned as an ordinary value), and DP-C1 made the
    // function uncallable anyway, so building the machinery would have been dead code.
    //
    // SPEC-fallible-calls lifts it, exactly as section 5.3 promised it would be lifted
    // "가산적으로": `try` supplies the call form, codegen supplies `Result<T, i32>`. Kept as
    // a test rather than deleted, so the unlock stays deliberate.
    compile_to_rust(
        "error E = 1\nfn g(x: i32) -> i32! { if x < 0 { fail E } return x }\n\
         export fn f(x: i32) -> i32! { return try g(x) }",
    )
    .expect("an internal fallible fn is legal once it can be called");
}

#[test]
fn a_fallible_callee_still_needs_a_caller_that_can_propagate() {
    // The declaration ban is gone; the ABI reason behind it is not. A non-fallible caller has
    // no status channel, so a propagated failure would have nowhere to go — and the old
    // symptom was exactly that: for `-> i32!` the code came back as an ordinary value.
    //
    // Checked for every return type, because the old defect differed by type: `f64!` and
    // `bool!` died inside rustc with no source position, while `i32!` compiled and lied.
    for (ty, ok) in [("i32", "0"), ("f64", "0.0"), ("bool", "true")] {
        let src = format!(
            "error E = 1\nfn g(x: i32) -> {ty}! {{ if x < 0 {{ fail E }} return {ok} }}\n\
             export fn f(x: i32) -> i32 {{ let y = try g(x) return x }}"
        );
        let err = compile_to_ir(&src).unwrap_err();
        let msg = format!("{err:?}").to_lowercase();
        assert!(msg.contains("fallible"), "{src} -> {err:?}");
    }
}

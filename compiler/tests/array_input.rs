//! Array **input** parameters (SPEC docs/slices/SPEC-array-input.md).
//!
//! The slice's justification is not capability — STATUS §9-9 measured for the fifth time that
//! arrays are not forced. It is the two lines that section left: a frozen fold host has to
//! guess the schedule, and a general fold pays in calls and in exposure.
//!
//! This file pins the frontend and the emitted shape. The load/call proof is in
//! `hosts/rust-oracle/tests/array_input.rs`.

use mlc::header::{emit_c_header, emit_delphi_unit};
use mlc::{compile_to_ir, compile_to_rust};

/// DP-A2: one surface parameter, two C parameters, pointer then length, in place.
#[test]
fn an_array_parameter_lowers_to_a_pointer_and_a_length() {
    let rust = compile_to_rust(
        "export fn total(xs: [i32], factor: i32) -> i32! {\n\
         \x20 let mut s = 0\n\
         \x20 let mut i = 0\n\
         \x20 while i < len(xs) {\n\
         \x20   s = s + xs[i]\n\
         \x20   i = i + 1\n\
         \x20 }\n\
         \x20 return s * factor\n\
         }",
    )
    .expect("compile");
    let sig = rust
        .lines()
        .find(|l| l.contains("mlx_total"))
        .expect("the export signature")
        .to_string();
    assert!(sig.contains("xs: *const i32"), "{sig}");
    assert!(sig.contains("xs_len: i32"), "{sig}");
    assert!(
        sig.find("xs: *const i32").unwrap() < sig.find("xs_len: i32").unwrap(),
        "pointer comes first: {sig}"
    );
    assert!(
        sig.find("xs_len: i32").unwrap() < sig.find("factor: i32").unwrap(),
        "the companion sits with its array, not at the end: {sig}"
    );
}

/// §2.4. Indexing can fail, and D17 forbids an invisible status channel — so the failure has
/// to be visible in the signature. The diagnostic must name the fix, not just the problem.
#[test]
fn indexing_in_a_non_fallible_function_is_rejected() {
    let err = compile_to_ir("export fn f(xs: [i32]) -> i32 { return xs[0] }").unwrap_err();
    let msg = format!("{err:?}");
    assert!(msg.contains("!"), "the fix must be named: {msg}");
    let lower = msg.to_lowercase();
    assert!(
        lower.contains("index") || lower.contains("array"),
        "and the cause: {msg}"
    );
}

/// Taking an array is not by itself fallible: `len` cannot fail.
#[test]
fn taking_an_array_without_indexing_needs_no_bang() {
    compile_to_rust("export fn n(xs: [i32]) -> i32 { return len(xs) }")
        .expect("len alone does not make a function fallible");
}

/// §2.1: borrowed means read-only. A module writing into host memory reopens ownership.
#[test]
fn writing_through_an_index_is_rejected() {
    let err = compile_to_ir("export fn f(xs: [i32]) -> i32! { xs[0] = 1 return 0 }").unwrap_err();
    let lower = format!("{err:?}").to_lowercase();
    assert!(
        lower.contains("read") || lower.contains("write") || lower.contains("borrow"),
        "{err:?}"
    );
}

/// §2.1: parameter position only. Each of these asks where the elements live, and the module
/// has no allocator — the same answer `string` gives for locals.
#[test]
fn arrays_outside_a_parameter_are_rejected() {
    for src in [
        "export fn f(xs: [i32]) -> [i32] { return xs }",
        "export fn f() -> i32! { let ys = [1, 2] return 0 }",
        "export fn f(xs: [[i32]]) -> i32 { return 0 }",
    ] {
        assert!(compile_to_ir(src).is_err(), "{src} must be rejected");
    }
}

/// §2.1: scalar elements only. A string element reopens variable length inside variable
/// length, which is the thing Q12 exists to bound.
#[test]
fn a_string_element_type_is_rejected() {
    let err = compile_to_ir("export fn f(xs: [string]) -> i32 { return 0 }").unwrap_err();
    let lower = format!("{err:?}").to_lowercase();
    assert!(
        lower.contains("string") || lower.contains("scalar"),
        "{err:?}"
    );
}

/// §2.2: the companion is the compiler's, and a collision has to be loud. `out_value` is
/// reserved the same way (#67).
#[test]
fn a_companion_name_clash_is_rejected() {
    let err =
        compile_to_ir("export fn f(xs: [i32], xs_len: i32) -> i32 { return xs_len }").unwrap_err();
    assert!(
        format!("{err:?}").contains("xs_len"),
        "the message should name the collision: {err:?}"
    );
}

/// §3.1: no implicit conversion at the index, the same rule as everywhere else (DP-I2).
#[test]
fn a_non_i32_index_is_rejected() {
    let err = compile_to_ir("export fn f(xs: [i32]) -> i32! { return xs[1.0] }").unwrap_err();
    let lower = format!("{err:?}").to_lowercase();
    assert!(lower.contains("i32") || lower.contains("index"), "{err:?}");
}

/// §2.5: `len` is a builtin SIGNATURE, not a keyword — the rule DP-R1 set for the rounding
/// builtins. A local called `len` must still be legal.
#[test]
fn len_is_a_builtin_not_a_keyword() {
    compile_to_rust("export fn f(a: i32) -> i32 { let len = 3 return a + len }")
        .expect("`len` is still usable as a name");
}

/// §2.3: the check is `0 <= i < xs_len`, and the failure is the reserved negative.
#[test]
fn an_out_of_range_index_returns_the_reserved_negative() {
    let rust = compile_to_rust("export fn f(xs: [i32], i: i32) -> i32! { return xs[i] }")
        .expect("compile");
    assert!(
        rust.contains("-2"),
        "the reserved status must appear in the generated body:\n{rust}"
    );
    assert!(
        rust.contains("xs_len"),
        "and the bound must be the companion, not a constant:\n{rust}"
    );
}

/// §2.2 / §3-A: what a host actually reads.
#[test]
fn the_bindings_declare_two_parameters() {
    let ir = compile_to_ir("export fn total(xs: [i32]) -> i32! { return xs[0] }").expect("compile");
    let h = emit_c_header(&ir, "m");
    // The C header spells parameter names as comments (`c_param_name`), so the companion has
    // to go through the same treatment — the first cut produced `int32_t /* xs */_len`, which
    // is a name leaking out of the convention that hides it.
    assert!(
        h.contains("const int32_t* /* xs */, int32_t /* xs_len */"),
        "the C header pairs them: {h}"
    );
    let pas = emit_delphi_unit(&ir, "m");
    assert!(
        pas.contains("xs: PInteger; xs_len: Integer"),
        "and so does the unit: {pas}"
    );
}

/// §2.7: the manifest records the SURFACE type, so adding an array changes the fingerprint.
/// The companion does not appear — it is not part of the surface.
#[test]
fn the_fingerprint_sees_the_surface_type_and_not_the_companion() {
    let with = mlc::iface::manifest(
        &compile_to_ir("export fn f(xs: [i32]) -> i32! { return xs[0] }").expect("compile"),
    );
    assert!(with.contains("xs:[i32]"), "{with}");
    assert!(
        !with.contains("xs_len"),
        "the companion is not surface: {with}"
    );

    let scalar = mlc::iface::manifest(
        &compile_to_ir("export fn f(xs: i32) -> i32! { return xs }").expect("compile"),
    );
    assert_ne!(with, scalar, "an array parameter must change the manifest");
}

/// §3-H: an empty array is an ordinary success path, not an error.
#[test]
fn an_empty_array_is_a_normal_path() {
    compile_to_rust(
        "export fn total(xs: [i32]) -> i32! {\n\
         \x20 let mut s = 0\n\
         \x20 let mut i = 0\n\
         \x20 while i < len(xs) {\n\
         \x20   s = s + xs[i]\n\
         \x20   i = i + 1\n\
         \x20 }\n\
         \x20 return s\n\
         }",
    )
    .expect("a loop that may run zero times is ordinary");
}

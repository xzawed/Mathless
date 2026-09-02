//! WK1/WK2 (SPEC-string-concat §2.1, §2.2, §3-J) — what the surface accepts and refuses.
//!
//! The refusals carry as much of this slice as the acceptances. DP-K1 keeps `string + i32`
//! rejected so the "no implicit mixing" rule has no exception, and DP-K3 keeps a built string
//! out of every position that would need somewhere to live — which is the whole reason the
//! lowering can work without an allocator (§0.2).

use mlc::{compile_to_ir, ir::IrExprKind, ir::IrStmt, ir::IrType};

fn err(src: &str) -> String {
    compile_to_ir(src)
        .expect_err("should not compile")
        .to_string()
}

fn ok(src: &str) {
    compile_to_ir(src).expect("should compile");
}

// ------------------------------------------------------------------ §2.1 accepted

#[test]
fn two_strings_concatenate() {
    ok("export fn f(a: string, b: string) -> string! { return a + b }");
}

#[test]
fn a_literal_and_a_parameter_concatenate() {
    ok("export fn f(last: string, first: string) -> string! { return last + \" \" + first }");
}

#[test]
fn an_i32_becomes_a_string_with_as() {
    ok("export fn f(n: i32) -> string! { return n as string }");
}

#[test]
fn a_computed_i32_becomes_a_string() {
    // R1's shape: the number is the result of arithmetic, not a bare parameter.
    ok("export fn f(qty: i32, price: i32) -> string! { return (qty * price) as string }");
}

#[test]
fn the_measured_business_rules_compile() {
    // §0.1 R1, R2, R5 — the rules that were blocked before this slice.
    ok(
        "export fn receipt_line(item: string, qty: i32, unit_price: i32) -> string! {\n\
        return item + \" x \" + (qty as string) + \" = \" + ((qty * unit_price) as string) }",
    );
    ok("export fn full_name(first: string, last: string) -> string! { return last + \" \" + first }");
    ok("export fn address_line(city: string, district: string) -> string! { return city + \" \" + district }");
}

// ------------------------------------------------------------------ §2.1 refused (DP-K1)

#[test]
fn a_string_and_an_i32_do_not_concatenate() {
    // DP-K1. The language refuses `f64 + i32` too; this keeps that rule without an exception.
    let e = err("export fn f(a: string, n: i32) -> string! { return a + n }");
    assert!(e.contains("string") && e.contains("i32"), "{e}");
    assert!(
        e.contains("as string"),
        "the message must point at the fix, not just refuse: {e}"
    );
}

#[test]
fn a_string_and_an_f64_do_not_concatenate() {
    let e = err("export fn f(a: string, x: f64) -> string! { return a + x }");
    assert!(e.contains("string") && e.contains("f64"), "{e}");
}

#[test]
fn concatenation_is_only_addition() {
    for op in ["-", "*", "/", "%"] {
        let e = err(&format!(
            "export fn f(a: string, b: string) -> string! {{ return a {op} b }}"
        ));
        assert!(
            e.contains("string"),
            "`{op}` on strings must be refused: {e}"
        );
    }
}

// ------------------------------------------------------------------ §2.1 casts (DP-K5/K6)

#[test]
fn f64_as_string_is_out_of_scope() {
    // DP-K5, measured: decimal money reduces to integer cents, which this slice can format.
    let e = err("export fn f(x: f64) -> string! { return x as string }");
    assert!(e.contains("f64") && e.contains("string"), "{e}");
}

#[test]
fn bool_as_string_is_out_of_scope() {
    let e = err("export fn f(b: bool) -> string! { return b as string }");
    assert!(e.contains("bool"), "{e}");
}

#[test]
fn a_string_cannot_be_cast_to_a_number() {
    // Parsing text is a different slice entirely; `as` must not look like it does that.
    let e = err("export fn f(s: string) -> i32 { return s as i32 }");
    assert!(e.contains("string"), "{e}");
}

// ------------------------------------------------------------------ §2.2 position (DP-K3)

#[test]
fn a_built_string_cannot_be_bound_to_a_local() {
    // The measured diagnostic that decided the design (§0.2): there is nowhere for it to live.
    let e = err("export fn f(a: string, b: string) -> string! { let t = a + b  return t }");
    assert!(e.contains("allocator") || e.contains("local"), "{e}");
}

#[test]
fn a_built_string_cannot_be_passed_as_an_argument() {
    let e = err("fn g(s: string) -> bool { return s == \"x\" }\n\
         export fn f(a: string, b: string) -> bool { return g(a + b) }");
    assert!(!e.is_empty(), "passing a built string must be refused: {e}");
}

#[test]
fn a_built_string_cannot_be_compared() {
    // Comparison would need the bytes to exist somewhere before the buffer is known.
    let e = err("export fn f(a: string, b: string) -> bool { return (a + b) == \"xy\" }");
    assert!(!e.is_empty(), "{e}");
}

#[test]
fn a_non_fallible_string_return_is_still_refused() {
    // `-> string` (no `!`) has no way to report truncation, so it stays rejected (#92).
    let e = err("export fn f(a: string, b: string) -> string { return a + b }");
    assert!(e.contains("string"), "{e}");
}

// ------------------------------------------------------------------ WK2 the IR shape

#[test]
fn nested_concatenation_flattens_into_one_ordered_list() {
    // DP-K7/K4: codegen walks a flat list of pieces, never a tree — that is what makes the
    // two-pass length count a simple sum.
    let ir = compile_to_ir(
        "export fn f(a: string, b: string, n: i32) -> string! { return a + \" \" + b + (n as string) }",
    )
    .expect("compile");
    let f = &ir.functions[0];
    let Some(IrStmt::Return(e)) = f.body.last() else {
        panic!("expected a return, got {:?}", f.body.last());
    };
    let IrExprKind::Concat(pieces) = &e.kind else {
        panic!("expected a flattened Concat, got {:?}", e.kind);
    };
    assert_eq!(pieces.len(), 4, "four pieces, in source order: {pieces:?}");
    assert!(
        pieces.iter().all(|p| p.ty == IrType::Str),
        "every piece is a string by the time codegen sees it: {pieces:?}"
    );
    assert!(
        matches!(pieces[3].kind, IrExprKind::Cast { .. }),
        "the last piece is the i32 cast: {:?}",
        pieces[3].kind
    );
}

#[test]
fn a_single_string_is_not_wrapped_in_a_concat() {
    // #92's path must stay exactly as it was: one borrowed pointer, `ml_strout`, no change.
    let ir = compile_to_ir("export fn f(a: string) -> string! { return a }").expect("compile");
    let Some(IrStmt::Return(e)) = ir.functions[0].body.last() else {
        panic!("expected a return");
    };
    assert!(
        !matches!(e.kind, IrExprKind::Concat(_)),
        "a lone string must not become a Concat: {:?}",
        e.kind
    );
}

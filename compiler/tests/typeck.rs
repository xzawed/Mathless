//! W3 acceptance (WBS W3): AST → typed IR, and type errors are reported.

use mlc::ir::*;
use mlc::{check, parse};

fn ir_of(src: &str) -> IrModule {
    let module = parse(src).expect("parse");
    check(&module).expect("typecheck")
}

fn type_err(src: &str) -> String {
    let module = parse(src).expect("parse");
    check(&module).expect_err("should be a type error").message
}

#[test]
fn rejects_duplicate_function_names() {
    // Two `mlx_f` exports would collide as duplicate `#[no_mangle]` symbols at link time;
    // catch it early with a clear type error instead.
    let msg = type_err("export fn f() -> f64 { return 1.0 }\nexport fn f() -> f64 { return 2.0 }");
    assert!(msg.to_lowercase().contains("duplicate"), "{msg}");
    assert!(msg.contains('f'), "{msg}");
}

#[test]
fn rejects_case_insensitive_duplicate_function_names() {
    // The Delphi import unit is case-insensitive, so `foo` and `Foo` would collide there
    // even though C/Rust keep them distinct — reject case-insensitive duplicates.
    let msg =
        type_err("export fn foo() -> f64 { return 1.0 }\nexport fn Foo() -> f64 { return 2.0 }");
    assert!(msg.to_lowercase().contains("duplicate"), "{msg}");
}

#[test]
fn lowers_discount_to_typed_ir() {
    let src = include_str!("../../examples/discount.mls");

    let expected = IrModule {
        errors: vec![],
        functions: vec![IrFunction {
            name: "discount".into(),
            params: vec![
                IrParam {
                    name: "price".into(),
                    ty: IrType::F64,
                },
                IrParam {
                    name: "vip".into(),
                    ty: IrType::Bool,
                },
            ],
            ret: IrType::F64,
            fallible: false,
            body: vec![
                IrStmt::If {
                    cond: IrExpr {
                        ty: IrType::Bool,
                        kind: IrExprKind::Var("vip".into()),
                    },
                    body: vec![IrStmt::Return(IrExpr {
                        ty: IrType::F64,
                        kind: IrExprKind::Binary {
                            op: IrBinOp::Mul,
                            lhs: Box::new(IrExpr {
                                ty: IrType::F64,
                                kind: IrExprKind::Var("price".into()),
                            }),
                            rhs: Box::new(IrExpr {
                                ty: IrType::F64,
                                kind: IrExprKind::ConstF64(0.9),
                            }),
                        },
                    })],
                },
                IrStmt::Return(IrExpr {
                    ty: IrType::F64,
                    kind: IrExprKind::Var("price".into()),
                }),
            ],
        }],
    };

    assert_eq!(ir_of(src), expected);
}

#[test]
fn rejects_return_type_mismatch() {
    let msg = type_err("export fn f(x: f64) -> bool { return x }");
    assert!(msg.contains("return type"), "message was: {msg}");
}

#[test]
fn rejects_non_bool_if_condition() {
    let msg = type_err("export fn f(x: f64) -> f64 { if x { return x } return x }");
    assert!(msg.contains("if condition"), "message was: {msg}");
}

#[test]
fn rejects_unknown_variable() {
    let msg = type_err("export fn f() -> f64 { return q }");
    assert!(msg.contains("unknown variable"), "message was: {msg}");
}

#[test]
fn rejects_arithmetic_on_bool() {
    let msg = type_err("export fn f(b: bool) -> f64 { return b * 0.9 }");
    assert!(msg.contains("f64"), "message was: {msg}");
}

#[test]
fn rejects_parameter_reserved_in_rust_and_pascal() {
    // `type` is a keyword in Rust and Pascal (case-insensitive in Pascal).
    let msg = type_err("export fn f(type: f64) -> f64 { return type }");
    assert!(msg.contains("reserved"), "message was: {msg}");
    assert!(
        msg.contains("Rust") && msg.contains("Pascal"),
        "message was: {msg}"
    );
}

#[test]
fn rejects_parameter_reserved_in_c_only() {
    // `int` is a C keyword but a valid identifier in Rust/Pascal.
    let msg = type_err("export fn f(int: f64) -> f64 { return int }");
    assert!(
        msg.contains("reserved") && msg.contains("C"),
        "message was: {msg}"
    );
}

#[test]
fn rejects_parameter_reserved_in_pascal_case_insensitively() {
    // Pascal is case-insensitive, so `Begin` collides with `begin`.
    let msg = type_err("export fn f(Begin: f64) -> f64 { return Begin }");
    assert!(
        msg.contains("reserved") && msg.contains("Pascal"),
        "message was: {msg}"
    );
}

#[test]
fn accepts_ordinary_parameter_names() {
    // Sanity: normal names still typecheck (discount uses price/vip).
    assert!(
        ir_of("export fn f(price: f64, vip: bool) -> f64 { return price }")
            .functions
            .len()
            == 1
    );
}

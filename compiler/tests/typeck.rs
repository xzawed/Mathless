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
fn lowers_discount_to_typed_ir() {
    let src = include_str!("../../examples/discount.mls");

    let expected = IrModule {
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

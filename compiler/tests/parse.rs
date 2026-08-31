//! W2 acceptance (SPEC §2.1 / WBS W2): `discount.mls` → AST, and bad input → clear error.

use mlc::ast::*;
use mlc::parse;

#[test]
fn parses_discount_example() {
    let src = include_str!("../../examples/discount.mls");
    let module = parse(src).expect("discount.mls should parse");

    let expected = Module {
        errors: vec![],
        functions: vec![Function {
            name: "discount".into(),
            params: vec![
                Param {
                    name: "price".into(),
                    ty: Type::F64,
                    out: false,
                },
                Param {
                    name: "vip".into(),
                    ty: Type::Bool,
                    out: false,
                },
            ],
            ret: Type::F64,
            fallible: false,
            exported: true,
            body: vec![
                Stmt::If {
                    cond: Expr::Var("vip".into()),
                    body: vec![Stmt::Return(Expr::Binary {
                        op: BinOp::Mul,
                        lhs: Box::new(Expr::Var("price".into())),
                        rhs: Box::new(Expr::Number(0.9)),
                    })],
                },
                Stmt::Return(Expr::Var("price".into())),
            ],
        }],
    };

    assert_eq!(module, expected);
}

#[test]
fn reports_missing_arrow_with_position() {
    // no `-> <type>` before the block
    let err = parse("export fn f(x: f64) { return x }").unwrap_err();
    assert!(err.message.contains("->"), "message was: {}", err.message);
    assert!(err.line >= 1 && err.col >= 1);
}

#[test]
fn rejects_unknown_type() {
    // `i64` is not a supported type (the set is f64|bool|i32).
    let err = parse("export fn f(x: i64) -> f64 { return x }").unwrap_err();
    assert!(err.message.contains("type"), "message was: {}", err.message);
}

#[test]
fn trailing_garbage_is_not_ignored() {
    // Tokens after the last function must not be silently dropped: parse_module loops to
    // EOF, so `bogus` is treated as a (failed) next function declaration. Since `export`
    // became optional (internal `fn`), the expected token there is `fn` rather than
    // `export` — a more accurate message for the same rejection.
    let err = parse("export fn f() -> f64 { return 0 } bogus").unwrap_err();
    assert!(err.message.contains("fn"), "message was: {}", err.message);
    assert!(
        err.message.contains("bogus"),
        "message was: {}",
        err.message
    );
}

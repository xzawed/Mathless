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

/// Recursive descent has a depth, and past it the process dies instead of reporting anything.
///
/// Measured on the CLI before the guard existed (debug build, `mlc build`):
///
/// | input                       | result                                              |
/// |-----------------------------|-----------------------------------------------------|
/// | `return` + 110 nested `(`   | exit 0 — wrote `.dll` / `.h` / `.pas` / `.lib`      |
/// | `return` + 125 nested `(`   | exit 127, `thread 'main' has overflowed its stack`   |
/// | 100 nested `if` blocks      | exit 0                                              |
/// | 150 nested `if` blocks      | exit 127, same overflow                             |
///
/// No diagnostic, no line, nothing naming the input — the user is handed a compiler crash.
/// A stack overflow cannot be caught after the fact, so the only place to stop it is before
/// recursing.
///
/// Depth 110 compiling all the way through is also the evidence for where the limit sits: the
/// parser is not the only pass that walks the tree recursively (typeck, codegen and dropping
/// the `Box` chain all do), and 110 proved safe for every one of them.
#[test]
fn nesting_deeper_than_the_walker_can_handle_is_an_error_not_a_crash() {
    for (what, src) in [
        (
            "parenthesised expressions",
            format!(
                "export fn f(x: f64) -> f64 {{ return {}x{} }}",
                "(".repeat(400),
                ")".repeat(400)
            ),
        ),
        (
            "if blocks",
            format!(
                "export fn f(b: bool) -> f64 {{ {}return 1.0{} return 0.0 }}",
                "if b { ".repeat(400),
                " }".repeat(400)
            ),
        ),
    ] {
        // If the guard is gone this does not fail — it aborts the whole test binary. That is
        // the shape of the bug, and why the assertion below cannot be the only protection.
        let shown = match parse(&src) {
            Ok(_) => String::from("<it parsed>"),
            Err(e) => e.message,
        };
        assert!(
            shown.contains("nested"),
            "{what} nested past the limit must be reported: {shown}"
        );
    }
}

/// The limit must sit far above anything a person writes, or it trades a rare crash for a
/// common false rejection.
#[test]
fn ordinary_nesting_is_nowhere_near_the_limit() {
    let src = format!(
        "export fn f(x: f64) -> f64 {{ return {}x{} }}",
        "(".repeat(32),
        ")".repeat(32)
    );
    parse(&src).expect("32 levels of parentheses is ordinary and must still parse");

    let ifs = format!(
        "export fn f(b: bool) -> f64 {{ {}return 1.0{} return 0.0 }}",
        "if b { ".repeat(16),
        " }".repeat(16)
    );
    parse(&ifs).expect("16 nested `if`s is ordinary and must still parse");
}

/// The limit has to bound the **tree**, not the parser's frames — they are different
/// quantities, and #157 bounded the wrong one.
///
/// A left-associative chain builds its spine in a LOOP, so it spends one parser frame no
/// matter how long it is. Measured after #157 and before this fix, with `mlc build`:
///
/// | input | result |
/// |---|---|
/// | `return x + x + … + x`, 140 terms | exit 0 |
/// | 160 terms | **exit 127**, no diagnostic — the pre-#157 signature |
/// | 20 000 terms | **exit 127** *inside the parser*, dropping the tree it had just refused |
///
/// Isolated per pass: the parser survived 500 terms, dropping survived 500, and
/// `compile_to_ir` died at 200 — the typechecker was the killer, with codegen right behind it.
/// The 20 000 case is the sharper one: a post-parse check DID refuse it, and the refusal could
/// not be delivered, because delivering it meant freeing a 19 999-deep `Box` chain.
///
/// So the level is charged where the node is BUILT (`Parser::wrap`, the unary recursion and
/// the cast loop), not counted afterwards — nothing deep is ever constructed.
#[test]
fn a_long_chain_is_bounded_like_any_other_nesting() {
    for (what, src) in [
        (
            "a + chain",
            format!(
                "export fn f(x: f64) -> f64 {{ return {} }}",
                vec!["x"; 4000].join(" + ")
            ),
        ),
        (
            "a unary chain",
            format!(
                "export fn f(b: bool) -> bool {{ return {} b }}",
                vec!["!"; 4000].join(" ")
            ),
        ),
        (
            "a cast chain",
            format!(
                "export fn f(x: i32) -> i32 {{ return x{} }}",
                " as i32".repeat(4000)
            ),
        ),
        (
            "a comparison chain",
            format!(
                "export fn f(a: i32) -> bool {{ return {} }}",
                vec!["a"; 2000].join(" == ")
            ),
        ),
    ] {
        let err = parse(&src).expect_err(&format!("{what} past the limit must be refused"));
        assert!(
            err.message.contains("nested") || err.message.contains("deep"),
            "{what}: the refusal must name the reason: {}",
            err.message
        );
        assert!(
            err.line >= 1 && err.col >= 1,
            "{what}: and carry a position: {}:{}",
            err.line,
            err.col
        );
    }
}

/// The lengths people actually write still parse. The limit bounds NESTING, not size: a body
/// of a thousand sibling statements is depth 1 and must be untouched by any of this.
#[test]
fn ordinary_length_is_not_ordinary_depth() {
    let chain = format!(
        "export fn f(x: f64) -> f64 {{ return {} }}",
        vec!["x"; 30].join(" + ")
    );
    parse(&chain).expect("a 30-term sum is an ordinary business expression");

    let many = format!(
        "export fn f(x: f64) -> f64 {{ let mut t = x {} return t }}",
        "t = t + x ".repeat(500)
    );
    parse(&many).expect("500 sibling statements are depth 1, not depth 500");
}

//! Recursive-descent parser for the Mathless MVP subset (W2).
//!
//! Grammar (informal):
//! ```text
//! module   := function*
//! function := 'export'? 'fn' ident '(' params? ')' '->' type block
//! params   := param (',' param)*
//! param    := ident ':' type
//! type     := 'f64' | 'bool'
//! block    := '{' stmt* '}'
//! stmt     := 'if' expr block | 'while' expr block | 'return' expr | 'fail' ident
//!           | 'let' 'mut'? ident '=' expr | ident '=' expr
//! expr     := or
//! or       := and ('||' and)*
//! and      := compare ('&&' compare)*
//! compare  := add (('<'|'>'|'<='|'>='|'=='|'!=') add)*
//! add      := mul (('+'|'-') mul)*
//! mul      := unary (('*'|'/') unary)*
//! unary    := ('-'|'!') unary | cast
//! cast     := primary ('as' type)*
//! primary  := number | 'true' | 'false' | ident | ident '(' args? ')' | '(' expr ')'
//! ```

use crate::ast::*;
use crate::error::ParseError;
use crate::lexer::{Spanned, Token};

pub fn parse(tokens: Vec<Spanned>) -> Result<Module, ParseError> {
    Parser::new(tokens).parse_module()
}

struct Parser {
    toks: Vec<Spanned>,
    pos: usize,
}

impl Parser {
    fn new(toks: Vec<Spanned>) -> Self {
        Parser { toks, pos: 0 }
    }

    fn peek(&self) -> &Token {
        // Bounds-safe: the stream always ends in `Eof` and the parser stops there, but never
        // index out of range even if that invariant is somehow violated.
        static EOF: Token = Token::Eof;
        self.toks.get(self.pos).map(|s| &s.tok).unwrap_or(&EOF)
    }

    /// Look `n` tokens past the cursor. Bounds-safe like [`Parser::peek`].
    fn peek_at(&self, n: usize) -> &Token {
        static EOF: Token = Token::Eof;
        self.toks.get(self.pos + n).map(|s| &s.tok).unwrap_or(&EOF)
    }

    fn err<T>(&self, msg: impl Into<String>) -> Result<T, ParseError> {
        // Report at the current token, or the last one (the `Eof`) if `pos` is past the end;
        // `toks` is never empty (tokenize always appends `Eof`).
        let s = self.toks.get(self.pos).or_else(|| self.toks.last());
        let (line, col) = s.map(|s| (s.line, s.col)).unwrap_or((0, 0));
        Err(ParseError::new(msg, line, col))
    }

    /// Consume `want` or produce "expected {what}, found {token}".
    fn eat(&mut self, want: &Token, what: &str) -> Result<(), ParseError> {
        if self.peek() == want {
            self.pos += 1;
            Ok(())
        } else {
            self.err(format!("expected {what}, found {:?}", self.peek()))
        }
    }

    fn ident(&mut self, what: &str) -> Result<String, ParseError> {
        match self.peek().clone() {
            Token::Ident(s) => {
                self.pos += 1;
                Ok(s)
            }
            // Keywords are the common near-miss here (`fn f(mut: f64)`, `let let = 1`), and
            // they never reach the reserved-word check in `reserved.rs` because the lexer
            // already claimed them. Name the keyword instead of dumping the token variant.
            other => match other.keyword_text() {
                Some(kw) => self.err(format!(
                    "expected {what}, found keyword `{kw}` — keywords cannot be used as names"
                )),
                None => self.err(format!("expected {what}, found {other:?}")),
            },
        }
    }

    fn parse_module(&mut self) -> Result<Module, ParseError> {
        let mut functions = Vec::new();
        let mut errors = Vec::new();
        while *self.peek() != Token::Eof {
            if *self.peek() == Token::Error {
                errors.push(self.parse_error_decl()?);
            } else {
                functions.push(self.parse_function()?);
            }
        }
        Ok(Module { functions, errors })
    }

    /// `error NAME = N` — N must be a positive integer (Q13: 0 is OK, negatives reserved).
    fn parse_error_decl(&mut self) -> Result<ErrorDecl, ParseError> {
        self.eat(&Token::Error, "'error'")?;
        let name = self.ident("error code name")?;
        self.eat(&Token::Assign, "'='")?;
        match self.peek().clone() {
            Token::Int(i) if i > 0 && i <= i32::MAX as i64 => {
                self.pos += 1;
                Ok(ErrorDecl {
                    name,
                    code: i as i32,
                })
            }
            other => self.err(format!(
                "error code must be a positive integer (1..={}), found {other:?}",
                i32::MAX
            )),
        }
    }

    fn parse_function(&mut self) -> Result<Function, ParseError> {
        // `export fn …` is visible to hosts; a bare `fn …` is internal to the module and
        // never reaches the export table (SPEC-calls section 2.3).
        let exported = if self.peek() == &Token::Export {
            self.pos += 1;
            true
        } else {
            false
        };
        self.eat(&Token::Fn, "'fn'")?;
        let name = self.ident("function name")?;
        self.eat(&Token::LParen, "'('")?;

        let mut params = Vec::new();
        if *self.peek() != Token::RParen {
            loop {
                let pname = self.ident("parameter name")?;
                self.eat(&Token::Colon, "':'")?;
                let ty = self.parse_type()?;
                params.push(Param { name: pname, ty });
                if *self.peek() == Token::Comma {
                    self.pos += 1;
                } else {
                    break;
                }
            }
        }
        self.eat(&Token::RParen, "')'")?;
        self.eat(&Token::Arrow, "'->'")?;
        let ret = self.parse_type()?;
        // `-> T!` marks the function fallible (D17 ABI = i32 status + out-param).
        let fallible = if *self.peek() == Token::Bang {
            self.pos += 1;
            true
        } else {
            false
        };
        let body = self.parse_block()?;
        Ok(Function {
            name,
            params,
            ret,
            fallible,
            exported,
            body,
        })
    }

    fn parse_type(&mut self) -> Result<Type, ParseError> {
        match self.peek().clone() {
            Token::Ident(s) if s == "f64" => {
                self.pos += 1;
                Ok(Type::F64)
            }
            Token::Ident(s) if s == "bool" => {
                self.pos += 1;
                Ok(Type::Bool)
            }
            Token::Ident(s) if s == "i32" => {
                self.pos += 1;
                Ok(Type::I32)
            }
            other => self.err(format!("expected type (f64|bool|i32), found {other:?}")),
        }
    }

    fn parse_block(&mut self) -> Result<Vec<Stmt>, ParseError> {
        self.eat(&Token::LBrace, "'{'")?;
        let mut stmts = Vec::new();
        while *self.peek() != Token::RBrace {
            if *self.peek() == Token::Eof {
                return self.err("unexpected end of input inside block (missing '}')");
            }
            stmts.push(self.parse_stmt()?);
        }
        self.eat(&Token::RBrace, "'}'")?;
        Ok(stmts)
    }

    fn parse_stmt(&mut self) -> Result<Stmt, ParseError> {
        match self.peek() {
            Token::If => {
                self.pos += 1;
                let cond = self.parse_expr()?;
                let body = self.parse_block()?;
                Ok(Stmt::If { cond, body })
            }
            Token::While => {
                self.pos += 1;
                let cond = self.parse_expr()?;
                let body = self.parse_block()?;
                Ok(Stmt::While { cond, body })
            }
            Token::Return => {
                self.pos += 1;
                let e = self.parse_expr()?;
                Ok(Stmt::Return(e))
            }
            Token::Fail => {
                self.pos += 1;
                let code = self.ident("error code name")?;
                Ok(Stmt::Fail(code))
            }
            Token::Let => {
                self.pos += 1;
                // `let mut NAME` (DP-M1): one declaration keyword, `mut` as a modifier.
                let mutable = if self.peek() == &Token::Mut {
                    self.pos += 1;
                    true
                } else {
                    false
                };
                let name = self.ident("variable name")?;
                self.eat(&Token::Assign, "'='")?;
                let value = self.parse_expr()?;
                Ok(Stmt::Let {
                    name,
                    value,
                    mutable,
                })
            }
            // Assignment is the only statement that starts with an identifier, so one token
            // of lookahead (`ident` then `=`) is enough to tell it from a stray expression.
            Token::Ident(_) if self.peek_at(1) == &Token::Assign => {
                let name = self.ident("variable name")?;
                self.pos += 1; // '='
                let value = self.parse_expr()?;
                Ok(Stmt::Assign { name, value })
            }
            other => self.err(format!(
                "expected statement (if|while|return|fail|let|assignment), found {other:?}"
            )),
        }
    }

    fn parse_expr(&mut self) -> Result<Expr, ParseError> {
        self.parse_or()
    }

    /// `or := and ('||' and)*` — the loosest binding level.
    fn parse_or(&mut self) -> Result<Expr, ParseError> {
        let mut lhs = self.parse_and()?;
        while self.peek() == &Token::OrOr {
            self.pos += 1;
            let rhs = self.parse_and()?;
            lhs = Expr::Binary {
                op: BinOp::Or,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
            };
        }
        Ok(lhs)
    }

    /// `and := compare ('&&' compare)*` — tighter than `||`, looser than comparison, as in
    /// every C-family language (D15, SPEC-logical-ops DP-B4).
    fn parse_and(&mut self) -> Result<Expr, ParseError> {
        let mut lhs = self.parse_compare()?;
        while self.peek() == &Token::AndAnd {
            self.pos += 1;
            let rhs = self.parse_compare()?;
            lhs = Expr::Binary {
                op: BinOp::And,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
            };
        }
        Ok(lhs)
    }

    fn parse_compare(&mut self) -> Result<Expr, ParseError> {
        let mut lhs = self.parse_add()?;
        loop {
            let op = match self.peek() {
                Token::Lt => BinOp::Lt,
                Token::Gt => BinOp::Gt,
                Token::Le => BinOp::Le,
                Token::Ge => BinOp::Ge,
                Token::EqEq => BinOp::Eq,
                Token::Ne => BinOp::Ne,
                _ => break,
            };
            self.pos += 1;
            let rhs = self.parse_add()?;
            lhs = Expr::Binary {
                op,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
            };
        }
        Ok(lhs)
    }

    fn parse_add(&mut self) -> Result<Expr, ParseError> {
        let mut lhs = self.parse_mul()?;
        loop {
            let op = match self.peek() {
                Token::Plus => BinOp::Add,
                Token::Minus => BinOp::Sub,
                _ => break,
            };
            self.pos += 1;
            let rhs = self.parse_mul()?;
            lhs = Expr::Binary {
                op,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
            };
        }
        Ok(lhs)
    }

    /// `unary := ('-' | '!') unary | cast` — binds tighter than `*` and `/`, and is
    /// right-recursive so `- -x` and `!!b` parse (SPEC-unary DP-U3).
    fn parse_unary(&mut self) -> Result<Expr, ParseError> {
        let op = match self.peek() {
            Token::Minus => UnOp::Neg,
            Token::Bang => UnOp::Not,
            _ => return self.parse_cast(),
        };
        self.pos += 1;
        let operand = self.parse_unary()?;
        Ok(Expr::Unary {
            op,
            operand: Box::new(operand),
        })
    }

    /// `cast := primary ('as' type)*` — binds tighter than unary, so `-x as f64` is
    /// `-(x as f64)`. Left-associative, so `x as i32 as f64` chains.
    fn parse_cast(&mut self) -> Result<Expr, ParseError> {
        let mut e = self.parse_primary()?;
        while self.peek() == &Token::As {
            self.pos += 1;
            let to = self.parse_type()?;
            e = Expr::Cast {
                to,
                operand: Box::new(e),
            };
        }
        Ok(e)
    }

    fn parse_mul(&mut self) -> Result<Expr, ParseError> {
        let mut lhs = self.parse_unary()?;
        loop {
            let op = match self.peek() {
                Token::Star => BinOp::Mul,
                Token::Slash => BinOp::Div,
                _ => break,
            };
            self.pos += 1;
            let rhs = self.parse_unary()?;
            lhs = Expr::Binary {
                op,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
            };
        }
        Ok(lhs)
    }

    fn parse_primary(&mut self) -> Result<Expr, ParseError> {
        match self.peek().clone() {
            Token::Number(n) => {
                self.pos += 1;
                Ok(Expr::Number(n))
            }
            Token::Int(n) => {
                self.pos += 1;
                Ok(Expr::Int(n))
            }
            Token::True => {
                self.pos += 1;
                Ok(Expr::Bool(true))
            }
            Token::False => {
                self.pos += 1;
                Ok(Expr::Bool(false))
            }
            Token::Ident(s) => {
                self.pos += 1;
                // `name(` is a call; a bare `name` is a variable.
                if self.peek() == &Token::LParen {
                    self.pos += 1;
                    let mut args = Vec::new();
                    if self.peek() != &Token::RParen {
                        loop {
                            args.push(self.parse_expr()?);
                            if self.peek() == &Token::Comma {
                                self.pos += 1;
                                continue;
                            }
                            break;
                        }
                    }
                    self.eat(&Token::RParen, "')' to close the argument list")?;
                    return Ok(Expr::Call { name: s, args });
                }
                Ok(Expr::Var(s))
            }
            Token::LParen => {
                self.pos += 1;
                let e = self.parse_expr()?;
                self.eat(&Token::RParen, "')'")?;
                Ok(e)
            }
            other => self.err(format!("expected expression, found {other:?}")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::tokenize;

    #[test]
    fn parses_let_mut_and_assignment_as_distinct_statements() {
        // WM2: `let mut` sets the mutable flag; a bare `ident =` is an assignment statement.
        let m =
            parse(tokenize("export fn f() -> f64 { let mut x = 1.0 x = 2.0 return x }").unwrap())
                .expect("parse");
        let body = &m.functions[0].body;
        assert!(
            matches!(&body[0], Stmt::Let { name, mutable: true, .. } if name == "x"),
            "{body:?}"
        );
        assert!(
            matches!(&body[1], Stmt::Assign { name, .. } if name == "x"),
            "{body:?}"
        );
        // Without `mut` the same declaration is immutable.
        let m = parse(tokenize("export fn f() -> f64 { let x = 1.0 return x }").unwrap()).unwrap();
        assert!(
            matches!(&m.functions[0].body[0], Stmt::Let { mutable: false, .. }),
            "plain `let` stays immutable"
        );
    }

    #[test]
    fn an_identifier_that_is_not_followed_by_assign_is_not_a_statement() {
        // One token of lookahead: `x` alone must not be mistaken for an assignment.
        let err = parse(tokenize("export fn f() -> f64 { x return 1.0 }").unwrap()).unwrap_err();
        assert!(format!("{err:?}").contains("expected statement"), "{err:?}");
    }

    #[test]
    fn parses_while_as_a_statement_with_a_block_body() {
        // WW2: same shape as `if` — condition without parens, block body.
        let m = parse(
            tokenize("export fn f(b: bool) -> i32 { while b { let x = 1 } return 0 }").unwrap(),
        )
        .expect("parse");
        let body = &m.functions[0].body;
        assert!(
            matches!(&body[0], Stmt::While { body, .. } if body.len() == 1),
            "{body:?}"
        );
    }

    #[test]
    fn a_keyword_used_as_a_name_is_named_in_the_error() {
        // A keyword never reaches `reserved.rs` (the lexer claimed it), so the parser has to
        // say why. Applies to every keyword, not just the newly-added `mut`.
        for (src, kw) in [
            ("export fn f(mut: f64) -> f64 { return 0.0 }", "mut"),
            ("export fn f(let: f64) -> f64 { return 0.0 }", "let"),
            ("export fn f() -> f64 { let if = 1.0 return 1.0 }", "if"),
            ("export fn f(as: f64) -> f64 { return 0.0 }", "as"),
        ] {
            let err = parse(tokenize(src).unwrap()).unwrap_err();
            let msg = format!("{err:?}");
            assert!(
                msg.contains(&format!("keyword `{kw}`")),
                "should name the keyword: {msg}"
            );
        }
        // `let mut = 1.0` is a *missing name*, not a keyword-as-name: `mut` was consumed as
        // the mutability modifier. The diagnostic should stay the plain one.
        let err = parse(tokenize("export fn f() -> f64 { let mut = 1.0 return 1.0 }").unwrap())
            .unwrap_err();
        assert!(
            format!("{err:?}").contains("expected variable name"),
            "{err:?}"
        );
    }

    #[test]
    fn peek_and_err_are_bounds_safe_past_the_end() {
        // Defensive: the token stream always ends in `Eof` and the parser stops there, but an
        // over-advanced `pos` must not index out of bounds / panic.
        let mut p = Parser::new(tokenize("export fn f() -> f64 { return 1.0 }").unwrap());
        p.pos = 9999;
        assert_eq!(*p.peek(), Token::Eof, "peek past the end returns Eof");
        // `eat` mismatches → calls `err`, which must also be bounds-safe (not index `pos`).
        let r: Result<(), ParseError> = p.eat(&Token::Fn, "'fn'");
        assert!(r.is_err(), "eat past the end errors instead of panicking");
    }
}

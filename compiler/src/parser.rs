//! Recursive-descent parser for the Mathless MVP subset (W2).
//!
//! Grammar (informal). This block is the contract a reader checks the parser against, so it
//! has to be the grammar the file parses — it had drifted on five productions: `type` still
//! listed only `f64|bool` (i32 and string have both landed since), `module` did not mention
//! `error`, `function` did not mention the `!` that makes it fallible, `stmt` did not mention
//! any of the three `try` forms, and `primary` did not mention string literals.
//!
//! ```text
//! module    := (function | error_decl)*
//! error_decl:= 'error' ident '=' int_literal            … 1..=i32::MAX (Q13)
//! function  := 'export'? 'fn' ident '(' params? ')' '->' type '!'? block
//!                                                      … '!' = fallible (D17)
//! params    := param (',' param)*
//! param     := 'out'? ident ':' type
//! type      := 'f64' | 'bool' | 'i32' | 'string'
//! block     := '{' stmt* '}'
//! stmt      := 'if' expr block | 'while' expr block
//!            | 'return' expr | 'return' try_call
//!            | 'fail' ident
//!            | 'let' 'mut'? ident '=' (expr | try_call)
//!            | ident '=' (expr | try_call)
//! try_call  := 'try' ident '(' args? ')'
//! expr      := or
//! or        := and ('||' and)*
//! and       := compare ('&&' compare)*
//! compare   := add (('<'|'>'|'<='|'>='|'=='|'!=') add)*
//! add       := mul (('+'|'-') mul)*
//! mul       := cast (('*'|'/'|'%') cast)*
//! cast      := unary ('as' type)*
//! unary     := ('-'|'!') unary | primary
//! primary   := number | int_literal | string_literal | 'true' | 'false'
//!            | ident | ident '(' args? ')' | '(' expr ')'
//! args      := expr (',' expr)*
//! ```
//!
//! Not in the grammar and enforced elsewhere, because they are not shape questions: the
//! nesting limit ([`MAX_NESTING`]), and every rule about which of these forms is legal WHERE
//! (`typeck`) — an `out` parameter is export-only, a `string` return demands `!`, a `try`
//! caller must itself be fallible.

use crate::ast::*;
use crate::error::ParseError;
use crate::lexer::{Spanned, Token};

pub fn parse(tokens: Vec<Spanned>) -> Result<Module, ParseError> {
    Parser::new(tokens).parse_module()
}

/// How deep expressions and blocks may nest.
///
/// Measured on the CLI before this limit existed (debug build): `return` with 110 nested
/// parentheses compiled all the way to a `.dll`, 125 aborted the process with
/// `thread 'main' has overflowed its stack` and exit 127 — no diagnostic, no position.
/// 100 nested `if` blocks compiled; 150 aborted the same way.
///
/// The limit is well under that cliff on purpose. The parser is not the only pass that walks
/// the tree recursively — typecheck, codegen and even dropping the `Box` chain do — so a tree
/// the parser accepts has to be safe for all of them. Depth 110 was measured safe end to end,
/// which is the evidence for 64 being safe; 64 is also far above anything hand-written
/// (`((a+b)*c)` is 3), so it does not trade a rare crash for a common false rejection.
const MAX_NESTING: u32 = 64;

struct Parser {
    toks: Vec<Spanned>,
    pos: usize,
    /// Current recursive-descent depth — see [`MAX_NESTING`].
    depth: u32,
}

impl Parser {
    fn new(toks: Vec<Spanned>) -> Self {
        Parser {
            toks,
            pos: 0,
            depth: 0,
        }
    }

    /// Descend one level, or refuse before the stack runs out.
    ///
    /// Expressions and blocks share one counter because the stack does: a `return` inside 40
    /// nested `if`s inside 30 parentheses costs 70 frames regardless of which construct spent
    /// them. Every caller pairs this with `self.depth -= 1` on the way out, including the
    /// error path, so a rejected parse does not leave the counter raised.
    fn enter(&mut self) -> Result<(), ParseError> {
        if self.depth >= MAX_NESTING {
            return self.err(format!(
                "expressions and blocks may not be nested more than {MAX_NESTING} levels deep \
                 — every pass of the compiler walks the tree recursively, so a deeper one would \
                 exhaust the stack instead of producing a message like this one"
            ));
        }
        self.depth += 1;
        Ok(())
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
            // A declaration form the language does not have yet reads as an ordinary
            // identifier here, so the default message is `expected 'fn', found
            // Ident("struct")` — a Debug dump that names the token and not the gap. Measured
            // on `struct P { x: i32 }`; the same applies to `const`, `import` and friends.
            //
            // Scoped to the `fn` expectation, which is the top-level item position. `eat` is
            // shared, so without this an `Ident("struct")` where a `)` was expected would be
            // told "the top level takes `export fn`…" — a confident wrong answer, and this
            // change exists to stop giving those (Grok verify).
            if want == &Token::Fn {
                if let Token::Ident(name) = self.peek() {
                    if let Some(gap) = Self::unsupported_declaration(name) {
                        let msg = format!(
                            "{gap} are not in Mathless yet — the top level takes `export fn`, \
                             `fn` or `error`"
                        );
                        return self.err(msg);
                    }
                }
            }
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

    /// The plural noun for a declaration keyword the language does not have, or `None`.
    ///
    /// Deliberately a short list of forms a user coming from C#, Pascal or Rust would
    /// actually type. It is not a reserved-word list — these stay legal as ordinary names
    /// (`fn struct(...)` is fine since #101); this only improves the message when one of them
    /// appears where a top-level item was expected.
    fn unsupported_declaration(name: &str) -> Option<&'static str> {
        match name {
            "struct" | "record" => Some("struct declarations"),
            "class" | "interface" => Some("classes"),
            "enum" => Some("enums"),
            "const" => Some("constant declarations"),
            "import" | "uses" => Some("imports"),
            "type" => Some("type aliases"),
            _ => None,
        }
    }

    /// The plural noun for a control-flow form the language does not have, or `None`.
    ///
    /// Same shape and same caveat as [`Self::unsupported_declaration`]: these stay legal as
    /// ordinary names, and this only improves the message where a statement was expected.
    fn unsupported_statement(name: &str) -> Option<&'static str> {
        match name {
            "for" | "foreach" => Some("`for` loops"),
            "else" => Some("`else` branches"),
            "break" => Some("`break` statements"),
            "continue" => Some("`continue` statements"),
            "switch" | "match" | "case" => Some("`switch`/`match` statements"),
            "do" | "repeat" => Some("`do`/`repeat` loops"),
            _ => None,
        }
    }

    /// `error NAME = N` — N must be **strictly positive**, and the code enforces `i > 0`.
    ///
    /// This said "0 is OK", which read as a rule about N and is not one. D17/Q13 splits the
    /// whole i32 status space: `0` **is** success, positive is a module-defined domain error,
    /// negative is reserved for the runtime and the ABI. So `0` is a perfectly good *status* —
    /// and for that exact reason it cannot also be a declared error. Measured: `error E = 0`
    /// is refused with "error code must be a positive integer (1..=2147483647)".
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
                // `out p: T` — the marker sits before the name, as in Pascal and C#. The
                // generated Delphi unit already spells `out` for D17's implicit out-param,
                // so the surface and that binding agree.
                let out = if *self.peek() == Token::Out {
                    self.pos += 1;
                    true
                } else {
                    false
                };
                let pname = self.ident("parameter name")?;
                self.eat(&Token::Colon, "':'")?;
                let ty = self.parse_type()?;
                params.push(Param {
                    name: pname,
                    ty,
                    out,
                });
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
            // `string` is a type NAME, not a keyword, so it stays usable as an identifier —
            // the same choice DP-R1 made for the rounding builtins.
            Token::Ident(s) if s == "string" => {
                self.pos += 1;
                Ok(Type::Str)
            }
            other => self.err(format!(
                "expected type (f64|bool|i32|string), found {other:?}"
            )),
        }
    }

    fn parse_block(&mut self) -> Result<Vec<Stmt>, ParseError> {
        // The other re-entry point: an `if` or `while` body is a block, so nesting them
        // recurses here. Counted against the same budget as expressions (see `enter`).
        self.enter()?;
        let stmts = self.parse_block_body();
        self.depth -= 1;
        stmts
    }

    fn parse_block_body(&mut self) -> Result<Vec<Stmt>, ParseError> {
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
                if self.peek() == &Token::Try {
                    let (callee, args) = self.parse_try_call()?;
                    return Ok(Stmt::TryCall {
                        dest: TryDest::Return,
                        callee,
                        args,
                    });
                }
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
                if self.peek() == &Token::Try {
                    let (callee, args) = self.parse_try_call()?;
                    return Ok(Stmt::TryCall {
                        dest: TryDest::Let { name, mutable },
                        callee,
                        args,
                    });
                }
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
                if self.peek() == &Token::Try {
                    let (callee, args) = self.parse_try_call()?;
                    return Ok(Stmt::TryCall {
                        dest: TryDest::Assign(name),
                        callee,
                        args,
                    });
                }
                let value = self.parse_expr()?;
                Ok(Stmt::Assign { name, value })
            }
            // Control flow the language does not have yet arrives here as a plain identifier,
            // so the fallback below names the token instead of the gap — the same defect the
            // top level had (#144). `for i in 0..3` was worse still: lexing died on the `..`
            // first, so the user heard about ranges rather than about `for`.
            Token::Ident(name) if Self::unsupported_statement(name).is_some() => {
                let gap = Self::unsupported_statement(name).expect("checked in the guard");
                self.err(format!(
                    "{gap} are not in Mathless yet — a statement is if, while, return, fail, \
                     let, or an assignment"
                ))
            }
            other => self.err(format!(
                "expected statement (if|while|return|fail|let|assignment), found {other:?}"
            )),
        }
    }

    /// `try IDENT ( args )` — the ONLY shape `try` accepts (DP-F2/DP-F3).
    ///
    /// The grammar, not the typechecker, is what forbids `try` inside an expression: no AST
    /// node exists for a try-call, so `1 + try f(x)`, `f(try g(x))` and `if try f(x)` cannot be
    /// built. That is deliberate — it makes the `i32` division guard's right-operand-first
    /// evaluation and the `&&` short-circuit hazard unreachable rather than merely tested.
    ///
    /// Only a CALL may follow. `try x` or `try (a + b)` is refused here with a message that
    /// says what `try` is for, rather than failing later as a type error about something else.
    fn parse_try_call(&mut self) -> Result<(String, Vec<Expr>), ParseError> {
        self.pos += 1; // `try`
        let Token::Ident(_) = self.peek() else {
            return self.err(format!(
                "`try` must be followed by a call to a fallible function, as in \
                 `let x = try f(a)` — found {:?}",
                self.peek()
            ));
        };
        let callee = self.ident("function name")?;
        if self.peek() != &Token::LParen {
            return self.err(format!(
                "`try {callee}` must be a call — write `try {callee}(…)`. `try` marks a call \
                 whose failure exits this function; it is not a prefix on a value"
            ));
        }
        self.pos += 1; // '('
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
        self.eat(&Token::RParen, "')'")?;
        Ok((callee, args))
    }

    fn parse_expr(&mut self) -> Result<Expr, ParseError> {
        // The one re-entry point for nested expressions: `(e)`, a call argument and a `try`
        // argument all come back through here, and the precedence chain below it does not
        // recurse into itself.
        self.enter()?;
        let e = self.parse_or();
        self.depth -= 1;
        e
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

    /// `unary := ('-' | '!') unary | primary` — the tightest layer above `primary`, and
    /// right-recursive so `- -x` and `!!b` parse (SPEC-unary DP-U3).
    fn parse_unary(&mut self) -> Result<Expr, ParseError> {
        let op = match self.peek() {
            Token::Minus => UnOp::Neg,
            Token::Bang => UnOp::Not,
            _ => return self.parse_primary(),
        };
        self.pos += 1;
        let operand = self.parse_unary()?;
        Ok(Expr::Unary {
            op,
            operand: Box::new(operand),
        })
    }

    /// `cast := unary ('as' type)*` — binds LOOSER than unary and tighter than `*` and `/`,
    /// so `-x as f64` is `(-x) as f64` and `a as f64 * b` casts only `a`. This is the
    /// Rust/C#/Kotlin layering (DP-N1, reversed 2026-08-31): the old Mathless binding put
    /// `as` below unary, which agreed everywhere except `i32::MIN` — where it silently gave
    /// the opposite sign. Left-associative, so `x as i32 as f64` chains.
    fn parse_cast(&mut self) -> Result<Expr, ParseError> {
        let mut e = self.parse_unary()?;
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
        let mut lhs = self.parse_cast()?;
        loop {
            let op = match self.peek() {
                Token::Star => BinOp::Mul,
                Token::Slash => BinOp::Div,
                Token::Percent => BinOp::Rem,
                _ => break,
            };
            self.pos += 1;
            let rhs = self.parse_cast()?;
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
            Token::Str(s) => {
                self.pos += 1;
                Ok(Expr::Str(s))
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

//! Recursive-descent parser for the Mathless MVP subset (W2).
//!
//! Grammar (informal):
//! ```text
//! module   := function*
//! function := 'export' 'fn' ident '(' params? ')' '->' type block
//! params   := param (',' param)*
//! param    := ident ':' type
//! type     := 'f64' | 'bool'
//! block    := '{' stmt* '}'
//! stmt     := 'if' expr block | 'return' expr
//! expr     := compare
//! compare  := add (('<'|'>'|'<='|'>='|'=='|'!=') add)*
//! add      := mul (('+'|'-') mul)*
//! mul      := primary (('*'|'/') primary)*
//! primary  := number | 'true' | 'false' | ident | '(' expr ')'
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
        &self.toks[self.pos].tok
    }

    fn err<T>(&self, msg: impl Into<String>) -> Result<T, ParseError> {
        let s = &self.toks[self.pos];
        Err(ParseError::new(msg, s.line, s.col))
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
            other => self.err(format!("expected {what}, found {other:?}")),
        }
    }

    fn parse_module(&mut self) -> Result<Module, ParseError> {
        let mut functions = Vec::new();
        while *self.peek() != Token::Eof {
            functions.push(self.parse_function()?);
        }
        Ok(Module { functions })
    }

    fn parse_function(&mut self) -> Result<Function, ParseError> {
        self.eat(&Token::Export, "'export'")?;
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
        let body = self.parse_block()?;
        Ok(Function {
            name,
            params,
            ret,
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
            other => self.err(format!("expected type (f64|bool), found {other:?}")),
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
            Token::Return => {
                self.pos += 1;
                let e = self.parse_expr()?;
                Ok(Stmt::Return(e))
            }
            other => self.err(format!("expected statement (if|return), found {other:?}")),
        }
    }

    fn parse_expr(&mut self) -> Result<Expr, ParseError> {
        self.parse_compare()
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

    fn parse_mul(&mut self) -> Result<Expr, ParseError> {
        let mut lhs = self.parse_primary()?;
        loop {
            let op = match self.peek() {
                Token::Star => BinOp::Mul,
                Token::Slash => BinOp::Div,
                _ => break,
            };
            self.pos += 1;
            let rhs = self.parse_primary()?;
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

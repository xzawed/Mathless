//! Mathless AST — Phase 1 MVP subset (D15): `export fn`, `f64`/`bool`, `if`, `return`,
//! arithmetic and comparison expressions.

#[derive(Debug, PartialEq)]
pub struct Module {
    pub functions: Vec<Function>,
    /// Module-scoped error-code declarations (`error NAME = N`), for fallible functions (D17).
    pub errors: Vec<ErrorDecl>,
}

/// `error NAME = N` — a module-defined domain error code (Q13: positive i32).
#[derive(Debug, PartialEq)]
pub struct ErrorDecl {
    pub name: String,
    pub code: i32,
}

#[derive(Debug, PartialEq)]
pub struct Function {
    pub name: String,
    pub params: Vec<Param>,
    pub ret: Type,
    /// `-> T!` — the function may `fail`; it lowers to the D17 ABI (i32 status + out-param).
    pub fallible: bool,
    pub body: Vec<Stmt>,
}

#[derive(Debug, PartialEq)]
pub struct Param {
    pub name: String,
    pub ty: Type,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Type {
    F64,
    Bool,
}

#[derive(Debug, PartialEq)]
pub enum Stmt {
    /// `if <cond> { <body> }` (no `else` in the MVP subset).
    If { cond: Expr, body: Vec<Stmt> },
    /// `return <expr>`.
    Return(Expr),
    /// `fail <CODE>` — fail with a declared error code (only in a fallible function).
    Fail(String),
    /// `let <NAME> = <EXPR>` — an immutable, block-scoped local binding.
    Let { name: String, value: Expr },
}

#[derive(Debug, PartialEq)]
pub enum Expr {
    Number(f64),
    Bool(bool),
    Var(String),
    Binary {
        op: BinOp,
        lhs: Box<Expr>,
        rhs: Box<Expr>,
    },
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Lt,
    Gt,
    Le,
    Ge,
    Eq,
    Ne,
}

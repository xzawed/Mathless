//! Typed, backend-independent IR (W3).
//!
//! Per **D19 / Q11** this IR is deliberately **not** Rust and not Object-Pascal source —
//! it is an independent typed tree so a future C-emit backend stays possible. The W4
//! codegen lowers *this* to `no_std` + `extern "C"` + `repr(C)` Rust.
//!
//! Every [`IrExpr`] carries its resolved [`IrType`], so the backend never re-infers types.

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum IrType {
    F64,
    Bool,
    I32,
}

impl std::fmt::Display for IrType {
    /// The **surface** spelling. Diagnostics quote what the user wrote (`f64`), not the
    /// Rust variant name (`F64`).
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            IrType::F64 => "f64",
            IrType::Bool => "bool",
            IrType::I32 => "i32",
        })
    }
}

#[derive(Debug, PartialEq)]
pub struct IrModule {
    pub functions: Vec<IrFunction>,
    /// Module-defined error codes (D17), used by the header/unit generators and resolved
    /// into [`IrStmt::Fail`] by the typechecker.
    pub errors: Vec<IrErrorDecl>,
}

#[derive(Debug, PartialEq)]
pub struct IrErrorDecl {
    pub name: String,
    pub code: i32,
}

#[derive(Debug, PartialEq)]
pub struct IrFunction {
    pub name: String,
    pub params: Vec<IrParam>,
    pub ret: IrType,
    /// Fallible (`-> T!`): lowers to `int32 status` return + a `*mut T` out-param (D17).
    pub fallible: bool,
    pub body: Vec<IrStmt>,
}

#[derive(Debug, PartialEq)]
pub struct IrParam {
    pub name: String,
    pub ty: IrType,
}

#[derive(Debug, PartialEq)]
pub enum IrStmt {
    If {
        cond: IrExpr,
        body: Vec<IrStmt>,
    },
    /// `while <cond> { <body> }`. Deliberately **not** a terminator (see
    /// [`block_always_returns`]): the body may run zero times.
    While {
        cond: IrExpr,
        body: Vec<IrStmt>,
    },
    Return(IrExpr),
    /// `fail` with the resolved positive error code (only in a fallible function).
    Fail(i32),
    /// `let <name> = <value>` — a local binding; `mutable` lowers to Rust `let mut`.
    Let {
        name: String,
        value: IrExpr,
        mutable: bool,
    },
    /// `<name> = <value>` — reassign a mutable local (already checked in scope, mutable,
    /// and type-compatible by the typechecker).
    Assign {
        name: String,
        value: IrExpr,
    },
}

/// A type-annotated expression.
#[derive(Debug, PartialEq)]
pub struct IrExpr {
    pub ty: IrType,
    pub kind: IrExprKind,
}

#[derive(Debug, PartialEq)]
pub enum IrExprKind {
    ConstF64(f64),
    ConstI32(i32),
    ConstBool(bool),
    /// Reference to a parameter/local by name.
    Var(String),
    Binary {
        op: IrBinOp,
        lhs: Box<IrExpr>,
        rhs: Box<IrExpr>,
    },
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum IrBinOp {
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

/// Whether a statement list is guaranteed to exit the function: its last statement is a
/// `return` or (in a fallible function) a `fail`. An `if` without an `else` can fall through,
/// and a `while` may run zero times, so a well-formed body must end in one of these. Shared
/// by the typechecker (frontend error) and codegen (backend safety net for directly-built IR).
///
/// `while true { … }` is NOT special-cased: proving it diverges would need constant folding
/// plus divergence typing, and `while 1 == 1` would immediately fall outside whatever rule we
/// wrote (SPEC-while DP-W2).
pub fn block_always_returns(body: &[IrStmt]) -> bool {
    matches!(body.last(), Some(IrStmt::Return(_) | IrStmt::Fail(_)))
}

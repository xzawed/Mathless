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
    Return(IrExpr),
    /// `fail` with the resolved positive error code (only in a fallible function).
    Fail(i32),
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

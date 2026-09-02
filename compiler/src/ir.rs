//! Typed, backend-independent IR (W3).
//!
//! Per **D19 / Q11** this IR is deliberately **not** Rust and not Object-Pascal source —
//! it is an independent typed tree so a future C-emit backend stays possible. The W4
//! codegen lowers *this* to `no_std` + `extern "C"` + `repr(C)` Rust.
//!
//! Every [`IrExpr`] carries its resolved [`IrType`], so the backend never re-infers types.

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum IrType {
    /// Lowered as `*const u8`: borrowed for the call (D16 rule 1), never owned.
    Str,
    F64,
    Bool,
    I32,
}

impl std::fmt::Display for IrType {
    /// The **surface** spelling. Diagnostics quote what the user wrote (`f64`), not the
    /// Rust variant name (`F64`).
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            IrType::Str => "string",
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
    /// Exported to hosts. Internal functions are emitted as plain Rust functions, so they
    /// stay out of the export table and out of the generated bindings.
    pub exported: bool,
    pub body: Vec<IrStmt>,
}

#[derive(Debug, PartialEq)]
pub struct IrParam {
    pub name: String,
    pub ty: IrType,
    /// Lowered as `*mut T` and written through, exactly like D17's implicit `out_value`.
    /// Declared outs keep source order; `out_value` is appended after all of them (DP-O1).
    pub out: bool,
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
    /// `<name> = <value>` where `<name>` is an `out` parameter — the same surface statement,
    /// but it lowers to a write THROUGH a pointer. Kept as a separate variant so codegen
    /// cannot confuse the two: emitting a plain `name = …` for an out-param would assign the
    /// pointer itself and silently drop the value.
    AssignOut {
        name: String,
        value: IrExpr,
    },
    /// `<dest> = try <callee>(<args>)` — call a fallible internal function and propagate its
    /// status on failure (SPEC-fallible-calls). A statement, never an expression.
    TryCall {
        dest: IrTryDest,
        callee: String,
        args: Vec<IrExpr>,
        /// The callee's success type — what the destination receives.
        ty: IrType,
    },
}

/// Where a [`IrStmt::TryCall`] puts the value it received.
#[derive(Debug, PartialEq)]
pub enum IrTryDest {
    Let {
        name: String,
        mutable: bool,
    },
    /// A mutable local.
    Assign(String),
    /// An `out` parameter — a write THROUGH a pointer, like [`IrStmt::AssignOut`].
    AssignOut(String),
    Return,
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
    /// A string literal, stored WITHOUT its NUL; codegen appends one.
    ConstStr(String),
    ConstI32(i32),
    ConstBool(bool),
    /// Reference to a parameter/local by name.
    Var(String),
    /// A call to another function in this module. The typechecker has already resolved the
    /// callee, checked arity and argument types, and proved the call graph acyclic.
    Call {
        name: String,
        args: Vec<IrExpr>,
    },
    Unary {
        op: IrUnOp,
        operand: Box<IrExpr>,
    },
    /// `e as T`. The value semantics are a **Mathless rule**, not "whatever the target's
    /// cast does": `f64 -> i32` truncates toward zero, saturates at the bounds, and maps NaN
    /// to 0 (SPEC-numeric-conversion section 2.3). Rust's `as` happens to match, but C's cast
    /// is UB out of range — a C backend has to implement this deliberately.
    Cast {
        to: IrType,
        operand: Box<IrExpr>,
    },
    Binary {
        op: IrBinOp,
        lhs: Box<IrExpr>,
        rhs: Box<IrExpr>,
    },
    /// A string built by appending pieces into the caller's buffer
    /// (SPEC-string-concat §2.3). **Always flat and always in source order** — the
    /// typechecker collapses `a + b + c` into one node with three pieces, never a tree.
    ///
    /// Flatness is not cosmetic. The length has to be counted before a single byte is
    /// written (Q12: a truncated call leaves the buffer untouched), and with a flat list
    /// that count is a sum over the pieces. Every piece has type `Str`; a piece that is a
    /// `Cast { to: Str }` is decimal digits the module produces, and any other piece is
    /// bytes it borrows — from the source or from the host.
    ///
    /// A lone string is NOT wrapped in this: `return a` keeps the #92 path exactly.
    Concat(Vec<IrExpr>),
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum IrUnOp {
    /// Arithmetic negation. Overflow wraps, same rule as the rest of i32 arithmetic
    /// (DP-I4), so `-i32::MIN == i32::MIN`.
    Neg,
    /// Logical not. `bool` only — the typechecker guarantees it, because Rust's `!` would
    /// silently become a bitwise complement on an integer.
    Not,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum IrBinOp {
    Add,
    Sub,
    Mul,
    Div,
    /// `%` — remainder. i32 only; lowered with the same zero guard as `Div`.
    Rem,
    Lt,
    Gt,
    Le,
    Ge,
    Eq,
    Ne,
    /// `&&` — short-circuiting conjunction (SPEC-logical-ops DP-B2).
    And,
    /// `||` — short-circuiting disjunction.
    Or,
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
    matches!(
        body.last(),
        Some(
            IrStmt::Return(_)
                | IrStmt::Fail(_)
                // `return try f(x)` leaves the function on BOTH arms — the value on success,
                // the propagated status on failure — so it terminates a block exactly as a
                // plain `return` does.
                | IrStmt::TryCall {
                    dest: IrTryDest::Return,
                    ..
                }
        )
    )
}

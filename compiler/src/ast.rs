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
    /// `export fn` — reachable by hosts. A bare `fn` is internal: it is emitted as a plain
    /// Rust function, so it never appears in the export table or the generated bindings.
    pub exported: bool,
    pub body: Vec<Stmt>,
}

#[derive(Debug, PartialEq)]
pub struct Param {
    pub name: String,
    pub ty: Type,
    /// `out p: T` — caller-allocates out-parameter: write-only and export-only
    /// (SPEC-out-params DP-O4/O5).
    pub out: bool,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Type {
    /// `string` — a NUL-terminated byte sequence that the module never owns.
    ///
    /// Legal in a **parameter** (borrowed from the host, D16) and in a **`-> string!`
    /// return**, where the bytes go into the host's own buffer under the Q12 protocol
    /// (SPEC-string-return). The `!` is not optional there: a buffer too small is reported as
    /// a status, so every such call has one.
    ///
    /// Still NOT a local and not an `out` — both would ask where the bytes live, and the
    /// module has no allocator. This doc said "**Parameter position only**" until the return
    /// slice landed and did not move with it.
    Str,
    F64,
    Bool,
    I32,
}

#[derive(Debug, PartialEq)]
pub enum Stmt {
    /// `if <cond> { <body> }` (no `else` in the MVP subset).
    If { cond: Expr, body: Vec<Stmt> },
    /// `while <cond> { <body> }`. Not a terminator — the body may run zero times.
    While { cond: Expr, body: Vec<Stmt> },
    /// `return <expr>`.
    Return(Expr),
    /// `fail <CODE>` — fail with a declared error code (only in a fallible function).
    Fail(String),
    /// `let <NAME> = <EXPR>` / `let mut <NAME> = <EXPR>` — a block-scoped local binding.
    /// `mutable` marks it reassignable by [`Stmt::Assign`].
    Let {
        name: String,
        value: Expr,
        mutable: bool,
    },
    /// `<NAME> = <EXPR>` — reassign a mutable local. A statement, never an expression
    /// (DP-M3), so `a = b = c` does not parse and assignment produces no value.
    Assign { name: String, value: Expr },
    /// `<dest> = try <callee>(<args>)` — call a fallible function, propagating its status.
    ///
    /// Deliberately a STATEMENT and not an expression (DP-F2). There is no try-call node in
    /// [`Expr`], so `1 + try f(x)`, `f(try g(x))` and `if try f(x)` are unrepresentable rather
    /// than merely rejected — which is what keeps two measured hazards out of reach: the i32
    /// division guard evaluates its RIGHT operand first, and hoisting a prelude out of a `&&`
    /// condition would evaluate the right operand unconditionally.
    TryCall {
        dest: TryDest,
        callee: String,
        args: Vec<Expr>,
    },
}

/// Where a [`Stmt::TryCall`] puts the value it received. One of the three statement positions
/// `try` is allowed in (DP-F2).
#[derive(Debug, PartialEq, Clone)]
pub enum TryDest {
    Let { name: String, mutable: bool },
    Assign(String),
    Return,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum UnOp {
    /// `-e` — arithmetic negation (numeric operands only).
    Neg,
    /// `!e` — logical not (`bool` only; Rust's `!` is bitwise on integers, so the type rule
    /// is what keeps the lowering honest).
    Not,
}

#[derive(Debug, PartialEq)]
pub enum Expr {
    Number(f64),
    /// `"…"` — an ASCII string literal. Lowers to a static NUL-terminated byte array.
    Str(String),
    Int(i64),
    Bool(bool),
    Var(String),
    /// `NAME(arg, …)` — a call to another function in this module.
    Call {
        name: String,
        args: Vec<Expr>,
    },
    Unary {
        op: UnOp,
        operand: Box<Expr>,
    },
    /// `e as T` — an explicit numeric conversion. There is no implicit widening: DP-I2's
    /// "no silent mixing" stands, this only gives a way to say it (SPEC-numeric-conversion).
    Cast {
        to: Type,
        operand: Box<Expr>,
    },
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
    /// `%` — remainder. i32 only, and total: `x % 0 == 0` (SPEC-i32-division DP-D1/D4).
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

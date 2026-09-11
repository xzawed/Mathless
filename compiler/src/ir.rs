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
    /// `[T]` — borrowed for the call, lowered as `*const T` **plus** an `i32` length the
    /// compiler appends (SPEC-array-input DP-A2). Parameter position only.
    Array(IrArrayElem),
}

/// What an array's elements may be — scalars, and that is a fact of the type rather than a
/// check: no strings (variable length inside variable length), no nesting.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum IrArrayElem {
    F64,
    Bool,
    I32,
}

impl IrArrayElem {
    /// The type one element has once it is read out.
    pub fn scalar(self) -> IrType {
        match self {
            IrArrayElem::F64 => IrType::F64,
            IrArrayElem::Bool => IrType::Bool,
            IrArrayElem::I32 => IrType::I32,
        }
    }
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
            // The manifest quotes this, so an array parameter changes the fingerprint and a
            // changed element type changes it again (SPEC-array-input 2.7).
            IrType::Array(IrArrayElem::F64) => "[f64]",
            IrType::Array(IrArrayElem::Bool) => "[bool]",
            IrType::Array(IrArrayElem::I32) => "[i32]",
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
    /// `result <len>` — declare the length of an array return (SPEC-array-return §2.4).
    ///
    /// Lowers to the capacity check: write `*ml_needed`, and return the truncation status if
    /// the declared length exceeds `ml_cap`. It has to come before any element write, which is
    /// why typeck only accepts it in the function's top-level block.
    ResultLen(IrExpr),
    /// `result[<index>] = <value>` — write one element of an array return.
    ResultSet {
        index: IrExpr,
        value: IrExpr,
    },
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

impl IrStmt {
    /// Does this statement leave the function, so that anything after it in the same block is
    /// dead? The single answer to that question: [`block_always_returns`] asks it of a block's
    /// last statement, and the typechecker's dead-code scan asks it of every earlier one.
    ///
    /// **Written as an exhaustive `match`, not a `matches!` list, on purpose.** A positive
    /// `matches!` list is invisible to the crate's `wildcard_enum_match_arm` deny, so a new
    /// variant falls into `false` without the compiler saying a word — that is exactly how
    /// #161 shipped: `return try` terminated a block by one reckoning and not by the other,
    /// because the two views were two lists. Adding a variant below is now a compile error
    /// here, and one answer serves both callers so the lists cannot drift apart again.
    pub fn is_terminator(&self) -> bool {
        match self {
            IrStmt::Return(_) | IrStmt::Fail(_) => true,
            // `return try f(x)` leaves the function on BOTH arms — the value on success, the
            // propagated status on failure — so it terminates a block exactly as a plain
            // `return` does. The other destinations bind or assign and fall through.
            IrStmt::TryCall { dest, .. } => match dest {
                IrTryDest::Return => true,
                IrTryDest::Let { .. } | IrTryDest::Assign(_) | IrTryDest::AssignOut(_) => false,
            },
            // Neither `result n` nor `result[i] = v` ends a block: the first declares the
            // length and falls through to the writes, and the second is an assignment. The
            // function's exit is generated after the body, the way `-> string!` already works.
            IrStmt::ResultLen(_) | IrStmt::ResultSet { .. } => false,
            // `while` may run zero times and an `if` without an `else` can fall through, so
            // neither ends a block however its body ends (SPEC-while DP-W2).
            IrStmt::If { .. }
            | IrStmt::While { .. }
            | IrStmt::Let { .. }
            | IrStmt::Assign { .. }
            | IrStmt::AssignOut { .. } => false,
        }
    }
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
    /// `xs[i]` — read one element, bounds-checked against the companion length.
    ///
    /// The check is part of the LOWERING, not of this node: codegen emits it because the
    /// failure is an early return, and the typechecker has already proved the enclosing
    /// function is fallible so that early return has somewhere to go (SPEC-array-input 2.4).
    Index {
        array: String,
        index: Box<IrExpr>,
    },
    /// `len(xs)` — the companion length, read straight out of the parameter. Cannot fail, so
    /// it does not make its function fallible (SPEC-array-input 2.4).
    Len {
        array: String,
    },
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

/// The first array an index reads, anywhere in `body`. `None` if nothing is indexed.
///
/// Exhaustive on both enums on purpose: a new statement or expression that can hold an index
/// must say so here, or DP-A3 would stop covering it and an infallible function would lower
/// an early return that has nowhere to go.
pub fn first_index(body: &[IrStmt]) -> Option<String> {
    fn in_expr(e: &IrExpr) -> Option<String> {
        match &e.kind {
            IrExprKind::Index { array, index } => {
                Some(in_expr(index).unwrap_or_else(|| array.clone()))
            }
            IrExprKind::Unary { operand, .. } | IrExprKind::Cast { operand, .. } => {
                in_expr(operand)
            }
            IrExprKind::Binary { lhs, rhs, .. } => in_expr(lhs).or_else(|| in_expr(rhs)),
            IrExprKind::Call { args, .. } | IrExprKind::Concat(args) => {
                args.iter().find_map(in_expr)
            }
            IrExprKind::Len { .. }
            | IrExprKind::ConstF64(_)
            | IrExprKind::ConstStr(_)
            | IrExprKind::ConstI32(_)
            | IrExprKind::ConstBool(_)
            | IrExprKind::Var(_) => None,
        }
    }
    body.iter().find_map(|s| match s {
        IrStmt::If { cond, body } | IrStmt::While { cond, body } => {
            in_expr(cond).or_else(|| first_index(body))
        }
        // The INDEX is searched first and on purpose: `result[xs[i]] = v` indexes an input
        // array inside the subscript, and that is the one this scan is looking for.
        IrStmt::ResultSet { index, value } => in_expr(index).or_else(|| in_expr(value)),
        IrStmt::ResultLen(e)
        | IrStmt::Return(e)
        | IrStmt::Let { value: e, .. }
        | IrStmt::Assign { value: e, .. }
        | IrStmt::AssignOut { value: e, .. } => in_expr(e),
        IrStmt::TryCall { args, .. } => args.iter().find_map(in_expr),
        IrStmt::Fail(_) => None,
    })
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
    body.last().is_some_and(IrStmt::is_terminator)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn expr() -> IrExpr {
        IrExpr {
            ty: IrType::I32,
            kind: IrExprKind::ConstI32(0),
        }
    }

    fn try_call(dest: IrTryDest) -> IrStmt {
        IrStmt::TryCall {
            dest,
            callee: "g".into(),
            args: vec![],
            ty: IrType::I32,
        }
    }

    /// One statement of every shape, and the answer each must give. The exhaustive `match` in
    /// `is_terminator` makes the compiler demand an arm for a NEW variant; this pins what the
    /// existing arms decide, so a flipped answer is caught here rather than in a `.dll`.
    #[test]
    fn every_statement_shape_gets_the_terminator_answer_it_should() {
        let terminating = [
            IrStmt::Return(expr()),
            IrStmt::Fail(1),
            try_call(IrTryDest::Return),
        ];
        for s in &terminating {
            assert!(s.is_terminator(), "{s:?} ends the function");
        }

        let falls_through = [
            IrStmt::If {
                cond: expr(),
                // Even with a body that returns: the `if` itself can be skipped.
                body: vec![IrStmt::Return(expr())],
            },
            IrStmt::While {
                cond: expr(),
                body: vec![IrStmt::Return(expr())],
            },
            IrStmt::Let {
                name: "x".into(),
                value: expr(),
                mutable: false,
            },
            IrStmt::Assign {
                name: "x".into(),
                value: expr(),
            },
            IrStmt::AssignOut {
                name: "o".into(),
                value: expr(),
            },
            try_call(IrTryDest::Let {
                name: "x".into(),
                mutable: false,
            }),
            try_call(IrTryDest::Assign("x".into())),
            try_call(IrTryDest::AssignOut("o".into())),
        ];
        for s in &falls_through {
            assert!(
                !s.is_terminator(),
                "{s:?} falls through to the next statement"
            );
        }
    }

    /// The shape #161 was made of: `return try` ends a block, and the two views of that
    /// question have to agree. They are one function now, and this states the agreement.
    #[test]
    fn block_always_returns_agrees_with_the_statement_it_ends_on() {
        assert!(block_always_returns(&[try_call(IrTryDest::Return)]));
        assert!(!block_always_returns(&[try_call(IrTryDest::Assign(
            "x".into()
        ))]));
        assert!(!block_always_returns(&[]));
    }
}

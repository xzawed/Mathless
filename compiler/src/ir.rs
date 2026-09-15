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
            // changed element type changes it again (SPEC-array-input 2.7). Measured by
            // `iface_hash.rs`'s mutation table — this sentence stood unmeasured from the
            // array slice until 2026-09-15, which is what that table was written to end.
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
    /// `fixed(x, places)` — `x` as fixed-point decimal text with exactly `places` digits after
    /// the point (`SPEC-fixed-decimals`).
    ///
    /// A BUILT string, like a concatenation: the digits exist only while they are written into
    /// the caller's buffer, so `is_built_string` must answer `true` for it.
    ///
    /// It exists because the documented in-module workaround is silently wrong three ways —
    /// `"1234.5"` for 1234.05, `"0.-7"` for -0.07, and `"21474836.47"` for fifty million
    /// (§9-53). None of the primitives it was built from is wrong; the assembly is.
    Fixed {
        x: Box<IrExpr>,
        places: Box<IrExpr>,
    },
    /// `byte_slice(s, from, to)` — the half-open span `[from, to)` of a BORROWED string.
    ///
    /// `to` is an end offset, not a length (DP-B2), so `byte_slice(s, 2, byte_len(s))` is
    /// "from byte 2 to the end" and the two builtins measure the same space.
    ///
    /// This is a BUILT string: the bytes it names are copied into the caller's buffer at
    /// `return` time, exactly like a concatenation, so `is_built_string` must answer `true`
    /// for it. And it is LENGTH-bounded, not NUL-bounded — every writer emitted before this
    /// slice stopped at the source NUL, and reusing one of those would silently return the
    /// suffix (`SPEC-string-slice` §2.5).
    ByteSlice {
        s: Box<IrExpr>,
        from: Box<IrExpr>,
        to: Box<IrExpr>,
    },
    /// `byte_len(s)` — bytes up to the first NUL, the terminator NOT counted
    /// (SPEC-string-length DP-L1).
    ///
    /// A different node from [`Len`] because it is a different computation, not just a
    /// different type: an array's length is HANDED to the module alongside the pointer and
    /// costs nothing, while this one WALKS to the NUL. It cannot fail either — there is no
    /// condition to report — but the reason is not the array's (SPEC §2.3).
    ByteLen(Box<IrExpr>),
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
            IrExprKind::Unary { operand, .. }
            | IrExprKind::Cast { operand, .. }
            | IrExprKind::ByteLen(operand) => in_expr(operand),
            // A span's offsets are ordinary expressions and an index can hide in either, so
            // all three children answer. (`byte_slice` itself needs `-> T!` for its own
            // out-of-range status, but that is DP-B4's business, not this walker's.)
            IrExprKind::ByteSlice { s, from, to } => {
                in_expr(s).or_else(|| in_expr(from)).or_else(|| in_expr(to))
            }
            // Same shape, same reason: `fixed(xs[i] as f64, n)` hides an index in the value,
            // and `fixed(x, xs[i])` hides one in the place count.
            IrExprKind::Fixed { x, places } => in_expr(x).or_else(|| in_expr(places)),
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

/// The first non-ASCII string literal sitting somewhere its bytes are INSPECTED rather than
/// written out, or `None` — `SPEC-non-ascii-literals` §2.2.
///
/// # Why a walker and not a list of guarded positions
///
/// Two drafts of that SPEC put this rule at call sites, and review broke both. The first said
/// `Stmt::Return`'s arm could do it: that arm matches only the ROOT kind, so it never sees a
/// literal nested in a concatenation. The second said to copy `reject_built_string`'s call
/// sites: those six calls live in five arms, and **enumerating consume sites is the defect this
/// repository hit five times in one session** — `byte_len` missing `reject_built_string`
/// (#224), and the binding gate for `-2` being too narrow twice (#238).
///
/// So the context travels DOWN the tree instead, and the match is exhaustive: a new
/// `IrExprKind` has to answer *"is this position an output?"* or the crate does not build.
/// That is the same mechanism `Cargo.toml` documents for `wildcard_enum_match_arm`, and
/// §9-50's conclusion was that it is the only one that has ever worked here.
///
/// # What counts as an output position
///
/// Bytes the module HANDS OUT are safe: the host receives them and the contract says what
/// encoding they are. Bytes the module COMPARES are not: a C or Delphi host sending the same
/// glyphs in its ANSI code page would not match, with status 0 and no warning (§0.4).
///
/// | position | output? |
/// |---|---|
/// | the expression of a `return` | yes |
/// | a piece of a concatenation, at any depth | yes |
/// | the string a `byte_slice` cuts | yes — it is written out next |
/// | the operand of `byte_len` | yes — counting the module's OWN bytes |
/// | either side of `==` / `!=` | **no** |
/// | a call or `try` argument | **no** — the callee may compare it |
/// | anything else | **no** |
///
/// # What this does NOT catch
///
/// The match is exhaustive over VARIANTS, not over FIELDS: arms like `Call { args, .. }` skip
/// the rest with `..`, so a new child expression added to an existing variant would compile
/// and never be walked. That is true of every walker in this file and is not fixed here —
/// written down so the guarantee is not read as wider than it is.
pub fn non_ascii_literal_outside_output(body: &[IrStmt]) -> Option<String> {
    fn in_expr(e: &IrExpr, output: bool) -> Option<String> {
        match &e.kind {
            IrExprKind::ConstStr(s) => (!output && !s.is_ascii()).then(|| s.clone()),
            // These carry the context through: their bytes go wherever the parent's did.
            IrExprKind::Concat(pieces) => pieces.iter().find_map(|p| in_expr(p, output)),
            IrExprKind::ByteLen(operand) => in_expr(operand, true),
            IrExprKind::ByteSlice { s, from, to } => in_expr(s, true)
                .or_else(|| in_expr(from, false))
                .or_else(|| in_expr(to, false)),
            // `fixed` MAKES bytes rather than carrying any: every byte it writes is `-`, `.`
            // or a digit (`SPEC-fixed-decimals` §2.1), so no literal can reach the output
            // through it. Both children are numeric, and both are inspected — `false`.
            IrExprKind::Fixed { x, places } => in_expr(x, false).or_else(|| in_expr(places, false)),
            // …and these do not. A comparison inspects; a call hands the bytes to code this
            // walker is not looking at.
            IrExprKind::Binary { lhs, rhs, .. } => {
                in_expr(lhs, false).or_else(|| in_expr(rhs, false))
            }
            IrExprKind::Call { args, .. } => args.iter().find_map(|a| in_expr(a, false)),
            // These two INHERIT rather than reset. No `ConstStr` can reach either today — a
            // cast's operand is an `i32` and a negation's is numeric — so it makes no
            // difference to any program that compiles. It is written this way because the
            // alternative is wrong for a reason that would be invisible: dropping `Return`'s
            // `true` here would silently refuse a literal the SPEC allows, the day a cast or
            // a unary operator gains a string operand. Review named it.
            IrExprKind::Unary { operand, .. } | IrExprKind::Cast { operand, .. } => {
                in_expr(operand, output)
            }
            IrExprKind::Index { index, .. } => in_expr(index, false),
            IrExprKind::Len { .. }
            | IrExprKind::ConstF64(_)
            | IrExprKind::ConstI32(_)
            | IrExprKind::ConstBool(_)
            | IrExprKind::Var(_) => None,
        }
    }
    body.iter().find_map(|s| match s {
        IrStmt::If { cond, body } | IrStmt::While { cond, body } => {
            in_expr(cond, false).or_else(|| non_ascii_literal_outside_output(body))
        }
        // The one output position. Everything else either inspects the bytes or hands them
        // somewhere this walker cannot follow.
        IrStmt::Return(e) => in_expr(e, true),
        IrStmt::ResultSet { index, value } => {
            in_expr(index, false).or_else(|| in_expr(value, false))
        }
        IrStmt::ResultLen(e)
        | IrStmt::Let { value: e, .. }
        | IrStmt::Assign { value: e, .. }
        | IrStmt::AssignOut { value: e, .. } => in_expr(e, false),
        IrStmt::TryCall { args, .. } => args.iter().find_map(|a| in_expr(a, false)),
        IrStmt::Fail(_) => None,
    })
}

/// Can anything in this body answer `ML_ST_INDEX_OUT_OF_RANGE`?
///
/// The bindings ask this to decide whether to declare the constant, and the header's own rule
/// is that a host must never retype a number the bindings did not promise. So the question has
/// to be *"can this module produce -2"* — not a stand-in for it.
///
/// It was a stand-in twice. `header.rs` asked "does it index an array", which stopped being
/// the same question when a span gained the same status; widening it to "…or compares a span"
/// still missed two more. Measured, each time by building a module and reading its `.h`:
///
/// | construct | emits -2 at |
/// |---|---|
/// | `xs[i]` read | the `Index` arm of `emit_expr` |
/// | `result[i] = v` | the `ResultSet` bounds check |
/// | `byte_slice(..) == ..` | the span comparison lowering |
/// | `return byte_slice(..)` | `emit_concat_return`'s `ml_sublen` bail |
/// | `fixed(x, n)` | `emit_concat_return`'s `ml_fixlen` bail |
///
/// **Exhaustive on purpose.** A new IR variant that can bail with `-2` has to answer here or
/// the crate does not build — which is the only mechanism that has ever worked for this
/// (`Cargo.toml`'s note on `wildcard_enum_match_arm`). A list in a comment would not.
pub fn can_fail_out_of_range(body: &[IrStmt]) -> bool {
    fn in_expr(e: &IrExpr) -> bool {
        match &e.kind {
            // Both of these carry a bounds check wherever they are lowered.
            IrExprKind::Index { .. } | IrExprKind::ByteSlice { .. } => true,
            // `fixed` is here for a DIFFERENT reason than the two above, and that is why it
            // is its own arm: nothing about it is an index. It reports `-2` for the three
            // conditions `SPEC-fixed-decimals` §2.4 lists — a NaN or infinite `x`, a
            // non-literal `places` outside 0..=9, and a scaled value past i64 — because a
            // new reserved status would drag in the documentation and guard cost #238
            // measured. The status is shared; the reason a host sees it is not.
            IrExprKind::Fixed { .. } => true,
            IrExprKind::Unary { operand, .. }
            | IrExprKind::Cast { operand, .. }
            | IrExprKind::ByteLen(operand) => in_expr(operand),
            IrExprKind::Binary { lhs, rhs, .. } => in_expr(lhs) || in_expr(rhs),
            IrExprKind::Call { args, .. } | IrExprKind::Concat(args) => args.iter().any(in_expr),
            IrExprKind::Len { .. }
            | IrExprKind::ConstF64(_)
            | IrExprKind::ConstStr(_)
            | IrExprKind::ConstI32(_)
            | IrExprKind::ConstBool(_)
            | IrExprKind::Var(_) => false,
        }
    }
    body.iter().any(|s| match s {
        IrStmt::If { cond, body } | IrStmt::While { cond, body } => {
            in_expr(cond) || can_fail_out_of_range(body)
        }
        // The write itself is bounds-checked against the DECLARED length, so it can bail even
        // when neither subexpression can. This is the case `SPEC-array-return` named as THE
        // example, and the binding gate missed it for as long as the gate asked about indexing
        // an INPUT array.
        IrStmt::ResultSet { .. } => true,
        IrStmt::ResultLen(e)
        | IrStmt::Return(e)
        | IrStmt::Let { value: e, .. }
        | IrStmt::Assign { value: e, .. }
        | IrStmt::AssignOut { value: e, .. } => in_expr(e),
        IrStmt::TryCall { args, .. } => args.iter().any(in_expr),
        IrStmt::Fail(_) => false,
    })
}

/// Does this body compare a span anywhere? — `SPEC-string-slice-compare` DP-C2.
///
/// The companion to [`first_index`], and it exists for the same stated reason: `check_expr`
/// cannot see the signature, so the question "may this function fail?" is a walk over the
/// finished body. A span outside the string is `ML_ST_INDEX_OUT_OF_RANGE`, and D17 only gives
/// a function a status when its signature says `!`.
///
/// Does this module RETURN any byte outside ASCII?
///
/// Only a returned literal can put one there (`SPEC-non-ascii-literals` §2.1 refuses every
/// other position), so this asks exactly that.
///
/// **It lives here because it has three consumers**, and they must not be able to disagree:
/// the C header and the Delphi unit each carry a one-line UTF-8 notice when it is true
/// (#238 — a binding names what the module can do), and since 2026-09-15 the interface
/// manifest carries `utf8=1` (`SPEC-iface-hash` §2.1). That third consumer is why it moved
/// out of `header.rs`; `can_fail_out_of_range` two functions up made the same move for the
/// same reason.
///
/// A module with no such literal is unchanged in all three places — which matters more here
/// than in the bindings, because in the manifest "unchanged" is what keeps an ASCII module's
/// fingerprint stable across this very change.
/// **The question is whether the bytes LEAVE, not whether the literal exists.** The first
/// version of this walker asked the second, and over-answered: measured 2026-09-15,
/// `export fn f() -> i32 { return byte_len("승인") }` returns the integer 6 — not one byte of
/// UTF-8 crosses the boundary — and its `.h` claimed "This module RETURNS UTF-8 bytes" twice.
/// Harmless while the notice was only prose; not harmless once the manifest reads the same
/// walker, because then that module's fingerprint moves for a contract that did not change.
/// So the walk carries an `output` flag, exactly as [`non_ascii_literal_outside_output`] does.
///
/// The two walkers ask opposite questions and so they disagree on one arm, deliberately:
/// `byte_len`'s operand is a LEGAL place for a non-ASCII literal (the other walker says
/// `true` there) and is NOT a place bytes escape from (this one says `false`). Counting is
/// not copying.
pub fn returns_non_ascii_bytes(module: &IrModule) -> bool {
    fn in_expr(e: &IrExpr, output: bool) -> bool {
        match &e.kind {
            IrExprKind::ConstStr(s) => output && !s.is_ascii(),
            // These carry the context: their bytes go wherever the parent's did.
            IrExprKind::Concat(pieces) => pieces.iter().any(|p| in_expr(p, output)),
            IrExprKind::Unary { operand, .. } | IrExprKind::Cast { operand, .. } => {
                in_expr(operand, output)
            }
            // A span's SOURCE is copied out; its offsets are arithmetic.
            IrExprKind::ByteSlice { s, from, to } => {
                in_expr(s, output) || in_expr(from, false) || in_expr(to, false)
            }
            // `byte_len` COUNTS its operand and yields an i32. Nothing of it is written to
            // the caller's buffer — this arm is the defect above.
            IrExprKind::ByteLen(operand) => in_expr(operand, false),
            // `fixed` emits only `-`, `.` and `0`-`9` (`SPEC-fixed-decimals` §2.1), so it
            // cannot carry a literal's bytes out however its children are written.
            IrExprKind::Fixed { x, places } => in_expr(x, false) || in_expr(places, false),
            IrExprKind::Binary { lhs, rhs, .. } => in_expr(lhs, false) || in_expr(rhs, false),
            IrExprKind::Call { args, .. } => args.iter().any(|a| in_expr(a, false)),
            IrExprKind::Index { index, .. } => in_expr(index, false),
            IrExprKind::Len { .. }
            | IrExprKind::ConstF64(_)
            | IrExprKind::ConstI32(_)
            | IrExprKind::ConstBool(_)
            | IrExprKind::Var(_) => false,
        }
    }
    fn in_stmts(stmts: &[IrStmt]) -> bool {
        stmts.iter().any(|s| match s {
            IrStmt::If { cond, body } | IrStmt::While { cond, body } => {
                in_expr(cond, false) || in_stmts(body)
            }
            // The one statement whose expression reaches a host. An internal function's
            // `return` counts too: a caller can propagate it out with `try`.
            IrStmt::Return(e) => in_expr(e, true),
            // An array return writes SCALARS, so nothing textual leaves through `result`.
            IrStmt::ResultLen(e) => in_expr(e, false),
            IrStmt::ResultSet { index, value } => in_expr(index, false) || in_expr(value, false),
            // An `out` parameter is a scalar today (`SPEC-out-params`: no `out string`), so
            // this is `false` for the same reason. Spelled rather than merged above, because
            // the day `out string` exists this arm has to change and the merge would hide it.
            IrStmt::AssignOut { value: e, .. } => in_expr(e, false),
            IrStmt::Let { value: e, .. } | IrStmt::Assign { value: e, .. } => in_expr(e, false),
            IrStmt::TryCall { args, .. } => args.iter().any(|a| in_expr(a, false)),
            IrStmt::Fail(_) => false,
        })
    }
    module.functions.iter().any(|f| in_stmts(&f.body))
}

/// Only an EQUALITY whose operand is a span counts. A span in a `return` does not: that is
/// the string-return path, which is `-> string!` by construction.
pub fn compares_a_span(body: &[IrStmt]) -> bool {
    fn in_expr(e: &IrExpr) -> bool {
        match &e.kind {
            IrExprKind::Binary { op, lhs, rhs } => {
                (matches!(op, IrBinOp::Eq | IrBinOp::Ne)
                    && (matches!(lhs.kind, IrExprKind::ByteSlice { .. })
                        || matches!(rhs.kind, IrExprKind::ByteSlice { .. })))
                    || in_expr(lhs)
                    || in_expr(rhs)
            }
            IrExprKind::Unary { operand, .. }
            | IrExprKind::Cast { operand, .. }
            | IrExprKind::ByteLen(operand) => in_expr(operand),
            IrExprKind::ByteSlice { s, from, to } => in_expr(s) || in_expr(from) || in_expr(to),
            // A span cannot be an argument to `fixed` — both parameters are numeric — but the
            // children are walked anyway, because this walker's job is to find a comparison
            // ANYWHERE beneath, and "no span can reach here" is a fact about today's
            // signature rather than about the node.
            IrExprKind::Fixed { x, places } => in_expr(x) || in_expr(places),
            IrExprKind::Index { index, .. } => in_expr(index),
            IrExprKind::Call { args, .. } | IrExprKind::Concat(args) => args.iter().any(in_expr),
            IrExprKind::Len { .. }
            | IrExprKind::ConstF64(_)
            | IrExprKind::ConstStr(_)
            | IrExprKind::ConstI32(_)
            | IrExprKind::ConstBool(_)
            | IrExprKind::Var(_) => false,
        }
    }
    body.iter().any(|s| match s {
        IrStmt::If { cond, body } | IrStmt::While { cond, body } => {
            in_expr(cond) || compares_a_span(body)
        }
        IrStmt::ResultSet { index, value } => in_expr(index) || in_expr(value),
        IrStmt::ResultLen(e)
        | IrStmt::Return(e)
        | IrStmt::Let { value: e, .. }
        | IrStmt::Assign { value: e, .. }
        | IrStmt::AssignOut { value: e, .. } => in_expr(e),
        IrStmt::TryCall { args, .. } => args.iter().any(in_expr),
        IrStmt::Fail(_) => false,
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

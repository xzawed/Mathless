//! Typecheck + lowering AST → typed IR (W3).
//!
//! Rules for the MVP subset:
//! - arithmetic (`+ - *`): both operands the same numeric type (`f64`→`f64`, `i32`→`i32`);
//!   no implicit i32/f64 mixing. Division `/` is `f64` only — `i32 /` is rejected because
//!   `/0` panics, and a panic in a generated module hangs the calling thread (STATUS §5-4).
//! - ordered comparison (`< > <= >=`): both operands the same numeric type → `bool`
//! - equality (`== !=`): operands of equal type → `bool`
//! - `if` condition must be `bool`
//! - `return <e>`: type of `<e>` must equal the function's return type
//! - variables must be in scope (parameters and `let` locals)
//! - assignment (`x = e`) targets a `let mut` local only: parameters (D16 borrow) and
//!   immutable `let`s are rejected, and the RHS type must equal the variable's

use std::collections::{HashMap, HashSet};

use crate::ast::{self, BinOp, Expr, Stmt, Type};
use crate::ir::*;

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct TypeError {
    pub message: String,
}

impl TypeError {
    pub fn new(message: impl Into<String>) -> Self {
        TypeError {
            message: message.into(),
        }
    }
}

impl std::fmt::Display for TypeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "type error: {}", self.message)
    }
}

impl std::error::Error for TypeError {}

/// How a name in scope was bound. Only [`Binding::LetMut`] may be assigned to; the other two
/// are rejected with a message that says *why* (DP-M2 — a parameter is a borrow for the
/// duration of the call, D16, so it is not a mutable slot either).
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
enum Binding {
    Param,
    Let,
    LetMut,
}

type Scope = HashMap<String, (IrType, Binding)>;

pub fn check(module: &ast::Module) -> Result<IrModule, TypeError> {
    // Module-scoped error table (D17). Codes are positive i32 (parser-validated).
    let mut error_table: Scope2 = HashMap::new();
    let mut errors = Vec::with_capacity(module.errors.len());
    for e in &module.errors {
        if error_table.insert(e.name.clone(), e.code).is_some() {
            return Err(TypeError::new(format!("duplicate error code '{}'", e.name)));
        }
        errors.push(IrErrorDecl {
            name: e.name.clone(),
            code: e.code,
        });
    }
    // Function names must be unique case-insensitively. Same-case duplicates collide as
    // duplicate `#[no_mangle] mlx_<name>` symbols at link time; `foo` vs `Foo` are distinct
    // in C/Rust but collide in the (case-insensitive) Delphi import unit.
    let mut seen_fns: HashSet<String> = HashSet::new();
    let mut functions = Vec::with_capacity(module.functions.len());
    for f in &module.functions {
        if !seen_fns.insert(f.name.to_ascii_lowercase()) {
            return Err(TypeError::new(format!(
                "duplicate function '{}' — function names must be unique case-insensitively (Delphi binding)",
                f.name
            )));
        }
        // An INTERNAL function's name is emitted raw into the generated Rust — the `mlx_`
        // prefix that makes exported names safe does not apply to it (Grok verify). So it
        // needs the same checks a parameter or local gets, plus the reserved namespaces.
        if !f.exported {
            let targets = crate::reserved::reserving_targets(&f.name);
            if !targets.is_empty() {
                return Err(TypeError::new(format!(
                    "internal function '{}' is a reserved word in {} — rename it (an internal \
                     name is emitted as-is, unlike an export which gets the `mlx_` prefix)",
                    f.name,
                    targets.join(", ")
                )));
            }
            if f.name.starts_with("ml_") || f.name.starts_with("mlx_") {
                return Err(TypeError::new(format!(
                    "internal function '{}' uses a reserved prefix — `ml_` is the runtime \
                     namespace and `mlx_` is what exports are emitted as (D18), so this can \
                     collide with a generated symbol",
                    f.name
                )));
            }
            // D17's error ABI is a HOST-BOUNDARY convention: an i32 status plus an
            // `out_value` out-param appended to the *exported* signature. There is no
            // internal calling convention for it, and DP-C1 forbids calling a fallible
            // callee, so an internal `-> T!` is unreachable by construction. Left to
            // codegen it lowered wrong, because a non-exported fn is emitted with
            // `fallible = false`: `fail E` became a plain `return <code>;` — a silent
            // wrong answer for `-> i32!`, and a rustc E0308 for the other types that
            // reached the user only as "cargo build of generated crate failed".
            if f.fallible {
                return Err(TypeError::new(format!(
                    "internal function '{}' is fallible (`-> {}!`) — the D17 error ABI is a \
                     host-boundary convention (i32 status + out-param), so a fallible \
                     function must be `export`ed; an internal one can never be called \
                     because a fallible callee is not a value",
                    f.name,
                    ir_type(f.ret),
                )));
            }
        }
        functions.push(f);
    }

    // First pass: every signature, so calls resolve regardless of declaration order (DP-C4).
    let mut sigs: Sigs = HashMap::new();
    for f in &module.functions {
        sigs.insert(
            f.name.clone(),
            Sig {
                params: f.params.iter().map(|p| ir_type(p.ty)).collect(),
                ret: ir_type(f.ret),
                fallible: f.fallible,
            },
        );
    }
    // Before checking bodies: the call graph must be acyclic (DP-C2).
    reject_recursion(module)?;

    let mut checked = Vec::with_capacity(functions.len());
    for f in functions {
        checked.push(check_function(f, &error_table, &sigs)?);
    }
    Ok(IrModule {
        functions: checked,
        errors,
    })
}

/// Error-code name → resolved positive code.
type Scope2 = HashMap<String, i32>;

/// A callable function's shape, gathered in a first pass so declaration order does not
/// matter (DP-C4).
struct Sig {
    params: Vec<IrType>,
    ret: IrType,
    fallible: bool,
}

type Sigs = HashMap<String, Sig>;

/// Every function name called anywhere in `body`, in encounter order.
fn collect_calls(body: &[Stmt], out: &mut Vec<String>) {
    fn expr(e: &Expr, out: &mut Vec<String>) {
        match e {
            Expr::Call { name, args } => {
                out.push(name.clone());
                for a in args {
                    expr(a, out);
                }
            }
            Expr::Unary { operand, .. } | Expr::Cast { operand, .. } => expr(operand, out),
            Expr::Binary { lhs, rhs, .. } => {
                expr(lhs, out);
                expr(rhs, out);
            }
            Expr::Number(_) | Expr::Int(_) | Expr::Bool(_) | Expr::Var(_) => {}
        }
    }
    for s in body {
        match s {
            Stmt::If { cond, body } | Stmt::While { cond, body } => {
                expr(cond, out);
                collect_calls(body, out);
            }
            Stmt::Return(e) | Stmt::Let { value: e, .. } | Stmt::Assign { value: e, .. } => {
                expr(e, out)
            }
            Stmt::Fail(_) => {}
        }
    }
}

/// Reject direct and mutual recursion (DP-C2, SPEC-calls section 5.1).
///
/// Unlike `while` non-termination — which is the halting problem, and therefore documented
/// as a host contract rather than prevented — a cycle in the call graph is *decidable*, and
/// its consequence is worse: infinite recursion overflows the stack and kills the host
/// process. What can be prevented is prevented.
fn reject_recursion(module: &ast::Module) -> Result<(), TypeError> {
    let mut graph: HashMap<&str, Vec<String>> = HashMap::new();
    for f in &module.functions {
        let mut calls = Vec::new();
        collect_calls(&f.body, &mut calls);
        graph.insert(f.name.as_str(), calls);
    }

    // Iterative DFS with an explicit "on the current path" set, so the reported cycle is the
    // actual path rather than just the fact that one exists.
    #[derive(PartialEq, Clone, Copy)]
    enum Mark {
        Doing,
        Done,
    }
    let mut mark: HashMap<String, Mark> = HashMap::new();
    let mut path: Vec<String> = Vec::new();

    fn walk(
        name: &str,
        graph: &HashMap<&str, Vec<String>>,
        mark: &mut HashMap<String, Mark>,
        path: &mut Vec<String>,
    ) -> Result<(), TypeError> {
        match mark.get(name) {
            Some(Mark::Done) => return Ok(()),
            Some(Mark::Doing) => {
                let start = path.iter().position(|n| n == name).unwrap_or(0);
                let mut cycle: Vec<String> = path[start..].to_vec();
                cycle.push(name.to_string());
                return Err(TypeError::new(format!(
                    "recursion is not supported: {} — a recursive call can overflow the stack \
                     and kill the host process, so the call graph must be acyclic",
                    cycle.join(" -> ")
                )));
            }
            None => {}
        }
        mark.insert(name.to_string(), Mark::Doing);
        path.push(name.to_string());
        if let Some(callees) = graph.get(name) {
            for callee in callees {
                // Unknown callees are reported by the type checker with a better message.
                if graph.contains_key(callee.as_str()) {
                    walk(callee, graph, mark, path)?;
                }
            }
        }
        path.pop();
        mark.insert(name.to_string(), Mark::Done);
        Ok(())
    }

    for f in &module.functions {
        walk(&f.name, &graph, &mut mark, &mut path)?;
    }
    Ok(())
}

fn ir_type(t: Type) -> IrType {
    match t {
        Type::F64 => IrType::F64,
        Type::Bool => IrType::Bool,
        Type::I32 => IrType::I32,
    }
}

fn check_function(
    f: &ast::Function,
    errors: &Scope2,
    sigs: &Sigs,
) -> Result<IrFunction, TypeError> {
    let mut scope: Scope = HashMap::new();
    // Parameter names must be unique case-insensitively: distinct in C/Rust, but the
    // (case-insensitive) Delphi import unit would see `x` and `X` as the same param.
    let mut seen_params: HashSet<String> = HashSet::new();
    let mut params = Vec::with_capacity(f.params.len());
    for p in &f.params {
        let targets = crate::reserved::reserving_targets(&p.name);
        if !targets.is_empty() {
            return Err(TypeError::new(format!(
                "function '{}': parameter '{}' is a reserved word in {} — rename it",
                f.name,
                p.name,
                targets.join(", ")
            )));
        }
        if !seen_params.insert(p.name.to_ascii_lowercase()) {
            return Err(TypeError::new(format!(
                "function '{}': duplicate parameter '{}' — parameter names must be unique case-insensitively (Delphi binding)",
                f.name, p.name
            )));
        }
        let ty = ir_type(p.ty);
        scope.insert(p.name.clone(), (ty, Binding::Param));
        params.push(IrParam {
            name: p.name.clone(),
            ty,
        });
    }
    // The fallible ABI synthesizes an `out_value` out-param; a user param of that name would
    // collide in the generated Rust (Grok verify). Reject it with a clear message.
    if f.fallible && f.params.iter().any(|p| p.name == "out_value") {
        return Err(TypeError::new(format!(
            "function '{}': parameter name 'out_value' is reserved in a fallible function \
             (it names the D17 out-param) — rename it",
            f.name
        )));
    }
    let ret = ir_type(f.ret);
    let body = check_block(&f.body, &scope, ret, &f.name, f.fallible, errors, sigs)?;
    // Every path must exit with a value (or `fail`); an `if` without `else` can fall through.
    // Caught here (frontend) as well as in codegen (backend safety net for directly-built IR).
    if !block_always_returns(&body) {
        return Err(TypeError::new(format!(
            "function '{}' may not return on all paths — end it with a `return`{}",
            f.name,
            if f.fallible { " or `fail`" } else { "" }
        )));
    }
    Ok(IrFunction {
        name: f.name.clone(),
        params,
        ret,
        fallible: f.fallible,
        exported: f.exported,
        body,
    })
}

fn check_block(
    stmts: &[Stmt],
    parent_scope: &Scope,
    ret: IrType,
    fname: &str,
    fallible: bool,
    errors: &Scope2,
    sigs: &Sigs,
) -> Result<Vec<IrStmt>, TypeError> {
    // Block scope (DP-L5): locals declared here extend a private copy of the scope and do not
    // leak to the parent block. Nested `if` bodies get their own copy the same way.
    let mut scope = parent_scope.clone();
    let mut out = Vec::with_capacity(stmts.len());
    for s in stmts {
        out.push(check_stmt(
            s, &mut scope, ret, fname, fallible, errors, sigs,
        )?);
    }
    // Anything after a `return`/`fail` is dead. Say that, instead of letting the
    // all-paths-return check below report "may not return on all paths" — which sends the
    // reader hunting for a missing `return` that is right there in front of them.
    let last = out.len().saturating_sub(1);
    if let Some(i) = out[..last]
        .iter()
        .position(|s| matches!(s, IrStmt::Return(_) | IrStmt::Fail(_)))
    {
        let kw = if matches!(out[i], IrStmt::Fail(_)) {
            "fail"
        } else {
            "return"
        };
        return Err(TypeError::new(format!(
            "function '{fname}': unreachable statement after `{kw}` — everything following it \
             is dead code"
        )));
    }
    Ok(out)
}

fn check_stmt(
    s: &Stmt,
    scope: &mut Scope,
    ret: IrType,
    fname: &str,
    fallible: bool,
    errors: &Scope2,
    sigs: &Sigs,
) -> Result<IrStmt, TypeError> {
    match s {
        Stmt::If { cond, body } => {
            let cond = check_expr(cond, scope, fname, sigs)?;
            if cond.ty != IrType::Bool {
                return Err(TypeError::new(format!(
                    "function '{fname}': if condition must be bool, found {}",
                    cond.ty
                )));
            }
            let body = check_block(body, scope, ret, fname, fallible, errors, sigs)?;
            Ok(IrStmt::If { cond, body })
        }
        Stmt::While { cond, body } => {
            let cond = check_expr(cond, scope, fname, sigs)?;
            if cond.ty != IrType::Bool {
                return Err(TypeError::new(format!(
                    "function '{fname}': while condition must be bool, found {}",
                    cond.ty
                )));
            }
            // Same block scope as `if`: the body sees the enclosing bindings (so it can assign
            // an outer `let mut` — the point of the slice) and its own locals do not escape.
            let body = check_block(body, scope, ret, fname, fallible, errors, sigs)?;
            Ok(IrStmt::While { cond, body })
        }
        Stmt::Return(e) => {
            let e = check_expr(e, scope, fname, sigs)?;
            if e.ty != ret {
                return Err(TypeError::new(format!(
                    "function '{fname}': return type mismatch: expected {ret}, found {}",
                    e.ty
                )));
            }
            Ok(IrStmt::Return(e))
        }
        Stmt::Fail(name) => {
            if !fallible {
                return Err(TypeError::new(format!(
                    "function '{fname}': `fail` is only allowed in a fallible function — declare it `-> {ret}!`"
                )));
            }
            match errors.get(name) {
                Some(&code) => Ok(IrStmt::Fail(code)),
                None => Err(TypeError::new(format!(
                    "function '{fname}': unknown error code '{name}' — declare `error {name} = <positive int>`"
                ))),
            }
        }
        Stmt::Assign { name, value } => {
            // The target must exist, be a `let mut`, and keep its type (DP-M2).
            let (ty, binding) = match scope.get(name) {
                Some(&(ty, binding)) => (ty, binding),
                None => {
                    return Err(TypeError::new(format!(
                        "function '{fname}': unknown variable '{name}' — assignment needs a `let mut` local in scope"
                    )))
                }
            };
            match binding {
                Binding::LetMut => {}
                Binding::Let => {
                    return Err(TypeError::new(format!(
                        "function '{fname}': '{name}' is immutable — declare it `let mut {name} = …` to reassign it"
                    )))
                }
                Binding::Param => {
                    return Err(TypeError::new(format!(
                        "function '{fname}': cannot assign to parameter '{name}' — parameters are immutable (D16: an argument is borrowed for the call)"
                    )))
                }
            }
            let value = check_expr(value, scope, fname, sigs)?;
            if value.ty != ty {
                return Err(TypeError::new(format!(
                    "function '{fname}': cannot assign {} to '{name}' of type {ty} — type mismatch",
                    value.ty
                )));
            }
            Ok(IrStmt::Assign {
                name: name.clone(),
                value,
            })
        }
        Stmt::Let {
            name,
            value,
            mutable,
        } => {
            // Check the RHS in the CURRENT scope first, so `let x = x` is use-before-def.
            let value = check_expr(value, scope, fname, sigs)?;
            // Local names honour the same reserved-word policy as parameters (all targets).
            let targets = crate::reserved::reserving_targets(name);
            if !targets.is_empty() {
                return Err(TypeError::new(format!(
                    "function '{fname}': local '{name}' is a reserved word in {} — rename it",
                    targets.join(", ")
                )));
            }
            // In a fallible fn, `out_value` names the synthesized D17 out-param.
            if fallible && name == "out_value" {
                return Err(TypeError::new(format!(
                    "function '{fname}': local 'out_value' is reserved in a fallible function (it names the D17 out-param) — rename it"
                )));
            }
            // No redeclaration or shadowing (DP-L2): the name must not already be in scope.
            if scope.contains_key(name) {
                return Err(TypeError::new(format!(
                    "function '{fname}': '{name}' is already in scope — no redeclaration or shadowing"
                )));
            }
            scope.insert(
                name.clone(),
                (
                    value.ty,
                    if *mutable {
                        Binding::LetMut
                    } else {
                        Binding::Let
                    },
                ),
            );
            Ok(IrStmt::Let {
                name: name.clone(),
                value,
                mutable: *mutable,
            })
        }
    }
}

fn check_expr(e: &Expr, scope: &Scope, fname: &str, sigs: &Sigs) -> Result<IrExpr, TypeError> {
    match e {
        Expr::Number(n) => Ok(IrExpr {
            ty: IrType::F64,
            kind: IrExprKind::ConstF64(*n),
        }),
        Expr::Int(n) => {
            // The only integer type is i32; the literal must fit.
            if *n < i32::MIN as i64 || *n > i32::MAX as i64 {
                return Err(TypeError::new(format!(
                    "function '{fname}': integer literal {n} does not fit in i32"
                )));
            }
            Ok(IrExpr {
                ty: IrType::I32,
                kind: IrExprKind::ConstI32(*n as i32),
            })
        }
        Expr::Bool(b) => Ok(IrExpr {
            ty: IrType::Bool,
            kind: IrExprKind::ConstBool(*b),
        }),
        Expr::Var(name) => match scope.get(name) {
            Some(&(ty, _)) => Ok(IrExpr {
                ty,
                kind: IrExprKind::Var(name.clone()),
            }),
            None => Err(TypeError::new(format!(
                "function '{fname}': unknown variable '{name}'"
            ))),
        },
        Expr::Call { name, args } => {
            let Some(sig) = sigs.get(name) else {
                return Err(TypeError::new(format!(
                    "function '{fname}': unknown function '{name}'"
                )));
            };
            // A fallible callee lowers to `int32 status` + an out-param, so it is not a value
            // and cannot sit in an expression (DP-C1).
            if sig.fallible {
                return Err(TypeError::new(format!(
                    "function '{fname}': '{name}' is fallible (`-> T!`) and cannot be called in \
                     an expression yet — it returns a status and writes through an out-param"
                )));
            }
            if args.len() != sig.params.len() {
                return Err(TypeError::new(format!(
                    "function '{fname}': '{name}' expects {} argument(s), found {}",
                    sig.params.len(),
                    args.len()
                )));
            }
            let mut checked = Vec::with_capacity(args.len());
            for (i, (arg, want)) in args.iter().zip(sig.params.iter()).enumerate() {
                let arg = check_expr(arg, scope, fname, sigs)?;
                if arg.ty != *want {
                    return Err(TypeError::new(format!(
                        "function '{fname}': '{name}' argument {} expects {want}, found {}",
                        i + 1,
                        arg.ty
                    )));
                }
                checked.push(arg);
            }
            Ok(IrExpr {
                ty: sig.ret,
                kind: IrExprKind::Call {
                    name: name.clone(),
                    args: checked,
                },
            })
        }
        Expr::Unary { op, operand } => {
            let operand = check_expr(operand, scope, fname, sigs)?;
            let (ir_op, ty) = match op {
                ast::UnOp::Neg => {
                    if !matches!(operand.ty, IrType::F64 | IrType::I32) {
                        return Err(TypeError::new(format!(
                            "function '{fname}': unary `-` needs f64 or i32, found {}",
                            operand.ty
                        )));
                    }
                    (IrUnOp::Neg, operand.ty)
                }
                ast::UnOp::Not => {
                    if operand.ty != IrType::Bool {
                        return Err(TypeError::new(format!(
                            "function '{fname}': unary `!` needs bool, found {}",
                            operand.ty
                        )));
                    }
                    (IrUnOp::Not, IrType::Bool)
                }
            };
            Ok(IrExpr {
                ty,
                kind: IrExprKind::Unary {
                    op: ir_op,
                    operand: Box::new(operand),
                },
            })
        }
        Expr::Cast { to, operand } => {
            let operand = check_expr(operand, scope, fname, sigs)?;
            let to = ir_type(*to);
            // Numeric only. `bool` is neither a source nor a target: numbers are not truthy
            // and truth is not numeric, which is the stance conditions already take.
            let ok = matches!(
                (operand.ty, to),
                (IrType::I32, IrType::F64)
                    | (IrType::F64, IrType::I32)
                    | (IrType::I32, IrType::I32)
                    | (IrType::F64, IrType::F64)
            );
            if !ok {
                return Err(TypeError::new(format!(
                    "function '{fname}': cannot cast {} to {to} — `as` converts between f64 \
                     and i32 only, and bool is not convertible",
                    operand.ty
                )));
            }
            Ok(IrExpr {
                ty: to,
                kind: IrExprKind::Cast {
                    to,
                    operand: Box::new(operand),
                },
            })
        }
        Expr::Binary { op, lhs, rhs } => {
            let lhs = check_expr(lhs, scope, fname, sigs)?;
            let rhs = check_expr(rhs, scope, fname, sigs)?;
            let (irop, ty) = check_binop(*op, lhs.ty, rhs.ty, fname)?;
            Ok(IrExpr {
                ty,
                kind: IrExprKind::Binary {
                    op: irop,
                    lhs: Box::new(lhs),
                    rhs: Box::new(rhs),
                },
            })
        }
    }
}

fn check_binop(
    op: BinOp,
    lt: IrType,
    rt: IrType,
    fname: &str,
) -> Result<(IrBinOp, IrType), TypeError> {
    match op {
        // Both sides bool, result bool. Numbers are not truthy — the same stance the `if`
        // and `while` conditions already take.
        BinOp::And | BinOp::Or => match (lt, rt) {
            (IrType::Bool, IrType::Bool) => Ok((map_op(op), IrType::Bool)),
            _ => Err(TypeError::new(format!(
                "function '{fname}': operator {op:?} expects two bool operands, found {lt} and {rt}"
            ))),
        },
        // Same numeric type on both sides; no implicit i32/f64 mixing (DP-I2).
        BinOp::Add | BinOp::Sub | BinOp::Mul => match (lt, rt) {
            (IrType::F64, IrType::F64) => Ok((map_op(op), IrType::F64)),
            (IrType::I32, IrType::I32) => Ok((map_op(op), IrType::I32)),
            _ => Err(TypeError::new(format!(
                "function '{fname}': operator {op:?} expects two f64 or two i32 operands, found {lt} and {rt}"
            ))),
        },
        BinOp::Div => match (lt, rt) {
            (IrType::F64, IrType::F64) => Ok((map_op(op), IrType::F64)),
            // i32 division is out of this slice (DP-I3) because `/0` panics, and a panic in a
            // generated module spins forever in the emitted `loop {}` panic handler rather
            // than aborting (STATUS §5-4 — the older "aborts" wording was wrong).
            //
            // The message names the f64 round-trip because it is exact for every i32 pair,
            // but it must also name that route's trap: `b == 0` becomes inf/-inf/NaN and
            // then saturates to i32::MAX / i32::MIN / 0, so a zero divisor silently returns a
            // plausible number instead of failing (measured on a loaded module).
            (IrType::I32, IrType::I32) => Err(TypeError::new(format!(
                "function '{fname}': i32 division `/` is not supported yet (SPEC-i32 DP-I3) — \
                 write `(a as f64 / b as f64) as i32`, but guard `b == 0` yourself: that form \
                 yields i32::MAX/i32::MIN/0 for a zero divisor instead of reporting an error"
            ))),
            _ => Err(TypeError::new(format!(
                "function '{fname}': operator / expects two f64 operands, found {lt} and {rt}"
            ))),
        },
        BinOp::Lt | BinOp::Gt | BinOp::Le | BinOp::Ge => match (lt, rt) {
            (IrType::F64, IrType::F64) | (IrType::I32, IrType::I32) => Ok((map_op(op), IrType::Bool)),
            _ => Err(TypeError::new(format!(
                "function '{fname}': comparison {op:?} expects two f64 or two i32 operands, found {lt} and {rt}"
            ))),
        },
        BinOp::Eq | BinOp::Ne => {
            if lt != rt {
                return Err(TypeError::new(format!(
                    "function '{fname}': {op:?} requires equal operand types, found {lt} and {rt}"
                )));
            }
            Ok((map_op(op), IrType::Bool))
        }
    }
}

fn map_op(op: BinOp) -> IrBinOp {
    match op {
        BinOp::Add => IrBinOp::Add,
        BinOp::Sub => IrBinOp::Sub,
        BinOp::Mul => IrBinOp::Mul,
        BinOp::Div => IrBinOp::Div,
        BinOp::Lt => IrBinOp::Lt,
        BinOp::Gt => IrBinOp::Gt,
        BinOp::Le => IrBinOp::Le,
        BinOp::Ge => IrBinOp::Ge,
        BinOp::Eq => IrBinOp::Eq,
        BinOp::Ne => IrBinOp::Ne,
        BinOp::And => IrBinOp::And,
        BinOp::Or => IrBinOp::Or,
    }
}

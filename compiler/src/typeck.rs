//! Typecheck + lowering AST → typed IR (W3).
//!
//! Rules for the MVP subset:
//! - arithmetic (`+ - * /`): both operands `f64` → `f64`
//! - ordered comparison (`< > <= >=`): both operands `f64` → `bool`
//! - equality (`== !=`): operands of equal type → `bool`
//! - `if` condition must be `bool`
//! - `return <e>`: type of `<e>` must equal the function's return type
//! - variables must be in scope (parameters only, in this subset)

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

type Scope = HashMap<String, IrType>;

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
        functions.push(check_function(f, &error_table)?);
    }
    Ok(IrModule { functions, errors })
}

/// Error-code name → resolved positive code.
type Scope2 = HashMap<String, i32>;

fn ir_type(t: Type) -> IrType {
    match t {
        Type::F64 => IrType::F64,
        Type::Bool => IrType::Bool,
    }
}

fn check_function(f: &ast::Function, errors: &Scope2) -> Result<IrFunction, TypeError> {
    let mut scope: Scope = HashMap::new();
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
        let ty = ir_type(p.ty);
        if scope.insert(p.name.clone(), ty).is_some() {
            return Err(TypeError::new(format!(
                "function '{}': duplicate parameter '{}'",
                f.name, p.name
            )));
        }
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
    let body = check_block(&f.body, &scope, ret, &f.name, f.fallible, errors)?;
    Ok(IrFunction {
        name: f.name.clone(),
        params,
        ret,
        fallible: f.fallible,
        body,
    })
}

fn check_block(
    stmts: &[Stmt],
    scope: &Scope,
    ret: IrType,
    fname: &str,
    fallible: bool,
    errors: &Scope2,
) -> Result<Vec<IrStmt>, TypeError> {
    stmts
        .iter()
        .map(|s| check_stmt(s, scope, ret, fname, fallible, errors))
        .collect()
}

fn check_stmt(
    s: &Stmt,
    scope: &Scope,
    ret: IrType,
    fname: &str,
    fallible: bool,
    errors: &Scope2,
) -> Result<IrStmt, TypeError> {
    match s {
        Stmt::If { cond, body } => {
            let cond = check_expr(cond, scope, fname)?;
            if cond.ty != IrType::Bool {
                return Err(TypeError::new(format!(
                    "function '{fname}': if condition must be bool, found {:?}",
                    cond.ty
                )));
            }
            let body = check_block(body, scope, ret, fname, fallible, errors)?;
            Ok(IrStmt::If { cond, body })
        }
        Stmt::Return(e) => {
            let e = check_expr(e, scope, fname)?;
            if e.ty != ret {
                return Err(TypeError::new(format!(
                    "function '{fname}': return type mismatch: expected {ret:?}, found {:?}",
                    e.ty
                )));
            }
            Ok(IrStmt::Return(e))
        }
        Stmt::Fail(name) => {
            if !fallible {
                return Err(TypeError::new(format!(
                    "function '{fname}': `fail` is only allowed in a fallible function — declare it `-> {ret:?}!`"
                )));
            }
            match errors.get(name) {
                Some(&code) => Ok(IrStmt::Fail(code)),
                None => Err(TypeError::new(format!(
                    "function '{fname}': unknown error code '{name}' — declare `error {name} = <positive int>`"
                ))),
            }
        }
    }
}

fn check_expr(e: &Expr, scope: &Scope, fname: &str) -> Result<IrExpr, TypeError> {
    match e {
        Expr::Number(n) => Ok(IrExpr {
            ty: IrType::F64,
            kind: IrExprKind::ConstF64(*n),
        }),
        Expr::Bool(b) => Ok(IrExpr {
            ty: IrType::Bool,
            kind: IrExprKind::ConstBool(*b),
        }),
        Expr::Var(name) => match scope.get(name) {
            Some(&ty) => Ok(IrExpr {
                ty,
                kind: IrExprKind::Var(name.clone()),
            }),
            None => Err(TypeError::new(format!(
                "function '{fname}': unknown variable '{name}'"
            ))),
        },
        Expr::Binary { op, lhs, rhs } => {
            let lhs = check_expr(lhs, scope, fname)?;
            let rhs = check_expr(rhs, scope, fname)?;
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
        BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div => {
            if lt != IrType::F64 || rt != IrType::F64 {
                return Err(TypeError::new(format!(
                    "function '{fname}': operator {op:?} expects f64 operands, found {lt:?} and {rt:?}"
                )));
            }
            Ok((map_op(op), IrType::F64))
        }
        BinOp::Lt | BinOp::Gt | BinOp::Le | BinOp::Ge => {
            if lt != IrType::F64 || rt != IrType::F64 {
                return Err(TypeError::new(format!(
                    "function '{fname}': comparison {op:?} expects f64 operands, found {lt:?} and {rt:?}"
                )));
            }
            Ok((map_op(op), IrType::Bool))
        }
        BinOp::Eq | BinOp::Ne => {
            if lt != rt {
                return Err(TypeError::new(format!(
                    "function '{fname}': {op:?} requires equal operand types, found {lt:?} and {rt:?}"
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
    }
}

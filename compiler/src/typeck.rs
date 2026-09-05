//! Typecheck + lowering AST → typed IR (W3).
//!
//! Rules for the MVP subset:
//! - arithmetic (`+ - * / %`): both operands the same numeric type (`f64`→`f64`, `i32`→`i32`);
//!   no implicit i32/f64 mixing. `%` is i32-only (SPEC-i32-division DP-D4). i32 `/` and `%`
//!   are TOTAL — the zero and `i32::MIN / -1` cases are closed in codegen, not here.
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
    /// An `out` parameter: assignable, but NOT readable (DP-O4). The host may have handed us
    /// the address of an uninitialised variable, so reading it is a mistake the language can
    /// prevent rather than document.
    OutParam,
    Let,
    LetMut,
}

type Scope = HashMap<String, (IrType, Binding)>;

/// The built-in rounding functions (SPEC-rounding). All `f64 -> f64`, all matching C's
/// `<math.h>` exactly — signed zero, NaN and infinities included (DP-R3).
///
/// They exist because `f64::floor` and friends are **not in `core`**, so a module that needs
/// to round had only `(x) as i32 as f64`, which saturates at `i32::MAX` and silently returns
/// 2,147,483,647 for any larger amount (measured).
pub const BUILTIN_ROUNDERS: &[&str] = &["floor", "ceil", "round", "trunc"];

pub fn check(module: &ast::Module) -> Result<IrModule, TypeError> {
    // Module-scoped error table (D17). Codes are positive i32 (parser-validated).
    let mut error_table: Scope2 = HashMap::new();
    let mut errors = Vec::with_capacity(module.errors.len());
    // Two names may not share a VALUE either (SPEC-fallible-calls DP-F9). The header emits a
    // `#define ML_ERR_<NAME>` per declaration, so a collision gives the host two constants
    // with one value: its `if (st == ML_ERR_A) … else if (st == ML_ERR_B)` always takes the
    // first branch, and two distinct module failures collapse into one.
    //
    // This was survivable while a code never crossed a function boundary — whoever wrote the
    // codes also read them. `try` propagation ends that: a code now arrives from a helper the
    // host has never seen, and the host has nothing but the number.
    let mut by_code: HashMap<i32, String> = HashMap::new();
    // lowercased name -> the spelling it was first declared with.
    let mut by_name: HashMap<String, String> = HashMap::new();
    for e in &module.errors {
        if let Some(first) = by_code.get(&e.code) {
            return Err(TypeError::new(format!(
                "error '{}' and error '{first}' both use code {} — a host maps a status to a \
                 name by its value, so two names with one value are indistinguishable at the \
                 boundary. Give them different codes",
                e.name, e.code
            )));
        }
        by_code.insert(e.code, e.name.clone());
        // Case-insensitively, for the same reason function names (below) and parameter names
        // (`check_fn`) are: the name is emitted verbatim as a constant in the generated Delphi
        // unit, and Pascal does not distinguish case. Measured before this check, `error E_Neg
        // = 1` + `error E_NEG = 2` compiled and put both `ML_M_ERR_E_Neg = 1;` and
        // `ML_M_ERR_E_NEG = 2;` into one `.pas` — the same identifier, declared twice.
        if let Some(first) = by_name.insert(e.name.to_ascii_lowercase(), e.name.clone()) {
            return Err(TypeError::new(format!(
                "duplicate error '{}' — it collides with '{first}'. Error names must be unique \
                 case-insensitively, because each becomes a constant in the generated Delphi \
                 unit and Pascal does not distinguish case",
                e.name
            )));
        }
        error_table.insert(e.name.clone(), e.code);
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
        // (DP-W4.) Two checks used to live here, and both have lost their reason.
        //
        // An internal function's name used to be emitted RAW into the generated Rust, so it
        // needed the target's reserved words rejected (`fn match` would not parse) and the
        // compiler's own prefixes reserved (`fn ml_floor` would collide with a helper).
        // Since the wrapper refactor every function is emitted as `ml_fn_<name>` — no user
        // function name reaches the generated Rust at all — so neither collision is reachable.
        //
        // Removing them takes a restriction AWAY. `fn match` and `fn type` are now legal, and
        // `export fn match`, which compiled all along because the `mlx_` prefix hid it, keeps
        // working. The same checks stay in force for PARAMETERS and LOCALS, which are still
        // emitted raw.
        if !f.exported {
            // D17's error ABI is a HOST-BOUNDARY convention: an i32 status plus an
            // `out_value` out-param appended to the *exported* signature. There is no
            // internal calling convention for it, and DP-C1 forbids calling a fallible
            // callee, so an internal `-> T!` is unreachable by construction. Left to
            // codegen it lowered wrong, because a non-exported fn is emitted with
            // `fallible = false`: `fail E` became a plain `return <code>;` — a silent
            // wrong answer for `-> i32!`, and a rustc E0308 for the other types that
            // reached the user only as "cargo build of generated crate failed".
            // (Lifted by SPEC-fallible-calls DP-F3.) An internal `-> T!` used to be rejected
            // here because DP-C1 made it uncallable, so codegen had no shape for it and
            // lowered `fail` as a plain `return <code>` — a silent wrong answer. `try` gives
            // it a call form and codegen gives it `Result<T, i32>`, so the ban is gone.
            //
            // A fallible internal function that returns a STRING is still rejected: the Q12
            // buffer belongs to the host and only an export has one (DP-F7).
            if f.fallible && f.ret == Type::Str {
                return Err(TypeError::new(format!(
                    "internal function '{}' returns `string!` — a returned string is written \
                     into the host's buffer, and only an `export` has one. Make it `export`, \
                     or return a scalar",
                    f.name
                )));
            }
        }
        functions.push(f);
    }

    // First pass: every signature, so calls resolve regardless of declaration order (DP-C4).
    let mut sigs: Sigs = HashMap::new();
    // Builtins go in first, so a user function of the same name collides below rather than
    // silently shadowing one. DP-R1: these are signatures, NOT lexer keywords — `let round = 1`
    // stays legal, because calls and variables are separate namespaces.
    for name in BUILTIN_ROUNDERS {
        sigs.insert(
            (*name).to_string(),
            Sig {
                params: vec![IrType::F64],
                ret: IrType::F64,
                fallible: false,
                // A builtin is emitted as `ml_<name>` and takes no out-param.
                out_param: None,
            },
        );
    }
    for f in &module.functions {
        if BUILTIN_ROUNDERS.contains(&f.name.as_str()) {
            return Err(TypeError::new(format!(
                "function '{}' collides with the built-in `{}` — rename it (the built-ins are \
                 {}, all f64 -> f64)",
                f.name,
                f.name,
                BUILTIN_ROUNDERS.join(", ")
            )));
        }
        sigs.insert(
            f.name.clone(),
            Sig {
                params: f.params.iter().map(|p| ir_type(p.ty)).collect(),
                ret: ir_type(f.ret),
                fallible: f.fallible,
                out_param: f.params.iter().find(|p| p.out).map(|p| p.name.clone()),
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
    /// The name of the callee's first `out` parameter, if it has one. A call expression has no
    /// syntax for passing a pointer, and `Sig.params` does not model one, so such a callee has
    /// to be rejected rather than silently type-checked against the wrong arity. The NAME is
    /// kept, not just a flag, so the diagnostic can point at the parameter the author has to
    /// deal with instead of making them find it.
    out_param: Option<String>,
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
            Expr::Number(_) | Expr::Int(_) | Expr::Bool(_) | Expr::Str(_) | Expr::Var(_) => {}
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
            // A try-call is a call-graph EDGE. If it were not walked here, a cycle through
            // `try` would slip past `reject_recursion` and overflow the host's stack — which
            // kills the process, not just the call.
            Stmt::TryCall { callee, args, .. } => {
                out.push(callee.clone());
                for a in args {
                    expr(a, out);
                }
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
        Type::Str => IrType::Str,
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
    // An EXPORTED parameter name is written into the `.h` and the `.pas`; an internal one is
    // not (measured: zero occurrences of either). So the languages it must avoid follow from
    // where it lands, not from a single global list — STATUS §4-2.
    let name_scope = if f.exported {
        crate::reserved::NameScope::Bindings
    } else {
        crate::reserved::NameScope::GeneratedModule
    };
    for p in &f.params {
        let targets = crate::reserved::reserving_targets_in(&p.name, name_scope);
        if !targets.is_empty() {
            return Err(TypeError::new(format!(
                "function '{}': parameter '{}' is a reserved word in {} — rename it",
                f.name,
                p.name,
                targets.join(", ")
            )));
        }
        // The generated header includes `<stdint.h>`, so its macros are already defined when
        // the declarations below them are read — and a macro is substituted, not shadowed.
        // Measured: a parameter named `INT32_MAX` emitted `double mlx_f(double INT32_MAX);`,
        // which the preprocessor turns into `double mlx_f(double 2147483647);`.
        if name_scope == crate::reserved::NameScope::Bindings {
            if let Some(header) = crate::reserved::included_macro(&p.name) {
                return Err(TypeError::new(format!(
                    "function '{}': parameter '{}' is a macro from <{header}>, which the \
                     generated header includes — the preprocessor would replace it with its \
                     value inside the declaration. Rename it.",
                    f.name, p.name
                )));
            }
        }
        // A parameter lands in the same emitted scope as the identifiers codegen injects, and
        // shadowing there is silent: a parameter named `__d` was captured by the `i32 /`
        // guard's divisor binding and made `__d / b` return 1 for every nonzero `b`.
        //
        // Scope-aware since the measurement in `reserved::generated_prefix_in`: for a name
        // that reaches the bindings the comparison is case-insensitive, because `ML_BUF` and
        // the generated `ml_buf` are one identifier to Delphi.
        if let Some(prefix) = crate::reserved::generated_prefix_in(&p.name, name_scope) {
            return Err(TypeError::new(format!(
                "function '{}': parameter '{}' starts with `{}`, which the compiler generates \
                 into the same scope — {}. Rename it.",
                f.name,
                p.name,
                prefix,
                crate::reserved::generated_prefix_reason(prefix)
            )));
        }
        if !seen_params.insert(p.name.to_ascii_lowercase()) {
            return Err(TypeError::new(format!(
                "function '{}': duplicate parameter '{}' — parameter names must be unique case-insensitively (Delphi binding)",
                f.name, p.name
            )));
        }
        // DP-O5: an `out` parameter is export-only. A call expression has no syntax for
        // passing a pointer, so an internal `fn` with one could never be called with it —
        // the same reason `-> T!` is export-only. Lifting this later is additive.
        if p.out && p.ty == Type::Str {
            return Err(TypeError::new(format!(
                "function '{}': parameter '{}' is `out string`, which is not supported — use \
                 `-> string!` instead, which delivers the string through the caller-allocates \
                 buffer (ml_buf/ml_cap/ml_needed). A function returns at most one string; \
                 several string outputs at once is not supported yet",
                f.name, p.name
            )));
        }
        if p.out && !f.exported {
            return Err(TypeError::new(format!(
                "function '{}': parameter '{}' is `out`, which is only allowed on an `export fn` \
                 — a call expression cannot pass a pointer, so an internal function could never \
                 receive one",
                f.name, p.name
            )));
        }
        let ty = ir_type(p.ty);
        scope.insert(
            p.name.clone(),
            (
                ty,
                if p.out {
                    Binding::OutParam
                } else {
                    Binding::Param
                },
            ),
        );
        params.push(IrParam {
            name: p.name.clone(),
            ty,
            out: p.out,
        });
    }
    // The fallible ABI synthesizes an `out_value` out-param; a user param of that name would
    // collide in the generated Rust (Grok verify). Reject it with a clear message.
    //
    // Case-insensitively, because the collision the message describes is worse in the `.pas`
    // than in the Rust: measured, `export fn f(OUT_VALUE: f64) -> f64!` emitted
    // `function mlx_f(OUT_VALUE: Double; out out_value: Double)` — one Pascal identifier,
    // twice in one parameter list.
    if f.fallible
        && f.params
            .iter()
            .any(|p| p.name.eq_ignore_ascii_case("out_value"))
    {
        return Err(TypeError::new(format!(
            "function '{}': parameter name 'out_value' is reserved in a fallible function \
             (it names the D17 out-param) — rename it",
            f.name
        )));
    }
    // SPEC-string-return: a returned string uses the Q12 caller-allocates protocol, which
    // means the module ALWAYS has a status to report — truncation is possible on every call.
    // So `!` is mandatory (DP-T1): the surface's one mark for "check the status" must not
    // come apart from the C-level `int32_t` return.
    if f.ret == Type::Str {
        if !f.fallible {
            return Err(TypeError::new(format!(
                "function '{}' returns `string`, so it must be declared `-> string!` — the host \
                 supplies the buffer (Q12), and a buffer too small is reported as a negative \
                 status, so every call returns one. Write `-> string!` and the header will \
                 declare `int32_t mlx_{}(…, char* ml_buf, int32_t ml_cap, int32_t* ml_needed)`",
                f.name, f.name
            )));
        }
        // Export-only, exactly as `-> T!` is (DP-O5 / SPEC-calls section 5.3): D17 and the
        // buffer triple are host-boundary conventions, and there is no internal calling
        // convention for either. The generic fallible check above already rejects an internal
        // `-> T!`, so this is unreachable today; it stays as a guard because the two rules
        // have different reasons and the other one could be relaxed first.
        if !f.exported {
            return Err(TypeError::new(format!(
                "function '{}' returns `string!`, which must be `export`ed — the caller-allocates \
                 buffer protocol is a host-boundary convention with no internal counterpart",
                f.name
            )));
        }
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
    // DP-O2: every path that RETURNS must have assigned every `out` parameter. Same spirit as
    // the check above — decidable, and the alternative is a host reading its own uninitialised
    // stack variable and believing the module put it there.
    let outs: Vec<&str> = params
        .iter()
        .filter(|p| p.out)
        .map(|p| p.name.as_str())
        .collect();
    if !outs.is_empty() {
        let mut assigned: Vec<&str> = Vec::new();
        check_outs_assigned(&body, &outs, &mut assigned, &f.name)?;
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

/// Definite assignment for `out` parameters (DP-O2).
///
/// Conservative and deliberately simple: an assignment inside an `if` or `while` body does
/// NOT count once control leaves it, because neither is guaranteed to run — Mathless has no
/// `else`, so there is no two-branch case to merge. A `return` inside such a body still sees
/// the assignments made along its own path, which is what makes the natural shape
/// (`if c { t = 0  return … }`) compile.
///
/// `fail` is exempt (DP-O3): on a non-zero status the host reads no out-param at all.
fn check_outs_assigned<'a>(
    body: &'a [IrStmt],
    outs: &[&str],
    assigned: &mut Vec<&'a str>,
    fname: &str,
) -> Result<(), TypeError> {
    for s in body {
        match s {
            IrStmt::AssignOut { name, .. } => {
                if !assigned.contains(&name.as_str()) {
                    assigned.push(name.as_str());
                }
            }
            IrStmt::Return(_) => {
                if let Some(missing) = outs.iter().find(|o| !assigned.contains(o)) {
                    return Err(TypeError::new(format!(
                        "function '{fname}': `out` parameter '{missing}' is not assigned on \
                         every path that returns — assign it before this `return`, or the host \
                         reads whatever was in its own variable"
                    )));
                }
            }
            // A branch body may not run, so its assignments do not survive it. Returns inside
            // it are still checked, against the path that reaches them.
            IrStmt::If { body, .. } | IrStmt::While { body, .. } => {
                let mut inner = assigned.clone();
                check_outs_assigned(body, outs, &mut inner, fname)?;
            }
            // A try-call has two exits, and they are treated differently on purpose.
            //
            // Its FAILURE exit is exempt, exactly as `fail` is: on a non-zero status the host
            // reads no out-param (D17 DP-E3 + DP-O3), so an unassigned out on that path is
            // not a defect — it is the contract.
            //
            // Its SUCCESS path continues, and if the destination IS an out parameter, that
            // counts as assigning it. `t = try g(x)` must satisfy DP-O2 the same way `t = 1`
            // does; without this arm it would not, and a correct program would be rejected.
            IrStmt::TryCall {
                dest: IrTryDest::AssignOut(name),
                ..
            } => {
                if !assigned.contains(&name.as_str()) {
                    assigned.push(name.as_str());
                }
            }
            // `return try g(x)` returns on success, so it is a return path like any other and
            // every out must already be assigned. Missing this arm would have let
            // `export fn f(out t: i32) -> i32! { return try g(1) }` through with `t` never
            // written — the host would read its own variable and believe the module wrote it,
            // which is the exact defect DP-O2 exists to prevent.
            IrStmt::TryCall {
                dest: IrTryDest::Return,
                ..
            } => {
                if let Some(missing) = outs.iter().find(|o| !assigned.contains(o)) {
                    return Err(TypeError::new(format!(
                        "function '{fname}': `out` parameter '{missing}' is not assigned on \
                         every path that returns — assign it before this `return`, or the host \
                         reads whatever was in its own variable"
                    )));
                }
            }
            IrStmt::TryCall { .. }
            | IrStmt::Fail(_)
            | IrStmt::Let { .. }
            | IrStmt::Assign { .. } => {}
        }
    }
    Ok(())
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
    //
    // `return try g(x)` counts too. It is a terminator by `ir::block_always_returns`'s
    // reckoning — that is why the all-paths check accepts it — and this scan matched only
    // `Return` and `Fail`, so the two views of "ends the block" disagreed. Measured: a
    // `return 999` after a `return try half(n)` compiled and shipped a `.dll`, while the same
    // shape after a plain `return` was refused. The statement was dead either way; only the
    // report went missing. The other three `try` destinations bind or assign and fall
    // through, so they are deliberately not terminators here either.
    let terminates = |s: &IrStmt| {
        matches!(
            s,
            IrStmt::Return(_)
                | IrStmt::Fail(_)
                | IrStmt::TryCall {
                    dest: IrTryDest::Return,
                    ..
                }
        )
    };
    let last = out.len().saturating_sub(1);
    if let Some(i) = out[..last].iter().position(terminates) {
        let kw = match &out[i] {
            IrStmt::Fail(_) => "fail",
            IrStmt::TryCall { .. } => "return try",
            _ => "return",
        };
        return Err(TypeError::new(format!(
            "function '{fname}': unreachable statement after `{kw}` — everything following it \
             is dead code"
        )));
    }
    Ok(out)
}

/// `try f(args)` in one of the three statement positions (SPEC-fallible-calls).
///
/// Every rejection here names the fix, because the message this replaces was a dead end:
/// "cannot be called in an expression yet" told the author nothing they could act on.
#[allow(clippy::too_many_arguments)]
fn check_try_call(
    dest: &ast::TryDest,
    callee: &str,
    args: &[Expr],
    scope: &mut Scope,
    ret: IrType,
    fname: &str,
    fallible: bool,
    sigs: &Sigs,
) -> Result<IrStmt, TypeError> {
    let Some(sig) = sigs.get(callee) else {
        return Err(TypeError::new(format!(
            "function '{fname}': unknown function '{callee}'"
        )));
    };
    // The marker is checked in BOTH directions, so it can never lie: a fallible callee
    // without `try` is rejected at the call site, and `try` on an infallible one here.
    if !sig.fallible {
        return Err(TypeError::new(format!(
            "function '{fname}': '{callee}' is not fallible, so `try` does not apply — it \
             cannot fail, and there is no status to propagate. Drop the `try`"
        )));
    }
    // DP-F4: a non-fallible caller has no status channel to leave through. Inferring `!` from
    // the body would change the exported ABI with no change to the source, which D17 already
    // rejected (DP-E1); defaulting is the silent-wrong-answer shape this repo keeps meeting.
    if !fallible {
        return Err(TypeError::new(format!(
            "function '{fname}': `try` needs a status to propagate into, but '{fname}' is not \
             fallible — declare it `-> {ret}!` so the failure can leave, or handle the case \
             without calling '{callee}'"
        )));
    }
    // (DP-F5 lifted by SPEC-export-wrappers.) An exported callee used to be rejected here
    // because its body was emitted against the C ABI while a `try` callee must return
    // `Result<T, i32>`. The wrapper refactor gave every function ONE body in the Rust-native
    // shape and moved the C ABI into a thin adapter, so there is nothing left to reject:
    // `export` now means only "also visible outside" (DP-C3), in the emission too.
    if let Some(out_name) = &sig.out_param {
        return Err(TypeError::new(format!(
            "function '{fname}': '{callee}' cannot be called because its parameter \
             '{out_name}' is an `out` — that is a pointer the host supplies, and a call \
             expression has no way to pass one"
        )));
    }
    // DP-F7: the returned string is written into the host's buffer, which a callee does not
    // have, and a string local has nowhere to live.
    if sig.ret == IrType::Str {
        return Err(TypeError::new(format!(
            "function '{fname}': '{callee}' returns `string!`, which cannot be `try`-called \
             yet — the bytes go into the host's buffer, so there is nowhere for the result to \
             land inside a module function"
        )));
    }
    if args.len() != sig.params.len() {
        return Err(TypeError::new(format!(
            "function '{fname}': '{callee}' expects {} argument(s), found {}",
            sig.params.len(),
            args.len()
        )));
    }
    let ty = sig.ret;
    let want: Vec<IrType> = sig.params.clone();
    let mut checked = Vec::with_capacity(args.len());
    for (i, (arg, w)) in args.iter().zip(want.iter()).enumerate() {
        let arg = check_expr(arg, scope, fname, sigs)?;
        if arg.ty != *w {
            return Err(TypeError::new(format!(
                "function '{fname}': '{callee}' argument {} expects {w}, found {}",
                i + 1,
                arg.ty
            )));
        }
        // The same guard an ordinary call applies to its arguments. A BUILT string (`a + b`,
        // `n as string`) is bytes written straight into the host's buffer, so it has nowhere
        // to live as an argument — and `try` was the one call form that never asked.
        reject_built_string(&arg, fname, &format!("as argument {} to '{callee}'", i + 1))?;
        checked.push(arg);
    }

    let ir_dest = match dest {
        ast::TryDest::Return => {
            if ty != ret {
                return Err(TypeError::new(format!(
                    "function '{fname}': return type mismatch: expected {ret}, found {ty}"
                )));
            }
            IrTryDest::Return
        }
        ast::TryDest::Let { name, mutable } => {
            // Same reserved-name policy a plain `let` gets — emitted raw into the module, and
            // present in no binding (STATUS §4-2).
            let targets = crate::reserved::reserving_targets_in(
                name,
                crate::reserved::NameScope::GeneratedModule,
            );
            if !targets.is_empty() {
                return Err(TypeError::new(format!(
                    "function '{fname}': local '{name}' is a reserved word in {}",
                    targets.join(", ")
                )));
            }
            if let Some(prefix) = crate::reserved::generated_prefix(name) {
                return Err(TypeError::new(format!(
                    "function '{fname}': local '{name}' starts with `{prefix}`, which the \
                     compiler generates into the same scope — {}",
                    crate::reserved::generated_prefix_reason(prefix)
                )));
            }
            // And the rest of what a plain `let` owes. A try-let binds a name like any other
            // binding form, so DP-L2 applies to it too — it was inserting straight into the
            // scope map, which let `let v = try g(v)` silently shadow the parameter `v`.
            if fallible && name == "out_value" {
                return Err(TypeError::new(format!(
                    "function '{fname}': local 'out_value' is reserved in a fallible function \
                     (it names the D17 out-param) — rename it"
                )));
            }
            if scope.contains_key(name) {
                return Err(TypeError::new(format!(
                    "function '{fname}': '{name}' is already in scope — no redeclaration or \
                     shadowing"
                )));
            }
            scope.insert(
                name.clone(),
                (
                    ty,
                    if *mutable {
                        Binding::LetMut
                    } else {
                        Binding::Let
                    },
                ),
            );
            IrTryDest::Let {
                name: name.clone(),
                mutable: *mutable,
            }
        }
        ast::TryDest::Assign(name) => {
            let (dty, binding) = match scope.get(name) {
                Some(&(t, b)) => (t, b),
                None => {
                    return Err(TypeError::new(format!(
                        "function '{fname}': unknown variable '{name}' — assignment needs a \
                         `let mut` local or an `out` parameter in scope"
                    )))
                }
            };
            if dty != ty {
                return Err(TypeError::new(format!(
                    "function '{fname}': cannot assign {ty} to '{name}' of type {dty} — type \
                     mismatch"
                )));
            }
            match binding {
                Binding::LetMut => IrTryDest::Assign(name.clone()),
                Binding::OutParam => IrTryDest::AssignOut(name.clone()),
                Binding::Let => {
                    return Err(TypeError::new(format!(
                        "function '{fname}': '{name}' is not mutable — declare it `let mut`"
                    )))
                }
                Binding::Param => {
                    return Err(TypeError::new(format!(
                        "function '{fname}': cannot assign to parameter '{name}' — parameters \
                         are immutable (D16: an argument is borrowed for the call)"
                    )))
                }
            }
        }
    };

    Ok(IrStmt::TryCall {
        dest: ir_dest,
        callee: callee.to_string(),
        args: checked,
        ty,
    })
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
            // A returned string is either BORROWED (a literal, or a `string` parameter) or
            // BUILT here and now, straight into the caller's buffer.
            //
            // DP-T5 used to allow only the borrowed forms. SPEC-string-concat opens the built
            // form and reopens DP-S2 with it, narrowly: the only bytes the module produces are
            // ASCII `-` and `0`-`9` (DP-K9), which are the same bytes in every encoding this
            // project has left undecided. Borrowed pieces stay opaque and are copied verbatim.
            //
            // `return` is the ONLY position a built string may appear in (DP-K3), and that is
            // enforced where the string could otherwise escape — see `no_built_string` below.
            // The reason is the diagnostic that has always been right: there is nowhere for it
            // to live. Building it here works precisely because the destination already exists.
            let e = if ret == IrType::Str {
                flatten_concat(e)
            } else {
                e
            };
            if ret == IrType::Str
                && !matches!(
                    e.kind,
                    IrExprKind::ConstStr(_) | IrExprKind::Var { .. } | IrExprKind::Concat(_)
                )
            {
                return Err(TypeError::new(format!(
                    "function '{fname}': a returned string must be a literal, a `string` \
                     parameter, or a concatenation of those and `i32 as string`"
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
                Binding::LetMut | Binding::OutParam => {}
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
            // An out-param assignment writes THROUGH a pointer, so it gets its own IR
            // statement — codegen must not be able to emit a plain `name = …` for it.
            Ok(if binding == Binding::OutParam {
                IrStmt::AssignOut {
                    name: name.clone(),
                    value,
                }
            } else {
                IrStmt::Assign {
                    name: name.clone(),
                    value,
                }
            })
        }
        Stmt::TryCall { dest, callee, args } => {
            check_try_call(dest, callee, args, scope, ret, fname, fallible, sigs)
        }
        Stmt::Let {
            name,
            value,
            mutable,
        } => {
            // Check the RHS in the CURRENT scope first, so `let x = x` is use-before-def.
            let value = check_expr(value, scope, fname, sigs)?;
            // A string may be a parameter or a `-> string!` return (SPEC-string-return), but
            // not a local: with no allocator there is nowhere for one to live. A parameter is
            // borrowed for the call and a return is copied straight into the host's buffer;
            // a local is the one position with no owner, so it stays rejected.
            if value.ty == IrType::Str {
                return Err(TypeError::new(format!(
                    "function '{fname}': local '{name}' would be a `string`, which is not \
                     supported — a string can be a parameter or a `-> string!` return, but \
                     there is nowhere for a local to live (the module has no allocator). \
                     Compare it in place, or return it directly, instead of binding it"
                )));
            }
            // A local is emitted raw into the generated module and appears in no binding —
            // measured as zero occurrences in the `.h` and the `.pas` (STATUS §4-2).
            let targets = crate::reserved::reserving_targets_in(
                name,
                crate::reserved::NameScope::GeneratedModule,
            );
            if !targets.is_empty() {
                return Err(TypeError::new(format!(
                    "function '{fname}': local '{name}' is a reserved word in {} — rename it",
                    targets.join(", ")
                )));
            }
            // Same reason as for parameters: a local shares the emitted scope with codegen's
            // own bindings, and Rust shadowing is silent.
            if let Some(prefix) = crate::reserved::generated_prefix(name) {
                return Err(TypeError::new(format!(
                    "function '{fname}': local '{name}' starts with `{prefix}`, which the \
                     compiler generates into the same scope — {}. Rename it.",
                    crate::reserved::generated_prefix_reason(prefix)
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

/// Collapse a `string` `+` tree into one flat, source-ordered [`IrExprKind::Concat`].
///
/// Anything that is not a string `+` is returned untouched — in particular a lone literal or
/// parameter keeps the #92 shape exactly, so the existing lowering is not disturbed.
///
/// Flat matters: codegen has to sum the piece lengths before writing a byte (Q12), and a sum
/// over a list needs no recursion in the emitted module.
fn flatten_concat(e: IrExpr) -> IrExpr {
    fn is_concat(e: &IrExpr) -> bool {
        e.ty == IrType::Str
            && matches!(
                e.kind,
                IrExprKind::Binary {
                    op: IrBinOp::Add,
                    ..
                }
            )
    }
    fn collect(e: IrExpr, out: &mut Vec<IrExpr>) {
        if is_concat(&e) {
            let IrExprKind::Binary { lhs, rhs, .. } = e.kind else {
                unreachable!("is_concat just matched Binary");
            };
            collect(*lhs, out);
            collect(*rhs, out);
        } else {
            out.push(e);
        }
    }
    // A lone `n as string` is a built string too — one piece, produced by the module rather
    // than borrowed. It takes the same append path, so it becomes a one-piece list rather
    // than a special case in codegen.
    if !is_concat(&e) {
        if matches!(e.kind, IrExprKind::Cast { .. }) && e.ty == IrType::Str {
            return IrExpr {
                ty: IrType::Str,
                kind: IrExprKind::Concat(vec![e]),
            };
        }
        return e;
    }
    let mut pieces = Vec::new();
    collect(e, &mut pieces);
    IrExpr {
        ty: IrType::Str,
        kind: IrExprKind::Concat(pieces),
    }
}

/// Is this a string the module BUILDS, as opposed to one it borrows?
///
/// A built string exists only while it is being written into the caller's buffer, so it may
/// appear in exactly one place: the `return` of a `-> string!` function (DP-K3). Everywhere
/// else it would need somewhere to live, and the module has no allocator.
fn is_built_string(e: &IrExpr) -> bool {
    e.ty == IrType::Str
        && matches!(
            e.kind,
            IrExprKind::Concat(_)
                | IrExprKind::Cast { .. }
                | IrExprKind::Binary {
                    op: IrBinOp::Add,
                    ..
                }
        )
}

/// Refuse a built string outside `return`, naming the position so the message is actionable.
fn reject_built_string(e: &IrExpr, fname: &str, position: &str) -> Result<(), TypeError> {
    if is_built_string(e) {
        return Err(TypeError::new(format!(
            "function '{fname}': a built string cannot be used {position} — the module has no \
             allocator, so a concatenation or `i32 as string` exists only while it is written \
             into the caller's buffer, which happens at `return`"
        )));
    }
    Ok(())
}

fn check_expr(e: &Expr, scope: &Scope, fname: &str, sigs: &Sigs) -> Result<IrExpr, TypeError> {
    match e {
        Expr::Number(n) => Ok(IrExpr {
            ty: IrType::F64,
            kind: IrExprKind::ConstF64(*n),
        }),
        // The lexer already guaranteed ASCII and no escapes (DP-S4), so the bytes reaching
        // codegen are exactly what the author typed.
        Expr::Str(s) => Ok(IrExpr {
            ty: IrType::Str,
            kind: IrExprKind::ConstStr(s.clone()),
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
            // DP-O4: an `out` parameter is write-only. The host passes the address of one of
            // its own variables, and nothing says that variable was initialised — so reading
            // it here would read whatever the host happened to have on its stack.
            Some(&(_, Binding::OutParam)) => Err(TypeError::new(format!(
                "function '{fname}': cannot read `out` parameter '{name}' — it is write-only, \
                 because the host may pass the address of an uninitialised variable. Take a \
                 separate input parameter if you need the value."
            ))),
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
                    "function '{fname}': '{name}' is fallible (`-> T!`) — call it with `try`, \
                     as in `let x = try {name}(…)`, so its failure propagates. `try` is a \
                     statement form and cannot appear inside a larger expression"
                )));
            }
            // An `out` parameter is a POINTER in the generated code, and a call expression has
            // no syntax for taking an address. `Sig.params` does not model the pointer either,
            // so without this check `c(a, 7)` type-checks against a signature that does not
            // exist and fails later inside the generated crate — a diagnostic with no source
            // position, at the wrong layer.
            if let Some(out_name) = &sig.out_param {
                return Err(TypeError::new(format!(
                    "function '{fname}': '{name}' cannot be called because its parameter \
                     '{out_name}' is an `out` — that is a pointer the host supplies, and a call \
                     expression has no way to pass one. Split the computation into a plain \
                     function that both can call, or inline it"
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
                // A borrowed string may be passed on; a built one may not (DP-K3). The callee
                // would receive a pointer to bytes that were never written anywhere.
                reject_built_string(&arg, fname, &format!("as argument {} to '{name}'", i + 1))?;
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
            // Numeric only, plus `i32 as string`. `bool` is neither a source nor a target:
            // numbers are not truthy and truth is not numeric, which is the stance conditions
            // already take.
            //
            // `i32 as string` is the ONLY way into `string` (DP-K5/K6). `f64 as string` is out
            // of scope because it is not needed, not because it is hard: decimal money reduces
            // to integer cents with `round(x*100.0) as i32`, and that was measured through a
            // real host. Nothing converts OUT of `string` — parsing text is a different slice,
            // and `as` must not look like it does that.
            let ok = matches!(
                (operand.ty, to),
                (IrType::I32, IrType::F64)
                    | (IrType::F64, IrType::I32)
                    | (IrType::I32, IrType::I32)
                    | (IrType::F64, IrType::F64)
                    | (IrType::I32, IrType::Str)
            );
            if !ok {
                // The hint has to fit the SOURCE type, not just the target: telling someone
                // casting a `bool` to "convert to whole units first" is advice for a problem
                // they do not have. Caught by reading the rejection-list transcript, which is
                // what that list is for.
                let hint = if to == IrType::Str && operand.ty == IrType::F64 {
                    " — only `i32 as string` exists; for a decimal, convert to whole units \
                     first (for example `round(x * 100.0) as i32`)"
                } else if to == IrType::Str {
                    " — only `i32 as string` exists"
                } else if operand.ty == IrType::Str {
                    " — a string is opaque bytes to the module; reading a number out of text \
                     is not part of this language"
                } else {
                    " — `as` converts between f64 and i32 only, and bool is not convertible"
                };
                return Err(TypeError::new(format!(
                    "function '{fname}': cannot cast {} to {to}{hint}",
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
            // DP-D5: `/ 0` and `% 0` written literally are rejected. The operators are total
            // at runtime, so this changes no value — it catches the one case that is
            // statically decidable and certainly a mistake. Deliberately SYNTACTIC, not
            // constant folding: `let z = 0  a / z` still compiles, because the runtime rule
            // covers it. `f64 / 0.0` also still compiles — that is `inf`, a defined value.
            if matches!(op, BinOp::Div | BinOp::Rem) && matches!(**rhs, Expr::Int(0)) {
                return Err(TypeError::new(format!(
                    "function '{fname}': dividing by the literal 0 is rejected — `{}` by zero is \
                     defined as 0 at runtime (SPEC-i32-division DP-D1), but writing the zero out \
                     is always a mistake",
                    if matches!(op, BinOp::Div) { "/" } else { "%" }
                )));
            }
            let lhs = check_expr(lhs, scope, fname, sigs)?;
            let rhs = check_expr(rhs, scope, fname, sigs)?;
            // Comparing a BUILT string would need its bytes to exist before the caller's
            // buffer is in play (DP-K3). Concatenating two of them is fine — that is one
            // longer append, not a second place to live.
            if matches!(op, BinOp::Eq | BinOp::Ne) {
                reject_built_string(&lhs, fname, "on the left of a comparison")?;
                reject_built_string(&rhs, fname, "on the right of a comparison")?;
            }
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
        //
        // `+` also concatenates two strings (SPEC-string-concat DP-K2). It does NOT accept a
        // string and a number: DP-K1 keeps the no-implicit-conversion rule exactly as strict
        // for strings as it already is for `f64` and `i32`, so `n as string` is required. The
        // message says so rather than only refusing — the fix is one token away.
        BinOp::Add if lt == IrType::Str || rt == IrType::Str => match (lt, rt) {
            (IrType::Str, IrType::Str) => Ok((map_op(op), IrType::Str)),
            (IrType::Str, other) | (other, IrType::Str) => Err(TypeError::new(format!(
                "function '{fname}': cannot concatenate string and {other} — this language \
                 never converts implicitly, so write `<{other} expression> as string` \
                 (only `i32 as string` exists today)"
            ))),
            _ => unreachable!("guarded on one side being Str"),
        },
        BinOp::Add | BinOp::Sub | BinOp::Mul => match (lt, rt) {
            (IrType::F64, IrType::F64) => Ok((map_op(op), IrType::F64)),
            (IrType::I32, IrType::I32) => Ok((map_op(op), IrType::I32)),
            _ => Err(TypeError::new(format!(
                "function '{fname}': operator {op:?} expects two f64 or two i32 operands, found {lt} and {rt}"
            ))),
        },
        // `/` takes both numeric types. The two are lowered differently, though: `f64 /` is a
        // plain operator because `f64 /0` is `inf`, while `i32 /` is guarded in codegen — the
        // plain Rust operator panics on BOTH `b == 0` and `i32::MIN / -1`, and a panic in a
        // generated module spins in the emitted `loop {}` handler (STATUS §5-4).
        BinOp::Div => match (lt, rt) {
            (IrType::F64, IrType::F64) => Ok((map_op(op), IrType::F64)),
            (IrType::I32, IrType::I32) => Ok((map_op(op), IrType::I32)),
            _ => Err(TypeError::new(format!(
                "function '{fname}': operator / expects two f64 or two i32 operands, found {lt} and {rt}"
            ))),
        },
        // DP-D4: `%` is i32-only. Floating-point remainder is a separate semantic argument
        // (C's `fmod` truncates toward zero, IEEE `remainder` rounds to nearest-even), and
        // this slice does not settle it.
        BinOp::Rem => match (lt, rt) {
            (IrType::I32, IrType::I32) => Ok((map_op(op), IrType::I32)),
            _ => Err(TypeError::new(format!(
                "function '{fname}': operator % expects two i32 operands, found {lt} and {rt} \
                 — f64 remainder is out of scope (SPEC-i32-division DP-D4)"
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
        BinOp::Rem => IrBinOp::Rem,
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

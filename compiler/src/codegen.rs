//! W4 codegen: typed IR → Rust source (`extern "C"` cdylib), then build to a DLL.
//!
//! Per D19 this is the **provisional rustc lowering**: emit Rust, `cargo build
//! --crate-type cdylib`. The IR is backend-independent, so a C-emit backend can be
//! added later without touching the front/middle end.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::ir::*;
use crate::typeck::Rounder;

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct CodegenError {
    pub message: String,
}

impl CodegenError {
    pub fn new(message: impl Into<String>) -> Self {
        CodegenError {
            message: message.into(),
        }
    }
}

impl std::fmt::Display for CodegenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "codegen error: {}", self.message)
    }
}

impl std::error::Error for CodegenError {}

/// The file named `name` that cargo produced under `root`, found rather than reconstructed.
///
/// A bounded, breadth-first walk — it never recurses, and it caps at four levels so a
/// pathological tree cannot turn the search into a hang.
///
/// **Shallowest wins, and exactly one at that depth.** Cargo puts the deliverable at
/// `<target>/[<triple>/]release/<name>` and a second copy one level deeper in `deps/`
/// (measured: the first strict "exactly one anywhere" version reported *"found 2 files named
/// 'discount.dll'"* on an ordinary build). Taking the shallowest names the deliverable, and
/// requiring exactly one *there* keeps ambiguity an error rather than a coin flip — `root` is
/// a `target` directory inside a temp crate this call just created, so a tie would mean
/// something is in it that this build did not put there.
///
/// This exists because the layout is not ours. `--target-dir` tells cargo where to work; it
/// does not fix the shape underneath, and `CARGO_BUILD_TARGET` adds a `<triple>/` component
/// (measured — see the call site). Enumerating the variables that reshape it would be a fourth
/// hand-kept list in a repository that has watched three of them fail.
fn single_artifact(root: &Path, name: &str) -> Result<PathBuf, CodegenError> {
    const MAX_LEVELS: usize = 4;
    let mut level = vec![root.to_path_buf()];
    for _ in 0..MAX_LEVELS {
        let mut hits: Vec<PathBuf> = Vec::new();
        let mut next = Vec::new();
        for dir in &level {
            let Ok(entries) = std::fs::read_dir(dir) else {
                continue;
            };
            for e in entries.flatten() {
                let p = e.path();
                if p.is_dir() {
                    next.push(p);
                } else if p.file_name().and_then(|n| n.to_str()) == Some(name) {
                    hits.push(p);
                }
            }
        }
        match hits.len() {
            0 => {}
            1 => return Ok(hits.remove(0)),
            _ => {
                hits.sort();
                return Err(CodegenError::new(format!(
                    "cargo left {} files named '{name}' at the same depth under {}, and \
                     choosing between them would be a guess: {:?}",
                    hits.len(),
                    root.display(),
                    hits
                )));
            }
        }
        if next.is_empty() {
            break;
        }
        next.sort();
        level = next;
    }
    Err(CodegenError::new(format!(
        "cargo reported success but produced no '{name}' anywhere under {} (searched \
         {MAX_LEVELS} levels)",
        root.display()
    )))
}

/// The `[profile.release]` written into every generated crate.
///
/// A `pub const` rather than an inline literal so a test can read what is actually shipped
/// instead of restating it — the pinned settings are load-bearing, not cosmetic.
///
/// `overflow-checks = false` is the newest of them and the reason for this constant. The
/// profile pinned `panic`, `strip`, `lto` and `opt-level` and left that one to cargo's
/// default, so an ambient `CARGO_PROFILE_RELEASE_OVERFLOW_CHECKS=true` reached the generated
/// crate and changed the shipped module. Measured with a C host on the built `.dll`:
/// `bump(2147483647)` returned `-2147483648` normally and **hung the calling thread** under
/// that variable (`ml_panic`'s `loop {}`, STATUS §5-4).
///
/// It is belt-and-braces, not the fix: cargo's env vars override a manifest profile, so this
/// alone would not hold. The lowering emits `wrapping_*` for i32 so the semantics live in the
/// code. What this pin still buys is the emitted HELPERS — `ml_slen`'s `n += 1`, `ml_wint`'s
/// index arithmetic — which are hand-written Rust the lowering never touches.
pub const CARGO_TOML_PROFILE: &str = "[profile.release]\npanic = \"abort\"\nstrip = true\nlto = true\nopt-level = \"z\"\noverflow-checks = false\n";

/// The module name [`crate::compile_to_rust`] uses when the caller has none.
///
/// That entry point lowers a source string with no file behind it, so there is no stem to
/// take a name from. It is the right shape for the ~100 tests that read the emitted Rust to
/// check a LOWERING, and the wrong shape for anything that then builds an artifact: the
/// fingerprint export would be `ml_iface_hash_unnamed` inside a DLL called something else.
///
/// [`build_cdylib`] refuses that combination rather than shipping it, so the divergence is a
/// loud failure at the one place a name is pinned to an artifact. Use
/// [`crate::compile_to_rust_named`] when the name matters.
pub const UNNAMED_MODULE: &str = "unnamed";

/// Emit Rust source implementing the IR module as a C-ABI cdylib (D18 exports:
/// `mlx_<fn>` + reserved `ml_module_abi_version` + `ml_iface_hash_<module>`).
///
/// `module_name` is the module's name — the source file's stem, the same string that becomes
/// the crate name, the DLL name, the C header guard and the Delphi unit name. It reaches the
/// back end only because the fingerprint export carries it (`SPEC-qualified-iface-hash`):
/// every module used to export an unqualified `ml_iface_hash`, so a host that LINKED two of
/// them got one binding chosen by the linker and called the other with no interface check.
pub fn emit(module: &IrModule, module_name: &str) -> Result<String, CodegenError> {
    let mut out = String::new();
    out.push_str("// Generated by mlc. Do not edit.\n");
    out.push_str("#![no_std]\n");
    out.push_str("#![allow(unused_parens)]\n");
    // A surface `let mut` that is never reassigned is legal Mathless; don't make rustc
    // complain about the faithful lowering.
    out.push_str("#![allow(unused_mut)]\n\n");
    out.push_str("#[no_mangle]\n");
    // Interpolate the single-source ABI version (D18) so the emitted module can't drift.
    let _ = writeln!(
        out,
        "pub extern \"C\" fn ml_module_abi_version() -> u32 {{ {} }}\n",
        crate::abi::ML_MODULE_ABI_VERSION
    );

    // The interface fingerprint (SPEC-iface-hash). The version above is a constant and so
    // says nothing about THIS module's signatures — measured: two modules with incompatible
    // interfaces both reported 1, resolved every symbol, and then returned a wrong number
    // and an access violation. A host compares this value against the constant its header
    // pinned and refuses the module when they differ.
    //
    // Qualified with the module name (SPEC-qualified-iface-hash). Unqualified, this is the
    // ONE reserved export whose value differs per module while its name does not, so a host
    // that links two modules resolves one of them for both — measured: `cl /W4` said nothing
    // and the second module was called with no interface check. `ml_module_abi_version`
    // deliberately keeps its bare name: its value is a compiler constant, identical in every
    // module, so the same collision is harmless there, and it is D18's bootstrap — the only
    // way a host can ask a module which ABI it speaks.
    out.push_str("#[no_mangle]\n");
    let _ = writeln!(
        out,
        "pub extern \"C\" fn ml_iface_hash_{}() -> u64 {{ 0x{:016X} }}\n",
        module_name,
        crate::iface::fingerprint(module)
    );

    emit_string_helper(module, &mut out);
    emit_strout_helper(module, &mut out);
    emit_slen_helper(module, &mut out);
    emit_concat_helpers(module, &mut out);
    // After the concat set, because `ml_sublen` calls `ml_slen` — which `builds_strings` has
    // already pulled in, since a span IS a built string.
    emit_span_helpers(module, &mut out);
    emit_span_compare_helper(module, &mut out);
    // Before both of its users, because both call `ml_trunc_raw`. Rust does not care about
    // definition order, but a reader does, and so does the next person adding a third caller.
    emit_trunc_helpers(module, &mut out);
    emit_rounding_helpers(module, &mut out);
    emit_fixed_helpers(module, &mut out);

    for f in &module.functions {
        emit_function(f, &mut out)?;
        out.push('\n');
    }

    // A no_std cdylib requires a panic handler. Nothing on today's surface can reach this one:
    // `f64 /0` is inf, `as` saturates, i32 `+ - *` and unary `-` are emitted as `wrapping_*`
    // and i32 `/`/`%` are emitted guarded below, so neither an overflow, a zero divisor nor
    // `i32::MIN / -1` reaches a panicking operator. So it exists to satisfy `no_std`, not to
    // handle anything.
    //
    // "integer arithmetic wraps because release leaves overflow-checks off" is what this said
    // until the wrapping lowering landed, and it was not true of the artifact: an ambient
    // `CARGO_PROFILE_RELEASE_OVERFLOW_CHECKS=true` turned the checks back on and `bump(i32::MAX)`
    // hung a real C host (measured). A reason that an environment variable can revoke is not a
    // reason.
    //
    // What it does if a future slice DOES reach it matters, because it is easy to get wrong:
    // in `no_std` the `#[panic_handler]` IS the panic runtime, and the profile's
    // `panic = "abort"` only drops unwinding tables — it does not call abort(). So a panic
    // lands here and spins, hanging the calling host thread while the process stays up. That
    // is the `while` liveness class, not the recursion class (STATUS §5-4).
    out.push_str("#[panic_handler]\nfn ml_panic(_: &core::panic::PanicInfo) -> ! { loop {} }\n");
    Ok(out)
}

fn emit_function(f: &IrFunction, out: &mut String) -> Result<(), CodegenError> {
    // An array return has no `return` statement -- the value IS the caller's buffer, and the
    // status is emitted after the body. Typeck exempts it for the same reason; this is the
    // backend half of that exemption, and leaving it out made every correct program fail here
    // with a message about the one thing that was not wrong (measured).
    if !matches!(f.ret, IrType::Array(_)) && !block_always_returns(&f.body) {
        // Backend safety net: typeck rejects this for real source (see `typeck::check`), but
        // directly-built IR could still fall off the end without returning a typed value.
        return Err(CodegenError::new(format!(
            "function '{}' may not return on all paths",
            f.name
        )));
    }

    // Declared `out` params become `*mut T`, in source order — in the BODY as well as in the
    // adapter, so the adapter forwards the pointer rather than re-deriving it.
    let params: Vec<String> = rust_params(&f.params);

    // ── The body. Every Mathless function has exactly one, and it is a plain Rust `fn`
    // (SPEC-export-wrappers DP-W1). It carries the Rust-native shape: `T` when infallible,
    // `Result<T, i32>` when fallible.
    //
    // The name is `ml_fn_<name>`, never the user's own. `ml_` is already reserved from user
    // identifiers (#85), so no user name reaches the generated Rust as a function — which is
    // why `export fn type` and `export fn match` keep compiling (measured: they compile today
    // only because the `mlx_` prefix hides them from rustc's keyword list) and why the
    // internal-name reserved-word check loses its reason to exist.
    let mut body_params = params.clone();
    let (body_ret, abi) = if let IrType::Array(elem) = f.ret {
        // The same triple as a string return, and for the same reason (RetAbi::ArrayOut): the
        // elements have nowhere to live until they are in the caller's buffer. The pointer is
        // `*mut` of the ELEMENT type, so `[f64]` writes 8-byte strides and `[bool]` 1-byte
        // ones -- the width the Delphi unit's `PBoolean` promises on the other side.
        body_params.push(format!("ml_buf: *mut {}", rust_type(elem.scalar())));
        body_params.push("ml_cap: i32".to_string());
        body_params.push("ml_needed: *mut i32".to_string());
        ("i32".to_string(), RetAbi::ArrayOut(elem))
    } else if f.ret == IrType::Str {
        // Q12's triple goes to the BODY, not just the adapter — see RetAbi::StringOut.
        body_params.push("ml_buf: *mut u8".to_string());
        body_params.push("ml_cap: i32".to_string());
        body_params.push("ml_needed: *mut i32".to_string());
        ("i32".to_string(), RetAbi::StringOut)
    } else if f.fallible {
        (
            format!("Result<{}, i32>", rust_type(f.ret)),
            RetAbi::Fallible,
        )
    } else {
        (rust_type(f.ret).to_string(), RetAbi::Plain)
    };
    let _ = writeln!(
        out,
        "fn ml_fn_{}({}) -> {} {{",
        f.name,
        body_params.join(", "),
        body_ret
    );
    for s in &f.body {
        emit_stmt(s, 1, abi, out);
    }
    // An array body has no `return`: reaching the end IS success, and the status says so.
    // Every failure before this point has already returned -- truncation, an out-of-range
    // write, or a `fail` placed before `result` (2.4b refuses one placed after it).
    if matches!(abi, RetAbi::ArrayOut(_)) {
        out.push_str("    0\n");
    }
    out.push_str("}\n");

    // An internal function stops here: no `#[no_mangle]`, so it never reaches the export
    // table (measured in acceptance C, not asserted here).
    if !f.exported {
        return Ok(());
    }

    // ── The adapter. This is the ONLY place the C ABI is spoken, and it contains no logic:
    // it forwards the arguments, then converts the body's Rust-native result into D17's
    // status + out-param or Q12's buffer triple.
    //
    // SPEC-export-wrappers DP-W2 puts it immediately after its body, so the two halves of one function read
    // together.
    let args: Vec<String> = rust_args(&f.params);
    let call = format!("ml_fn_{}({})", f.name, args.join(", "));
    let mut sig = params;

    out.push_str("#[no_mangle]\n");
    if let IrType::Array(elem) = f.ret {
        // Pure forward, exactly like the string return: the body already speaks the buffer
        // protocol, so this adapter converts nothing. That is the point of the split -- the C
        // ABI is spoken in exactly one place (SPEC-export-wrappers DP-W2).
        sig.push(format!("ml_buf: *mut {}", rust_type(elem.scalar())));
        sig.push("ml_cap: i32".to_string());
        sig.push("ml_needed: *mut i32".to_string());
        let mut a = rust_args(&f.params);
        a.extend([
            "ml_buf".to_string(),
            "ml_cap".to_string(),
            "ml_needed".to_string(),
        ]);
        let _ = writeln!(
            out,
            "pub extern \"C\" fn mlx_{}({}) -> i32 {{ ml_fn_{}({}) }}\n",
            f.name,
            sig.join(", "),
            f.name,
            a.join(", ")
        );
        return Ok(());
    }
    if f.ret == IrType::Str {
        // Q12: the module never allocates. The body already speaks the buffer protocol
        // (RetAbi::StringOut), so this adapter has nothing to convert — it forwards. That is
        // the point of the split: the C ABI is spoken in exactly one place, and here it is
        // identical to what it was before this slice (asserted byte-for-byte in the golden).
        sig.push("ml_buf: *mut u8".to_string());
        sig.push("ml_cap: i32".to_string());
        sig.push("ml_needed: *mut i32".to_string());
        let args_with_buf = {
            let mut a = rust_args(&f.params);
            a.extend([
                "ml_buf".to_string(),
                "ml_cap".to_string(),
                "ml_needed".to_string(),
            ]);
            a.join(", ")
        };
        let _ = writeln!(
            out,
            "pub extern \"C\" fn mlx_{}({}) -> i32 {{",
            f.name,
            sig.join(", ")
        );
        let _ = writeln!(out, "    ml_fn_{}({})", f.name, args_with_buf);
    } else if f.fallible {
        // D17: an `i32` status return plus a `*mut T` out-param, appended after every
        // declared out (DP-O1 — the return value is always last).
        //
        // `*out_value` is written ONLY on the success arm. Writing it first and choosing the
        // status afterwards would compile, would keep the status correct, and would clobber
        // the host's variable on the failure path — which DP-E3 says the host may then read
        // as untouched.
        sig.push(format!("out_value: *mut {}", rust_type(f.ret)));
        let _ = writeln!(
            out,
            "pub extern \"C\" fn mlx_{}({}) -> i32 {{",
            f.name,
            sig.join(", ")
        );
        let _ = writeln!(
            out,
            "    match {call} {{ Ok(__v) => {{ unsafe {{ *out_value = __v; }} 0 }} Err(__e) => __e }}"
        );
    } else {
        // Infallible: the value IS the C return, so the adapter is a forwarding call.
        let _ = writeln!(
            out,
            "pub extern \"C\" fn mlx_{}({}) -> {} {{",
            f.name,
            sig.join(", "),
            rust_type(f.ret)
        );
        let _ = writeln!(out, "    {call}");
    }
    out.push_str("}\n");
    Ok(())
}

fn rust_type(t: IrType) -> &'static str {
    match t {
        // Borrowed for the call (D16 rule 1). NUL-terminated, never owned, never allocated.
        IrType::Str => "*const u8",
        IrType::F64 => "f64",
        IrType::Bool => "bool",
        IrType::I32 => "i32",
        // Borrowed exactly like a string, and read-only for the same reason (D16 rule 1).
        // The LENGTH is a second parameter the caller of this function appends — see
        // `rust_params` (SPEC-array-input DP-A2).
        IrType::Array(IrArrayElem::F64) => "*const f64",
        IrType::Array(IrArrayElem::Bool) => "*const bool",
        IrType::Array(IrArrayElem::I32) => "*const i32",
    }
}

/// One Mathless parameter becomes one or TWO Rust parameters: an array brings the companion
/// length with it, sitting immediately after its pointer and before whatever the author
/// declared next (SPEC-array-input 2.2). Shared by the body and the adapter so the two lists
/// cannot drift.
fn rust_params(params: &[IrParam]) -> Vec<String> {
    let mut out = Vec::with_capacity(params.len());
    for p in params {
        if p.out {
            out.push(format!("{}: *mut {}", p.name, rust_type(p.ty)));
        } else {
            out.push(format!("{}: {}", p.name, rust_type(p.ty)));
        }
        if matches!(p.ty, IrType::Array(_)) {
            out.push(format!("{}_len: i32", p.name));
        }
    }
    out
}

/// The argument names for a forwarding call, in the same shape `rust_params` declares.
fn rust_args(params: &[IrParam]) -> Vec<String> {
    let mut out = Vec::with_capacity(params.len());
    for p in params {
        out.push(p.name.clone());
        if matches!(p.ty, IrType::Array(_)) {
            out.push(format!("{}_len", p.name));
        }
    }
    out
}

/// How a function BODY returns. Only two shapes, because a body never speaks the C ABI —
/// that lives in the adapter (SPEC-export-wrappers).
///
/// This used to have four variants, two of them C-facing (`Status` for D17's status +
/// out-param, `StringBuffer` for Q12's triple). Splitting exports into a body plus an adapter
/// deleted both: `return` and `fail` now lower one way for an infallible function and one way
/// for a fallible one, whether or not anyone exports it.
#[derive(Clone, Copy, PartialEq, Eq)]
enum RetAbi {
    /// Infallible: `return e` is `return e`.
    Plain,
    /// Fallible: `Result<T, i32>` — the only shape needing no invented value on the failure
    /// path (DP-F6). `Option` would lose the D17 code.
    Fallible,
    /// `-> string!`: the body itself takes the Q12 buffer triple and returns the status.
    ///
    /// It has to. A built string (SPEC-string-concat) has no representation to hand back —
    /// the module has no allocator, so the bytes exist only once they are in the caller's
    /// buffer. Giving the BODY the buffer is what lets `return a + b` mean "append, then
    /// report", and it makes the adapter a pure forward with no logic at all.
    StringOut,
    /// `-> [T]!`: the body takes the Q12 triple and returns the status, exactly like
    /// [`RetAbi::StringOut`].
    ///
    /// It has to, for the same reason: the elements have nowhere to live until they are in
    /// the caller's buffer. The element type rides along because it decides the pointer type
    /// and the value the zero fill writes (SPEC-array-return 2.5).
    ArrayOut(IrArrayElem),
}

impl RetAbi {
    /// How a propagating failure leaves a body with this shape.
    fn propagate(self) -> &'static str {
        match self {
            RetAbi::Fallible => "return Err(__e)",
            // A string body already returns the status directly, so a propagated code needs
            // no wrapping.
            //
            // This said "not reachable from source today", and that was wrong when it was
            // written: `export fn label(n: i32) -> string! { let c = try code(n)  … }`
            // compiles and emits `Err(__e) => return __e` from right here (measured; pinned by
            // `compiler/tests/fallible_calls.rs`). An arm believed unreachable is an arm nobody
            // tests, which is how a wrong one would have survived — this one happens to be
            // right.
            RetAbi::StringOut | RetAbi::ArrayOut(_) => "return __e",
            // Unreachable from source: typeck requires a `try` caller to be fallible. Kept
            // total so hand-built IR cannot fall through to something worse.
            RetAbi::Plain => "return __e",
        }
    }
}

fn emit_stmt(s: &IrStmt, indent: usize, abi: RetAbi, out: &mut String) {
    let pad = "    ".repeat(indent);
    match s {
        // AR1 landed the surface; AR3 lands the lowering. `emit` refuses a module carrying
        // these before it reaches here (`refuse_unlowered_array_return`), so this arm is an
        // invariant rather than a gap -- and it panics loudly rather than emitting nothing,
        // because silently dropping a statement is how a module compiles and answers wrong.
        // `result <n>` -- SPEC-array-return 2.4. Everything Q12 promises rides on the ORDER
        // of these four steps:
        //
        //   1. clamp a negative length to 0. 5.2.3: a negative n is an empty result, not an
        //      enormous one. The same clamp is applied to `ml_cap` (2.7), for the reason MSVC
        //      documents on `_snprintf(count < 0)`: reading it as an unsigned giant overruns.
        //   2. write `*ml_needed`. The truncation row of Q12's table says the host learns the
        //      size it needs, so this happens BEFORE the capacity test, not after it.
        //   3. bail if it does not fit. Nothing has been written to `ml_buf` yet, which is how
        //      "a truncated call writes nothing" survives a value that has to be computed.
        //   4. only now, zero the elements. 2.5: the author's loop may skip indices, and an
        //      unwritten element would otherwise be whatever the host left there -- the
        //      undefined hole this repository has been bitten by four times.
        //
        // On the probe call (`ml_cap = 0`, `ml_buf` NULL) step 3 returns first whenever n > 0,
        // and when n is 0 the fill loop runs zero times. Either way `ml_buf` is never
        // dereferenced, which is what makes the NULL of 2.7 safe rather than merely allowed.
        IrStmt::ResultLen(n) => {
            let elem = match abi {
                RetAbi::ArrayOut(e) => e,
                RetAbi::Plain | RetAbi::Fallible | RetAbi::StringOut => unreachable!(
                    "`result <n>` outside an array-returning function -- typeck refuses it"
                ),
            };
            let zero = match elem {
                IrArrayElem::F64 => "0.0",
                IrArrayElem::Bool => "false",
                IrArrayElem::I32 => "0",
            };
            let _ = writeln!(out, "{pad}let __n = {};", emit_expr(n, abi));
            let _ = writeln!(out, "{pad}let __n = if __n < 0 {{ 0 }} else {{ __n }};");
            let _ = writeln!(out, "{pad}unsafe {{ *ml_needed = __n; }}");
            let _ = writeln!(
                out,
                "{pad}let __cap = if ml_cap < 0 {{ 0 }} else {{ ml_cap }};"
            );
            let _ = writeln!(
                out,
                "{pad}if __n > __cap {{ return {}; }}",
                crate::abi::ML_ST_INSUFFICIENT_BUFFER
            );
            let _ = writeln!(out, "{pad}let mut __z = 0i32;");
            let _ = writeln!(out, "{pad}while __z < __n {{");
            let _ = writeln!(
                out,
                "{pad}    unsafe {{ *ml_buf.add(__z as usize) = {zero}; }}"
            );
            let _ = writeln!(out, "{pad}    __z += 1;");
            let _ = writeln!(out, "{pad}}}");
        }
        // `result[i] = v` -- bounds-checked against the DECLARED length, not against `ml_cap`.
        // The two are equal on this path (step 3 above returned otherwise), and checking the
        // declared one is what makes the error mean "you wrote outside your own result".
        IrStmt::ResultSet { index, value } => {
            let _ = writeln!(out, "{pad}{{");
            let _ = writeln!(out, "{pad}    let __i = {};", emit_expr(index, abi));
            let _ = writeln!(
                out,
                "{pad}    if __i < 0 || __i >= __n {{ return {}; }}",
                crate::abi::ML_ST_INDEX_OUT_OF_RANGE
            );
            let _ = writeln!(
                out,
                "{pad}    unsafe {{ *ml_buf.add(__i as usize) = {}; }}",
                emit_expr(value, abi)
            );
            let _ = writeln!(out, "{pad}}}");
        }
        IrStmt::Return(e) => match abi {
            // A fallible body hands its value back as `Ok`, so its caller — the adapter, or
            // another Mathless function through `try` — can tell success from a propagated
            // status without a sentinel value.
            RetAbi::Fallible => {
                let _ = writeln!(out, "{pad}return Ok({});", emit_expr(e, abi));
            }
            RetAbi::Plain => {
                let _ = writeln!(out, "{pad}return {};", emit_expr(e, abi));
            }
            // An array-returning function has no `return <value>`: the value IS the host's
            // buffer, which `result[i] = v` fills. Typeck refuses the statement, so reaching
            // here means the IR was built by hand with a shape the surface cannot express.
            RetAbi::ArrayOut(_) => unreachable!(
                "`return <expr>` in an array-returning function -- typeck refuses it, because                  the value is the caller's buffer"
            ),
            // A string return IS the write into the caller's buffer.
            RetAbi::StringOut => match &e.kind {
                // Built here: pieces appended in order (SPEC-string-concat §2.3).
                IrExprKind::Concat(pieces) => emit_concat_return(pieces, indent, out),
                // A lone span goes through the SAME emitter as a concatenation, as a
                // one-piece list. It could have had its own path, but then the length-bounded
                // copy would exist in two places and only one of them would be covered by the
                // test that uses `+`. The comment below predicted this arm's hazard exactly —
                // "a new string-shaped one would be handed to `ml_strout` as if it were an
                // address" — and a span has no NUL at `to`, so that would return the suffix.
                IrExprKind::ByteSlice { .. } => {
                    emit_concat_return(core::slice::from_ref(e), indent, out)
                }
                // And so does a lone `fixed(x, n)`, for the same reason and one more: it is
                // not a pointer at all. `ml_strout` would take the f64's bits as an address.
                IrExprKind::Fixed { .. } => {
                    emit_concat_return(core::slice::from_ref(e), indent, out)
                }
                // Borrowed: one pointer, the #92 path, unchanged. Spelled out rather than
                // `_`, because "everything else is already a pointer" is a fact about
                // today's variants, not about the enum — a new string-shaped one would be
                // handed to `ml_strout` as if it were an address.
                IrExprKind::ConstStr(_)
                | IrExprKind::Var(_)
                | IrExprKind::Call { .. }
                | IrExprKind::Cast { .. }
                | IrExprKind::ConstF64(_)
                | IrExprKind::ConstI32(_)
                | IrExprKind::ConstBool(_)
                | IrExprKind::Unary { .. }
                | IrExprKind::Binary { .. }
                | IrExprKind::Index { .. }
                | IrExprKind::Len { .. }
                // `byte_len` yields an `i32`, so typeck never lets it be a string return —
                // but it is named here rather than lumped under a catch-all for the reason
                // this arm exists: the fallback treats its operand as an address.
                | IrExprKind::ByteLen(_) => {
                    let _ = writeln!(
                        out,
                        "{pad}return ml_strout({}, ml_buf, ml_cap, ml_needed);",
                        emit_expr(e, abi)
                    );
                }
            },
        },
        IrStmt::Fail(code) => {
            // The domain code, wrapped so it survives the hop to whoever unwraps it. Emitting
            // a bare `return <code>` here is exactly the silent wrong answer #67 measured: for
            // `-> i32!` the code came back as an ordinary value.
            if abi == RetAbi::Fallible {
                let _ = writeln!(out, "{pad}return Err({code});");
            } else if abi == RetAbi::StringOut {
                // The body already returns the status directly, so the domain code IS the
                // return value. `ml_needed` is deliberately left alone: D17 says a failed call
                // writes no out-param, and `needed` is one.
                let _ = writeln!(out, "{pad}return {code};");
            } else {
                // Unreachable from source — `fail` requires a fallible function.
                let _ = writeln!(out, "{pad}return {code};");
            }
        }
        // `<dest> = try <callee>(<args>)`.
        //
        // Lowered as a `match`, never with Rust's `?`: `?` only works inside a function
        // returning `Result`, and two of the three C-facing shapes return a bare `i32`, so it
        // would need a nested inner fn per export. One `match` fits all four ABIs.
        //
        // `__v` and `__e` are ARM-LOCAL, so no two statements' temporaries can collide — the
        // `__d` shadowing defect (#85) came from a temporary that outlived its arm. `__` is
        // reserved from user identifiers anyway, which is the second line of defence.
        IrStmt::TryCall {
            dest, callee, args, ..
        } => {
            let call = format!(
                "ml_fn_{callee}({})",
                args.iter()
                    .map(|a| emit_expr(a, abi))
                    .collect::<Vec<_>>()
                    .join(", ")
            );
            let prop = abi.propagate();
            match dest {
                IrTryDest::Let { name, mutable } => {
                    let m = if *mutable { "mut " } else { "" };
                    let _ = writeln!(
                        out,
                        "{pad}let {m}{name} = match {call} {{ Ok(__v) => __v, Err(__e) => {prop} }};"
                    );
                }
                IrTryDest::Assign(name) => {
                    let _ = writeln!(
                        out,
                        "{pad}{name} = match {call} {{ Ok(__v) => __v, Err(__e) => {prop} }};"
                    );
                }
                // A write THROUGH a pointer, exactly like `IrStmt::AssignOut`. Emitting a
                // plain `name = …` here would assign the pointer itself.
                IrTryDest::AssignOut(name) => {
                    let _ = writeln!(
                        out,
                        "{pad}unsafe {{ *{name} = match {call} {{ Ok(__v) => __v, Err(__e) => {prop} }}; }}"
                    );
                }
                // `return try f(x)`: the success arm has to do whatever this function's own
                // `return` does, so it is delegated rather than duplicated — emitted INSIDE
                // the arm so `__v` stays arm-local like the other three destinations. Binding
                // it outside would work today (this statement always ends its block) but it
                // is the shape the `__d` defect came from, and there is no reason to keep it.
                IrTryDest::Return => {
                    let _ = writeln!(out, "{pad}match {call} {{");
                    let _ = writeln!(out, "{pad}    Ok(__v) => {{");
                    emit_stmt(
                        &IrStmt::Return(IrExpr {
                            // The type is not read by any `Return` arm — the emitted
                            // expression is the binding `__v`, whose type rustc infers from
                            // the callee's `Result`.
                            ty: IrType::I32,
                            kind: IrExprKind::Var("__v".to_string()),
                        }),
                        indent + 2,
                        abi,
                        out,
                    );
                    let _ = writeln!(out, "{pad}    }}");
                    let _ = writeln!(out, "{pad}    Err(__e) => {{ {prop}; }}");
                    let _ = writeln!(out, "{pad}}}");
                }
            }
        }
        IrStmt::Let {
            name,
            value,
            mutable,
        } => {
            // Internal binding — a plain Rust `let`, never an export.
            let kw = if *mutable { "let mut" } else { "let" };
            let _ = writeln!(out, "{pad}{kw} {name} = {};", emit_expr(value, abi));
        }
        IrStmt::AssignOut { name, value } => {
            // Same shape as the D17 success write: a raw store through the caller's pointer.
            // The host contract is that the pointer is valid for the duration of the call
            // (D16); a NULL is undefined behaviour here exactly as it already is for
            // `out_value` (SPEC-out-params section 5.1 — inherited, not introduced).
            let _ = writeln!(
                out,
                "{pad}unsafe {{ *{name} = {}; }}",
                emit_expr(value, abi)
            );
        }
        IrStmt::Assign { name, value } => {
            // Reassign an in-scope `let mut`. Inside an `if` this mutates the OUTER binding,
            // which is the point of the slice: `if` is a statement, so a mutable local is how
            // a branch result is collected.
            let _ = writeln!(out, "{pad}{name} = {};", emit_expr(value, abi));
        }
        IrStmt::If { cond, body } => {
            let _ = writeln!(out, "{pad}if {} {{", emit_expr(cond, abi));
            for st in body {
                emit_stmt(st, indent + 1, abi, out);
            }
            let _ = writeln!(out, "{pad}}}");
        }
        IrStmt::While { cond, body } => {
            // A module export may now fail to return (SPEC-while §5.1). There is nothing to
            // emit for that: no fuel counter without the VM R01 rejected, no timeout without a
            // runtime. The contract is documented in HOST_ABI instead.
            let _ = writeln!(out, "{pad}while {} {{", emit_expr(cond, abi));
            for st in body {
                emit_stmt(st, indent + 1, abi, out);
            }
            let _ = writeln!(out, "{pad}}}");
        }
    }
}

fn emit_expr(e: &IrExpr, abi: RetAbi) -> String {
    match &e.kind {
        // `xs[i]`, bounds-checked (SPEC-array-input 2.3).
        //
        // A Rust BLOCK EXPRESSION, because the check has to be able to leave the function and
        // an expression cannot otherwise: `{ let i = …; if out of range { return …; } read }`
        // yields a value and can early-return from the enclosing `fn`. That is why no hoisting
        // pass was needed, and why this arm has to know the return ABI — a fallible body hands
        // back `Result`, a `-> string!` body hands back the status directly.
        //
        // Typeck has already proved the enclosing function is fallible, so `Plain` is
        // unreachable from source. It is spelled out rather than `_` so that a new ABI has to
        // decide what an out-of-range index means instead of inheriting an answer.
        IrExprKind::Index { array, index } => {
            let bail = match abi {
                RetAbi::Fallible => {
                    format!("return Err({});", crate::abi::ML_ST_INDEX_OUT_OF_RANGE)
                }
                RetAbi::StringOut | RetAbi::ArrayOut(_) => {
                    format!("return {};", crate::abi::ML_ST_INDEX_OUT_OF_RANGE)
                }
                RetAbi::Plain => unreachable!(
                    "indexing requires `-> T!` — typeck rejects it in an infallible function"
                ),
            };
            format!(
                "{{ let __i = {}; if __i < 0 || __i >= {array}_len {{ {bail} }} unsafe {{ *{array}.add(__i as usize) }} }}",
                emit_expr(index, abi)
            )
        }
        // The companion, read straight out. Cannot fail, so no block and no early return.
        IrExprKind::Len { array } => format!("{array}_len"),
        // `byte_len(s)` — the same NUL walk `==` and concatenation already do, exposed
        // (SPEC-string-length DP-L1). The terminator is not counted: `""` is 0, not 1, which
        // is deliberately a different number from Q12's `ml_needed` (§2.1).
        IrExprKind::ByteLen(operand) => format!("ml_slen({})", emit_expr(operand, abi)),
        // A concatenation is not an expression in the emitted Rust: it has no value until it
        // is written into the caller's buffer, which is what `emit_concat_return` does. Typeck
        // confines it to `return` (DP-K3) precisely so this arm is unreachable.
        IrExprKind::Concat(_) => unreachable!(
            "a built string is only lowered at `return` — typeck rejects every other position"
        ),
        // Same reason, same rule: a span has no value until it is copied into the caller's
        // buffer. `is_built_string` answers `true` for it, so typeck confines it to `return`
        // and this arm is unreachable from source.
        IrExprKind::ByteSlice { .. } => {
            unreachable!("a span is only lowered at `return` — typeck rejects every other position")
        }
        // Third of the same family. `is_built_string` answers `true` for `fixed`, so typeck
        // confines it to `return` and nothing here can produce it as a value: its bytes exist
        // only once there is a buffer to put them in.
        IrExprKind::Fixed { .. } => unreachable!(
            "`fixed` is only lowered at `return` — typeck rejects every other position"
        ),
        // A static, NUL-terminated byte array: no allocation, and the NUL makes the module's
        // view of the bytes identical to the C caller's (SPEC-string-input DP-S1).
        // A byte string, with everything outside printable ASCII written as `\xNN`.
        //
        // Not cosmetic: rustc REFUSES a non-ASCII character inside `b"…"`, so before this a
        // Korean literal could not be lowered at all (`error: non-ASCII character in byte
        // string literal`). Escaping carries the same bytes and keeps the emitted Rust itself
        // ASCII — which is what DP-S4's reason was actually protecting, and why the artifacts
        // stay ASCII even now that the source need not be (`SPEC-non-ascii-literals` §2.3).
        //
        // A no-op for every literal in this repository today: the goldens did not move when
        // this landed. The lexer still refuses `"` and escapes inside a literal, so those two
        // arms are unreachable — spelled out anyway, because the day the lexer changes this
        // function should already be right.
        IrExprKind::ConstStr(s) => {
            let mut lit = String::new();
            for b in s.as_bytes() {
                match b {
                    b'"' => lit.push_str("\\\""),
                    b'\\' => lit.push_str("\\\\"),
                    0x20..=0x7e => lit.push(*b as char),
                    other => lit.push_str(&format!("\\x{other:02X}")),
                }
            }
            format!("b\"{lit}\\0\".as_ptr()")
        }
        IrExprKind::ConstF64(n) => format!("{n:?}f64"),
        IrExprKind::ConstI32(n) => format!("{n}i32"),
        IrExprKind::ConstBool(b) => b.to_string(),
        IrExprKind::Var(name) => name.clone(),
        IrExprKind::Call { name, args } => {
            let args: Vec<String> = args.iter().map(|a| emit_expr(a, abi)).collect();
            // A built-in rounder lowers to its `ml_`-prefixed helper (emitted above). That is
            // safe because `reserved::generated_prefix` rejects `ml_` on PARAMETERS and
            // LOCALS, which are still emitted raw — when this comment once claimed the prefix
            // was "already reserved" it was only true of function names, and
            // `export fn f(ml_floor: f64) -> f64 { return floor(ml_floor) }` emitted
            // `ml_floor(ml_floor)` and died in rustc (#85).
            //
            // Function names no longer need the rule: since the wrapper refactor a user
            // function is `ml_fn_<name>`, which cannot collide with `ml_floor` (SPEC-export-wrappers DP-W4).
            if crate::typeck::Rounder::from_name(name).is_some() {
                format!("ml_{name}({})", args.join(", "))
            } else {
                // Every Mathless function is one body named `ml_fn_<name>`, exported or not,
                // so a call site no longer has to know which it is. Before the wrapper
                // refactor this branch chose between `mlx_<name>` and the bare name, and
                // choosing wrong meant the generated crate did not build (#95).
                format!("ml_fn_{name}({})", args.join(", "))
            }
        }
        IrExprKind::Cast { to, operand } => {
            // Backend net for directly-built IR, and this one guards a *silent* failure:
            // Rust accepts `bool as i32` and yields 0/1, so hand-built IR would compile and
            // quietly mean something Mathless forbids. (`bool as f64` is a rustc error, so
            // that half is loud on its own.) Source cannot reach here — typeck rejects it.
            debug_assert!(
                operand.ty != IrType::Bool && *to != IrType::Bool,
                "IrExprKind::Cast involving bool ({:?} as {to:?}): `as` is numeric-only",
                operand.ty
            );
            // Rust's `as` truncates toward zero, saturates out-of-range, and maps NaN to 0 —
            // exactly the semantics the SPEC pins. That agreement is why this lowering is one
            // line; it is NOT the reason the semantics were chosen, and a C backend must
            // implement them by hand (C casts are UB out of range).
            format!("({} as {})", emit_expr(operand, abi), rust_type(*to))
        }
        IrExprKind::Unary { op, operand } => {
            // Backend safety net for directly-built IR, in the same spirit as
            // `block_always_returns`: Rust's `!` is a *bitwise* complement on integers, so
            // emitting `Not` on a non-bool would silently mean something else. Source can't
            // get here — typeck rejects it — but the IR is a public type (Grok verify).
            debug_assert!(
                !matches!(op, IrUnOp::Not) || operand.ty == IrType::Bool,
                "IrUnOp::Not on {:?}: Rust's `!` is bitwise on integers",
                operand.ty
            );
            // Parenthesised like the binary case, so precedence never depends on the target
            // language's table. Rust's unary binds tighter than `*` anyway; this makes it
            // explicit and survives a future C backend unchanged.
            // i32 negation wraps — `ir.rs` says so on `IrUnOp::Neg` itself ("`-i32::MIN ==
            // i32::MIN`"), and Rust's plain `-` only wraps while `overflow-checks` is off.
            // Same reasoning as the `wrapping_*` arms below: put the rule in the code.
            if matches!(op, IrUnOp::Neg) && operand.ty == IrType::I32 {
                return format!("({}).wrapping_neg()", emit_expr(operand, abi));
            }
            let sym = match op {
                IrUnOp::Neg => "-",
                IrUnOp::Not => "!",
            };
            format!("({sym}{})", emit_expr(operand, abi))
        }
        IrExprKind::Binary { op, lhs, rhs } => {
            // Directly-built IR could put `&&`/`||` on non-bool operands. Note this is a
            // *weaker* hazard than `IrUnOp::Not` on an integer: Rust's `&&` is bool-only, so
            // that would fail when the generated crate compiles rather than silently meaning
            // something else. The assert just moves the failure somewhere legible.
            debug_assert!(
                !matches!(op, IrBinOp::And | IrBinOp::Or)
                    || (lhs.ty == IrType::Bool && rhs.ty == IrType::Bool),
                "IrBinOp::{op:?} on {:?} and {:?}: logical operators are bool-only",
                lhs.ty,
                rhs.ty
            );
            // i32 `/` and `%` are TOTAL in Mathless (SPEC-i32-division DP-D1), and Rust's are
            // not: the plain operator panics on `b == 0` and, separately, on `i32::MIN / -1`.
            // The second one is easy to miss — division overflow panics in a release build too,
            // because it is NOT governed by `overflow-checks` (measured with
            // `rustc -O -C overflow-checks=off`). Either panic would enter `ml_panic` and spin
            // there, hanging the host thread (STATUS §5-4), so both edges close here:
            // `wrapping_*` handles MIN/-1 and the guard handles the zero.
            //
            // The divisor is bound first so it is evaluated exactly once — `a / f(b)` must not
            // call `f` twice just because the emitted form mentions the divisor in two places.
            if matches!(op, IrBinOp::Div | IrBinOp::Rem) && lhs.ty == IrType::I32 {
                let method = if matches!(op, IrBinOp::Div) {
                    "wrapping_div"
                } else {
                    "wrapping_rem"
                };
                return format!(
                    "{{ let __d = {}; if __d == 0 {{ 0i32 }} else {{ ({}).{}(__d) }} }}",
                    emit_expr(rhs, abi),
                    emit_expr(lhs, abi),
                    method
                );
            }
            // `+ - *` on i32 wrap (DP-I4, stated on `IrBinOp` in ir.rs). Rust's plain
            // operators wrap only while `overflow-checks` is off, and the generated profile
            // left that to cargo's default — so the module's arithmetic depended on the
            // environment it was built in, not on anything in the source.
            //
            // Measured, one variable apart, with a C host calling the built `.dll`:
            //
            //     mlc build                                        bump(2147483647) = -2147483648
            //     CARGO_PROFILE_RELEASE_OVERFLOW_CHECKS=true …     hung; killed at 8s
            //
            // The hang is `ml_panic`'s `loop {}` (STATUS §5-4), not a crash the host can see.
            // Pinning the flag in the profile is not enough on its own — cargo's env vars
            // override a manifest profile — so the rule goes where nothing can override it:
            // the emitted expression. Same move the `/` and `%` guard above already makes.
            if lhs.ty == IrType::I32 {
                // Enumerated, not `_ => None`: a new i32 arithmetic operator falling through
                // here would silently get the plain Rust operator back, which is precisely
                // the defect this arm exists to fix.
                if let Some(method) = match op {
                    IrBinOp::Add => Some("wrapping_add"),
                    IrBinOp::Sub => Some("wrapping_sub"),
                    IrBinOp::Mul => Some("wrapping_mul"),
                    // Guarded above with an explicit zero check and `wrapping_div`/`_rem`.
                    IrBinOp::Div | IrBinOp::Rem => None,
                    // Comparisons and the logical operators yield bool; nothing to wrap.
                    IrBinOp::Lt
                    | IrBinOp::Gt
                    | IrBinOp::Le
                    | IrBinOp::Ge
                    | IrBinOp::Eq
                    | IrBinOp::Ne
                    | IrBinOp::And
                    | IrBinOp::Or => None,
                } {
                    return format!(
                        "({}).{method}({})",
                        emit_expr(lhs, abi),
                        emit_expr(rhs, abi)
                    );
                }
            }
            // A string is a `*const u8` (SPEC-string-input DP-S1). Rust's `==` on raw pointers compares the
            // ADDRESSES, which would make `country == "KR"` false for a host string that
            // happens to hold exactly those bytes — the wrong answer, silently, with no
            // compile error. Route both directions through the byte-loop helper instead.
            if matches!(op, IrBinOp::Eq | IrBinOp::Ne) && lhs.ty == IrType::Str {
                // A SPAN cannot go through `ml_streq`: it has no NUL at `to`, so the helper
                // would walk on into the rest of the source and compare the WHOLE remainder
                // against the other side — answering `false` for `"ABZZZZ"` vs `"AB"` where
                // the span `(0, 2)` is exactly `"AB"`. The oppposite of the truth, silently
                // (`SPEC-string-slice-compare` §2.3).
                //
                // So the whole comparison is lowered here, and `IrExprKind::ByteSlice` stays
                // `unreachable!` as a standalone expression — if it ever produced a bare
                // `s.add(from)`, every NUL-walker in the emitted crate would read past `to`.
                // That is the hazard the pre-verification named, and this is where it is
                // closed (§2.5).
                let span = match (&lhs.kind, &rhs.kind) {
                    (IrExprKind::ByteSlice { s, from, to }, _) => Some((s, from, to, rhs)),
                    (_, IrExprKind::ByteSlice { s, from, to }) => Some((s, from, to, lhs)),
                    _ => None,
                };
                let call = match span {
                    Some((s, from, to, other)) => {
                        // The bail is chosen from the RETURN ABI, exactly as the `Index` arm
                        // above does. Writing `return -2` here would be the `StringOut` form,
                        // and this slice's whole motivation is a `-> bool!` predicate, whose
                        // body returns `Result<bool, i32>`. The generated crate would not
                        // compile — caught in the SPEC's own snippet by review, before it was
                        // written down as code.
                        let bail = match abi {
                            RetAbi::Fallible => {
                                format!("return Err({});", crate::abi::ML_ST_INDEX_OUT_OF_RANGE)
                            }
                            RetAbi::StringOut | RetAbi::ArrayOut(_) => {
                                format!("return {};", crate::abi::ML_ST_INDEX_OUT_OF_RANGE)
                            }
                            RetAbi::Plain => unreachable!(
                                "comparing a span requires `-> T!` — typeck rejects it in an \
                                 infallible function"
                            ),
                        };
                        // `s` is BOUND, not emitted twice. Today it can only be a parameter
                        // or a literal, so the cost would be nothing — but the concat
                        // emitter measured the other version of this exact mistake
                        // (`ml_fn_score(…)` called once per pass) and the note there is that
                        // two emissions are also the mechanism by which two reads could
                        // disagree. One binding, two uses.
                        format!(
                            "{{ let __ss = {}; let __sf = {}; \
                             let __sl = ml_sublen(__ss, __sf, {}); \
                             if __sl < 0 {{ {bail} }} ml_subeq(__ss, __sf, __sl, {}) }}",
                            emit_expr(s, abi),
                            emit_expr(from, abi),
                            emit_expr(to, abi),
                            emit_expr(other, abi)
                        )
                    }
                    None => format!("ml_streq({}, {})", emit_expr(lhs, abi), emit_expr(rhs, abi)),
                };
                return if matches!(op, IrBinOp::Eq) {
                    call
                } else {
                    format!("(!{call})")
                };
            }
            format!(
                "({} {} {})",
                emit_expr(lhs, abi),
                op_str(*op),
                emit_expr(rhs, abi)
            )
        }
    }
}

/// Copy a NUL-terminated string into the host's buffer, per the Q12 protocol
/// (SPEC-string-return section 2).
///
/// Every hazard this slice has lives in these fifteen lines, so each rule is written down
/// next to the code that keeps it:
///
/// - **Two passes.** The length is counted BEFORE anything is copied, because Q12 says a
///   truncated call leaves `buf` untouched — and with no allocator there is nowhere to stage
///   a partial result. DP-T2 confirmed that table unchanged, so this is load-bearing, not a
///   convenience.
/// - **One unit everywhere.** `cap` and `*needed` are total bytes INCLUDING the NUL, on every
///   path (DP-T4). That makes the host's retry exactly `cap = *needed`, and it makes
///   `*needed == 0` impossible — so 0 can never be confused with an empty string, which is 1.
/// - **`cap < n` covers `cap == 0` and `cap < 0`.** `n` is at least 1 (an empty string still
///   needs its NUL), so a zero or negative capacity is truncation by the same comparison as
///   any other. Nothing is reinterpreted as unsigned — MSVC documents exactly that overrun
///   for `_snprintf` with a negative count.
/// - **`buf` is only dereferenced after that check**, which is what makes the `cap == 0`
///   probe safe with a NULL buffer (DP-T7). The probe is the documented way to learn the
///   length, so it must be safe, not merely tolerated.
/// - **No slicing, no indexing, no bounds check.** Raw `ptr::add` only. A panic in a
///   generated module enters `ml_panic`'s `loop {}` and hangs the calling host thread rather
///   than crashing (STATUS section 5-4), so the goal is code that cannot panic at all.
fn emit_strout_helper(module: &IrModule, out: &mut String) {
    if !module
        .functions
        .iter()
        .any(|f| f.exported && f.ret == IrType::Str)
    {
        return;
    }
    out.push_str(
        "fn ml_strout(src: *const u8, buf: *mut u8, cap: i32, needed: *mut i32) -> i32 {\n\
         \x20   // Pass 1: count to the NUL, inclusive. `n` ends at >= 1 for every string.\n\
         \x20   let mut n: i32 = 0;\n\
         \x20   loop {\n\
         \x20       let b = unsafe { *src.add(n as usize) };\n\
         \x20       n += 1;\n\
         \x20       if b == 0 { break; }\n\
         \x20   }\n\
         \x20   unsafe { *needed = n; }\n\
         \x20   // Truncation is a FAILURE, not a short success (Q12). Nothing has been\n\
         \x20   // written, and nothing will be.\n\
         \x20   if cap < n { return -1; }\n\
         \x20   // Pass 2: copy, terminator included.\n\
         \x20   let mut i: i32 = 0;\n\
         \x20   while i < n {\n\
         \x20       unsafe { *buf.add(i as usize) = *src.add(i as usize); }\n\
         \x20       i += 1;\n\
         \x20   }\n\
         \x20   0\n\
         }\n\n",
    );
}

/// Emit `return <pieces appended into the caller's buffer>` (SPEC-string-concat §2.3).
///
/// Two passes, and the order is the contract:
///
/// 1. Sum every piece's length, plus one for the NUL, into `__n`, and publish it through
///    `*ml_needed` — the host's `cap = *needed` retry depends on it being exact even when the
///    call fails.
/// 2. `cap < __n` returns `-1` having written **nothing**. Q12 says a truncated call leaves
///    the buffer untouched, DP-T2 confirmed that table unchanged, and with no allocator there
///    is nowhere to stage a partial result anyway.
/// 3. Only then append, in source order, and terminate.
///
/// `ml_ilen` and `ml_wint` must agree to the byte or pass 2 walks off the end of the host's
/// buffer. They agree by construction: `ml_wint` asks `ml_ilen` for the width and fills that
/// exact span right-to-left (SPEC §5.2).
/// How one concat piece reaches the host's buffer.
///
/// The three passes of [`emit_concat_return`] each asked this question separately, with their
/// own `IrExprKind::Cast { .. } => … , _ => …`. Three copies of one rule, each able to drift —
/// and the two counting passes disagreeing is a write past the end of the host's buffer, which
/// is why the same function already refuses to let `ml_wint` recount.
///
/// Asked once, here, exhaustively: a new `IrExprKind` does not build until this says which
/// side it falls on. That is the shape the concat slice's own history argues for — commit
/// 9240ee7 updated three exhaustive walkers in this file correctly and missed the one that
/// ended in `_`, in the same commit (#158).
enum PieceKind<'a> {
    /// Decimal digits the module renders: `<i32> as string`. Counted by `ml_ilen`, written by
    /// `ml_wint`, and both are handed the CAST'S OPERAND, not the cast.
    Digits { operand: &'a IrExpr },
    /// Bytes the module borrows — from a literal or from the host. Counted by `ml_slen`,
    /// copied by `ml_wstr`.
    Bytes,
    /// A half-open span of borrowed bytes: `byte_slice(s, from, to)`.
    ///
    /// Its own kind because it is the one piece that is **not** NUL-bounded. `ml_slen` and
    /// `ml_wstr` both stop at the source's NUL, so counting or copying a span with them would
    /// take the whole suffix and ignore `to` — status 0, no warning (`SPEC-string-slice` §2.5).
    Span {
        s: &'a IrExpr,
        from: &'a IrExpr,
        to: &'a IrExpr,
    },
    /// Decimal digits the module COMPUTES: `fixed(x, places)`.
    ///
    /// Not `Digits`, which is `<i32> as string` and reaches `ml_ilen`/`ml_wint` — those take
    /// an `i32` and `fixed`'s value is an f64 that has not been scaled yet. Not `Bytes`
    /// either: there is no pointer here at all, so the fallback would hand `ml_slen` an f64's
    /// bit pattern as an address. Like `Span`, it binds more than one value and is sized by
    /// its own helper (`SPEC-fixed-decimals` §2.4).
    Decimal { x: &'a IrExpr, places: &'a IrExpr },
}

fn piece_kind(p: &IrExpr) -> PieceKind<'_> {
    match &p.kind {
        IrExprKind::Cast { operand, .. } => PieceKind::Digits { operand },
        IrExprKind::ByteSlice { s, from, to } => PieceKind::Span { s, from, to },
        IrExprKind::Fixed { x, places } => PieceKind::Decimal { x, places },
        // Array elements are scalars, so neither of these is ever `Str` and neither can reach
        // a concatenation. Named rather than lumped in below, because "already a pointer" is
        // false for both — treating one as an address is the bug this match exists to prevent.
        IrExprKind::Index { .. } | IrExprKind::Len { .. } => {
            unreachable!("an array element is a scalar, so it is never a piece of a built string")
        }
        // `byte_len` is an `i32`, so it reaches a concatenation only through a cast, and the
        // `Cast` arm above takes that path. Bare, it would be an integer treated as a
        // pointer — the exact confusion the two arms above exist to refuse.
        IrExprKind::ByteLen(_) => {
            unreachable!("byte_len is an i32, so it is never a piece of a built string")
        }
        // Every piece is `Str` by the time codegen sees it (typeck flattens and checks), so
        // each of these is already a pointer. Spelled out so that a future string-shaped
        // variant has to say which it is instead of being assumed to be an address.
        IrExprKind::ConstStr(_)
        | IrExprKind::Var(_)
        | IrExprKind::Call { .. }
        | IrExprKind::Concat(_)
        | IrExprKind::ConstF64(_)
        | IrExprKind::ConstI32(_)
        | IrExprKind::ConstBool(_)
        | IrExprKind::Unary { .. }
        | IrExprKind::Binary { .. } => PieceKind::Bytes,
    }
}

fn emit_concat_return(pieces: &[IrExpr], indent: usize, out: &mut String) {
    let pad = "    ".repeat(indent);
    // Each piece is bound ONCE, before either pass, and both passes use the binding.
    //
    // It was emitted twice — inlined into the `ml_slen`/`ml_ilen` line and again into the
    // `ml_wstr`/`ml_wint` line. Measured on
    //   `export fn label(…) -> string! { return "eq=" + score(a == b) as string }`
    // the generated Rust called `ml_fn_score(…)` in both passes, so a helper ran twice per
    // invocation. Today's language has no side effects, so that was cost and not a wrong
    // answer — but it is also the mechanism by which the two passes could disagree, and pass 1
    // is what sized the host's buffer. `ml_wint` already refuses to recount for exactly that
    // reason (it asks `ml_ilen`); this gives the string pieces the same guarantee.
    for (i, p) in pieces.iter().enumerate() {
        let e = match piece_kind(p) {
            // A concat only ever exists in a `-> string!` body, which is StringOut by
            // construction (SPEC-string-concat 2.3).
            PieceKind::Digits { operand } => emit_expr(operand, RetAbi::StringOut),
            PieceKind::Bytes => emit_expr(p, RetAbi::StringOut),
            // A span binds three values, not one, and is emitted separately below so the
            // offsets are evaluated exactly once — the same guarantee the note above earns
            // for the other kinds. `fixed` is the same: two values, and a scaling step whose
            // result both passes must share.
            PieceKind::Span { .. } | PieceKind::Decimal { .. } => continue,
        };
        let _ = writeln!(out, "{pad}let __p{i} = {e};");
    }
    // Spans: bind, then VALIDATE — before `*ml_needed` is written and before anything is
    // copied. `SPEC-string-slice` §2.4: an out-of-range span is a failure, not a clamp, and
    // D17 says a failed call leaves `*ml_needed` alone, so the check cannot come later.
    for (i, p) in pieces.iter().enumerate() {
        if let PieceKind::Span { s, from, to } = piece_kind(p) {
            let _ = writeln!(
                out,
                "{pad}let __s{i} = {};",
                emit_expr(s, RetAbi::StringOut)
            );
            let _ = writeln!(
                out,
                "{pad}let __f{i} = {};",
                emit_expr(from, RetAbi::StringOut)
            );
            let _ = writeln!(
                out,
                "{pad}let __t{i} = {};",
                emit_expr(to, RetAbi::StringOut)
            );
            let _ = writeln!(out, "{pad}let __l{i} = ml_sublen(__s{i}, __f{i}, __t{i});");
            let _ = writeln!(
                out,
                "{pad}if __l{i} < 0 {{ return {}; }}",
                crate::abi::ML_ST_INDEX_OUT_OF_RANGE
            );
        }
    }
    // `fixed`: scale ONCE, here, and let both passes read the scaled integer.
    //
    // The same ordering rule the spans above follow, and for the same D17 reason — the three
    // ways `fixed` can have no answer (`SPEC-fixed-decimals` §2.4: a NaN or infinite `x`, a
    // `places` outside 0..=9, a product past i64) are decided before `*ml_needed` is written
    // and before a byte is copied. And the same anti-drift rule `ml_wint` follows: the width
    // pass 1 uses is the width pass 2 fills, because `ml_wfix` asks `ml_fixlen` rather than
    // counting again.
    //
    // Note the binding order across the three loops above is BY KIND, not by source position:
    // `"a" + byte_slice(..) + fixed(..)` binds the literal, then the span, then the decimal.
    // Nothing observes that today — the language has no side effects and both out-of-range
    // kinds bail with the same status — but the day either changes, the piece that reports
    // first will stop being the leftmost one. Written down rather than restructured, because
    // the alternative is one loop that must then re-derive which kind it is looking at.
    for (i, p) in pieces.iter().enumerate() {
        if let PieceKind::Decimal { x, places } = piece_kind(p) {
            let _ = writeln!(
                out,
                "{pad}let __x{i} = {};",
                emit_expr(x, RetAbi::StringOut)
            );
            let _ = writeln!(
                out,
                "{pad}let __k{i} = {};",
                emit_expr(places, RetAbi::StringOut)
            );
            let _ = writeln!(
                out,
                "{pad}let (__v{i}, __ok{i}) = ml_fixscale(__x{i}, __k{i});"
            );
            let _ = writeln!(
                out,
                "{pad}if !__ok{i} {{ return {}; }}",
                crate::abi::ML_ST_INDEX_OUT_OF_RANGE
            );
        }
    }
    // Pass 1 — count. One byte for the NUL is always needed, so `__n` starts at 1 and an
    // empty result is 1, never 0. That keeps `*needed == 0` impossible, exactly as #92.
    let _ = writeln!(out, "{pad}let mut __n: i32 = 1;");
    for (i, p) in pieces.iter().enumerate() {
        match piece_kind(p) {
            PieceKind::Digits { .. } => {
                let _ = writeln!(out, "{pad}__n += ml_ilen(__p{i});");
            }
            PieceKind::Bytes => {
                let _ = writeln!(out, "{pad}__n += ml_slen(__p{i});");
            }
            // `to - from`, already computed and already proved in range. NOT `ml_slen`, which
            // would count to the source's NUL and size the buffer for the whole suffix.
            PieceKind::Span { .. } => {
                let _ = writeln!(out, "{pad}__n += __l{i};");
            }
            // From the scaled integer bound above — never from the f64, which would need the
            // same decisions taken twice.
            PieceKind::Decimal { .. } => {
                let _ = writeln!(out, "{pad}__n += ml_fixlen(__v{i}, __k{i});");
            }
        }
    }
    let _ = writeln!(out, "{pad}unsafe {{ *ml_needed = __n; }}");
    // This line is the capacity test AND the NULL-probe contract, and the second job is the
    // one nothing said out loud. DP-T7 lets a host pass `ml_buf = NULL` when `ml_cap == 0`;
    // `__n` is never less than 1, so this returns before any writer below dereferences that
    // pointer. Measured by removing it: the probe call does not answer wrongly, it takes the
    // host's process down with STATUS_ACCESS_VIOLATION
    // (`string_concat.rs::a_negative_capacity_is_refused_on_the_concat_path_too`).
    let _ = writeln!(out, "{pad}if ml_cap < __n {{ return -1; }}");
    // Pass 2 — append. `__o` is the running offset; every helper returns the next one.
    //
    // `ml_cap` is handed to `ml_wstr` as a hard stop. Binding the pieces above makes the two
    // passes read the same POINTER, but not necessarily the same BYTES: nothing in the C ABI
    // tells a host its output buffer may not overlap a string it passes in, and `ml_wstr`
    // copies until it finds a NUL in the source. Under that aliasing it would feed on its own
    // output and run past the end of the host's buffer. The bound turns a memory-safety
    // failure in the HOST's process into a bounded, wrong-looking string — and costs a
    // conforming host nothing, because `ml_cap >= __n` was already checked above.
    let _ = writeln!(out, "{pad}let mut __o: i32 = 0;");
    for (i, p) in pieces.iter().enumerate() {
        match piece_kind(p) {
            PieceKind::Digits { .. } => {
                let _ = writeln!(out, "{pad}__o = ml_wint(ml_buf, __o, __p{i});");
            }
            PieceKind::Bytes => {
                let _ = writeln!(out, "{pad}__o = ml_wstr(ml_buf, __o, __p{i}, ml_cap);");
            }
            // Length-bounded, not NUL-bounded. `ml_cap` is still a hard stop for the aliasing
            // case the note above describes.
            PieceKind::Span { .. } => {
                let _ = writeln!(
                    out,
                    "{pad}__o = ml_wsub(ml_buf, __o, __s{i}, __f{i}, __l{i}, ml_cap);"
                );
            }
            // No `ml_cap` bound, unlike the two above, and the reason is the difference
            // between them: those two COPY from a host pointer that may alias the output
            // buffer. These bytes come from an i64 in a register, so the only address in
            // play is the destination — and `ml_cap >= __n` was checked above.
            PieceKind::Decimal { .. } => {
                let _ = writeln!(out, "{pad}__o = ml_wfix(ml_buf, __o, __v{i}, __k{i});");
            }
        }
    }
    let _ = writeln!(out, "{pad}unsafe {{ *ml_buf.add(__o as usize) = 0; }}");
    let _ = writeln!(out, "{pad}return 0;");
}

/// Does this module build a string anywhere? Only then are the concat helpers emitted.
///
/// Exhaustive for the same reason [`compares_strings`] is — see the note there. This walker
/// had two catch-alls, and the statement one was also short: `AssignOut` and `TryCall` carry
/// ordinary expressions and were both falling to `_ => false`.
/// Does anything in this module call `byte_len(s)`?
///
/// Its own predicate rather than a flag folded into [`builds_strings`]: `byte_len` does not
/// build a string, it reads one, and saying otherwise would drag the whole concat helper set
/// (`ml_ilen`, `ml_wstr`, `ml_wint`) into a module that needs none of it.
///
/// Exhaustive, like its neighbours, and for the reason [`compares_strings`] records: a
/// catch-all here would silently drop a future variant's subtree and the generated crate
/// would fail to compile for a user who wrote nothing wrong.
fn uses_byte_len(module: &IrModule) -> bool {
    fn in_expr(e: &IrExpr) -> bool {
        match &e.kind {
            IrExprKind::ByteLen(_) => true,
            // A span does not itself call `byte_len`, but its offsets can — and typically do
            // (`byte_slice(s, 2, byte_len(s))` is the idiom DP-B2 was chosen for).
            IrExprKind::ByteSlice { s, from, to } => in_expr(s) || in_expr(from) || in_expr(to),
            // Same: `fixed(x, byte_len(s))` is a legal place count, so the count is walked.
            IrExprKind::Fixed { x, places } => in_expr(x) || in_expr(places),
            IrExprKind::Index { index, .. } => in_expr(index),
            IrExprKind::Binary { lhs, rhs, .. } => in_expr(lhs) || in_expr(rhs),
            IrExprKind::Unary { operand, .. } | IrExprKind::Cast { operand, .. } => {
                in_expr(operand)
            }
            IrExprKind::Call { args, .. } | IrExprKind::Concat(args) => args.iter().any(in_expr),
            IrExprKind::Len { .. }
            | IrExprKind::ConstF64(_)
            | IrExprKind::ConstStr(_)
            | IrExprKind::ConstI32(_)
            | IrExprKind::ConstBool(_)
            | IrExprKind::Var(_) => false,
        }
    }
    fn in_stmts(stmts: &[IrStmt]) -> bool {
        stmts.iter().any(|s| match s {
            IrStmt::If { cond, body } | IrStmt::While { cond, body } => {
                in_expr(cond) || in_stmts(body)
            }
            IrStmt::ResultLen(e) => in_expr(e),
            IrStmt::ResultSet { index, value } => in_expr(index) || in_expr(value),
            IrStmt::Return(e)
            | IrStmt::Let { value: e, .. }
            | IrStmt::Assign { value: e, .. }
            | IrStmt::AssignOut { value: e, .. } => in_expr(e),
            IrStmt::TryCall { args, .. } => args.iter().any(in_expr),
            IrStmt::Fail(_) => false,
        })
    }
    module.functions.iter().any(|f| in_stmts(&f.body))
}

fn builds_strings(module: &IrModule) -> bool {
    fn in_expr(e: &IrExpr) -> bool {
        match &e.kind {
            IrExprKind::Concat(_) => true,
            // A span IS a built string (`is_built_string` says so), and it is emitted through
            // the concat writer, so the module needs the whole helper set.
            IrExprKind::ByteSlice { .. } => true,
            // So is `fixed`, by the same test and through the same writer. A module whose
            // only output is `return fixed(x, 2)` still needs `ml_slen` — `emit_concat_return`
            // is one emitter, and the gate that feeds it cannot be narrower than it is.
            IrExprKind::Fixed { .. } => true,
            // Reads a string, never builds one — but its operand is an ordinary expression,
            // unlike `Len`'s bare array name, so the subtree is walked.
            IrExprKind::ByteLen(operand) => in_expr(operand),
            // The index is an ordinary expression and could hold anything; the array name
            // cannot. `Len` has no subtree at all.
            IrExprKind::Index { index, .. } => in_expr(index),
            IrExprKind::Len { .. } => false,
            IrExprKind::Binary { lhs, rhs, .. } => in_expr(lhs) || in_expr(rhs),
            IrExprKind::Unary { operand, .. } | IrExprKind::Cast { operand, .. } => {
                in_expr(operand)
            }
            IrExprKind::Call { args, .. } => args.iter().any(in_expr),
            IrExprKind::ConstF64(_)
            | IrExprKind::ConstStr(_)
            | IrExprKind::ConstI32(_)
            | IrExprKind::ConstBool(_)
            | IrExprKind::Var(_) => false,
        }
    }
    fn in_stmts(stmts: &[IrStmt]) -> bool {
        stmts.iter().any(|s| match s {
            IrStmt::If { cond, body } | IrStmt::While { cond, body } => {
                in_expr(cond) || in_stmts(body)
            }
            IrStmt::ResultLen(e) => in_expr(e),
            IrStmt::ResultSet { index, value } => in_expr(index) || in_expr(value),
            IrStmt::Return(e)
            | IrStmt::Let { value: e, .. }
            | IrStmt::Assign { value: e, .. }
            | IrStmt::AssignOut { value: e, .. } => in_expr(e),
            IrStmt::TryCall { args, .. } => args.iter().any(in_expr),
            IrStmt::Fail(_) => false,
        })
    }
    module.functions.iter().any(|f| in_stmts(&f.body))
}

/// `ml_slen` — bytes up to the NUL, terminator NOT counted.
///
/// Split out of [`emit_concat_helpers`] because it now has **two** callers with different
/// gates: concatenation sizes its pieces with it, and surface `byte_len(s)` IS it
/// (`SPEC-string-length` DP-L1). Leaving it inside the concat set would have meant a module
/// that only calls `byte_len` gets no helper and the GENERATED crate fails to compile —
/// `error[E0425]: cannot find function`, in code the user never wrote. That is the exact
/// failure `compares_strings`' own note records from `ml_streq`, so it was not going to be a
/// surprise twice; the pre-implementation review named it before a line was written.
fn emit_slen_helper(module: &IrModule, out: &mut String) {
    if !builds_strings(module) && !uses_byte_len(module) {
        return;
    }
    out.push_str(
        "fn ml_slen(src: *const u8) -> i32 {\n\
         \x20   let mut n: i32 = 0;\n\
         \x20   loop {\n\
         \x20       if unsafe { *src.add(n as usize) } == 0 { return n; }\n\
         \x20       n += 1;\n\
         \x20   }\n\
         }\n\n",
    );
}

/// The three append helpers. Emitted only when the module concatenates.
///
/// Every rule that keeps this safe is written next to the code that carries it, because the
/// failure mode here is writing past the host's buffer:
///
/// - **`ml_ilen` is the single source of width.** `ml_wint` calls it rather than counting
///   again, so pass 1 and pass 2 cannot disagree (SPEC §5.2).
/// - **`i32::MIN` never gets negated.** `-2147483648` has no positive counterpart, so the
///   magnitude is taken in `u32` (`wrapping_neg`), where it fits exactly.
/// - **No slicing, no indexing.** Raw `ptr::add` only: a panic in a generated module enters
///   `ml_panic`'s `loop {}` and hangs the calling host thread (STATUS §5-4), so the goal is
///   code that cannot panic.
/// - **Digits are ASCII `-` and `0`-`9` only** (DP-K9), which is the same byte sequence in
///   every encoding this project has deliberately left undecided (SPEC-string-input DP-S2).
fn emit_concat_helpers(module: &IrModule, out: &mut String) {
    if !builds_strings(module) {
        return;
    }
    out.push_str(
        "fn ml_ilen(v: i32) -> i32 {\n\
         \x20   // Width of the decimal form, sign included. `0` is one digit, not zero.\n\
         \x20   let neg = v < 0;\n\
         \x20   let mut m: u32 = if neg { (v as u32).wrapping_neg() } else { v as u32 };\n\
         \x20   let mut n: i32 = if neg { 2 } else { 1 };\n\
         \x20   while m >= 10 { m /= 10; n += 1; }\n\
         \x20   n\n\
         }\n\n\
         fn ml_wstr(buf: *mut u8, off: i32, src: *const u8, cap: i32) -> i32 {\n\
         \x20   // `cap` is a hard stop, not a length. It exists for the host that passes a\n\
         \x20   // string ALIASING its own output buffer: nothing in the C ABI forbids that,\n\
         \x20   // and without the bound this loop would copy its own output forward and run\n\
         \x20   // off the end of the caller's memory. One byte is left for the NUL.\n\
         \x20   //\n\
         \x20   // It is REACHED on a conforming call, and returns the same offset the source\n\
         \x20   // NUL would have: when the result exactly fills the buffer, `off + i` hits\n\
         \x20   // `cap - 1` on the same iteration the NUL is read. So it never truncates a\n\
         \x20   // legitimate result — it just gets there first (Grok verify; the exact-fill\n\
         \x20   // retry is measured through a loaded module in the oracle's string_concat).\n\
         \x20   let mut i: i32 = 0;\n\
         \x20   loop {\n\
         \x20       if off + i >= cap - 1 { return off + i; }\n\
         \x20       let b = unsafe { *src.add(i as usize) };\n\
         \x20       if b == 0 { return off + i; }\n\
         \x20       unsafe { *buf.add((off + i) as usize) = b; }\n\
         \x20       i += 1;\n\
         \x20   }\n\
         }\n\n\
         fn ml_wint(buf: *mut u8, off: i32, v: i32) -> i32 {\n\
         \x20   // Width comes from ml_ilen, the same function pass 1 used, then the span is\n\
         \x20   // filled right-to-left. Two counts that could drift would be a buffer overrun.\n\
         \x20   let w = ml_ilen(v);\n\
         \x20   let neg = v < 0;\n\
         \x20   let mut m: u32 = if neg { (v as u32).wrapping_neg() } else { v as u32 };\n\
         \x20   let mut i = off + w;\n\
         \x20   loop {\n\
         \x20       i -= 1;\n\
         \x20       unsafe { *buf.add(i as usize) = b'0' + (m % 10) as u8; }\n\
         \x20       m /= 10;\n\
         \x20       if m == 0 { break; }\n\
         \x20   }\n\
         \x20   if neg { unsafe { *buf.add((i - 1) as usize) = b'-'; } }\n\
         \x20   off + w\n\
         }\n\n",
    );
}

/// Does this module compare strings anywhere? Only then is the helper emitted.
///
/// The arms are written out one per variant, with **no `_` catch-all**, and that is the whole
/// point. This walker used to end in `_ => false`, which made "a variant nobody thought about"
/// indistinguishable from "no comparison here" — and `Concat` was that variant. A comparison
/// reachable only through a concatenation piece did not turn the helper on, so
///
/// ```text
/// fn score(b: bool) -> i32 { if b { return 1 }  return 0 }
/// export fn label(a: string, b: string) -> string! { return "eq=" + score(a == b) as string }
/// ```
///
/// compiled here and then failed inside the generated crate with `error[E0425]: cannot find
/// function `ml_streq` in this scope` — a valid program rejected by an error in code the user
/// never wrote. Adding the missing arm fixes today's bug; deleting the catch-all is what stops
/// the next variant from reintroducing it silently, because then it will not build.
fn compares_strings(module: &IrModule) -> bool {
    fn in_expr(e: &IrExpr) -> bool {
        match &e.kind {
            // An element is a scalar, so an index never compares strings — but the INDEX
            // itself is an ordinary expression and could.
            IrExprKind::Index { index, .. } => in_expr(index),
            IrExprKind::Len { .. } => false,
            // Builds a string without comparing one — but all three children are subtrees, so
            // a comparison nested in an offset still has to be found.
            IrExprKind::ByteSlice { s, from, to } => in_expr(s) || in_expr(from) || in_expr(to),
            // Likewise: `fixed(rate(a == b), 2)` compares strings inside a formatted number.
            IrExprKind::Fixed { x, places } => in_expr(x) || in_expr(places),
            // Reads a string without comparing it — but its operand is a subtree, so a
            // comparison nested inside still has to be found.
            IrExprKind::ByteLen(operand) => in_expr(operand),
            IrExprKind::Binary { op, lhs, rhs } => {
                // A SPAN comparison does not use `ml_streq` — it lowers to `ml_subeq`, which
                // `emit_span_helpers` provides. Counting it here would emit `ml_streq` into a
                // module that never calls it, and a widened gate over-emits as easily as a
                // narrow one under-emits: dead code in the module is dead code in the
                // shipped `.dll` (`the_helper_is_only_emitted_when_a_comparison_exists`).
                (matches!(op, IrBinOp::Eq | IrBinOp::Ne)
                    && lhs.ty == IrType::Str
                    && !matches!(lhs.kind, IrExprKind::ByteSlice { .. })
                    && !matches!(rhs.kind, IrExprKind::ByteSlice { .. }))
                    || in_expr(lhs)
                    || in_expr(rhs)
            }
            IrExprKind::Unary { operand, .. } | IrExprKind::Cast { operand, .. } => {
                in_expr(operand)
            }
            IrExprKind::Call { args, .. } => args.iter().any(in_expr),
            IrExprKind::Concat(pieces) => pieces.iter().any(in_expr),
            IrExprKind::ConstF64(_)
            | IrExprKind::ConstStr(_)
            | IrExprKind::ConstI32(_)
            | IrExprKind::ConstBool(_)
            | IrExprKind::Var(_) => false,
        }
    }
    fn in_stmts(body: &[IrStmt]) -> bool {
        body.iter().any(|s| match s {
            IrStmt::If { cond, body } | IrStmt::While { cond, body } => {
                in_expr(cond) || in_stmts(body)
            }
            IrStmt::ResultLen(e) => in_expr(e),
            IrStmt::ResultSet { index, value } => in_expr(index) || in_expr(value),
            IrStmt::Return(e)
            | IrStmt::Let { value: e, .. }
            | IrStmt::Assign { value: e, .. }
            | IrStmt::AssignOut { value: e, .. } => in_expr(e),
            // A try-call's arguments are ordinary expressions and may compare strings.
            IrStmt::TryCall { args, .. } => args.iter().any(in_expr),
            IrStmt::Fail(_) => false,
        })
    }
    module.functions.iter().any(|f| in_stmts(&f.body))
}

/// Byte-equality for two NUL-terminated pointers (SPEC-string-input section 2.3).
///
/// Deliberately NOT `strcmp`: calling the CRT would add an import, and the module's import set
/// is one of the protection proxies acceptance C measures (D04/D05). A byte loop costs a few
/// lines and keeps the boundary unchanged.
///
/// Encoding is opaque (SPEC-string-input DP-S2). Equal bytes are equal strings; matching encodings between host
/// and source is a host contract, stated in HOST_ABI.
fn emit_string_helper(module: &IrModule, out: &mut String) {
    if !compares_strings(module) {
        return;
    }
    out.push_str(
        "fn ml_streq(a: *const u8, b: *const u8) -> bool {\n\
         \x20   let mut i = 0usize;\n\
         \x20   loop {\n\
         \x20       let (x, y) = unsafe { (*a.add(i), *b.add(i)) };\n\
         \x20       if x != y { return false; }\n\
         \x20       if x == 0 { return true; }\n\
         \x20       i += 1;\n\
         \x20   }\n\
         }\n\n",
    );
}

/// Does anything in this module take a span? Only then are the two span helpers emitted.
///
/// Its own predicate, like [`uses_byte_len`], and for the same accounting reason: a module
/// that only concatenates must stay byte-for-byte what it was, or the golden tests and the
/// import/size acceptance would move for a feature that module does not use.
fn uses_byte_slice(module: &IrModule) -> bool {
    fn in_expr(e: &IrExpr) -> bool {
        match &e.kind {
            IrExprKind::ByteSlice { .. } => true,
            // `false` for the node, `true` for its subtrees: `fixed` needs no span helper of
            // its own, but `fixed(x, byte_len(byte_slice(s, 0, 2)))` puts one underneath.
            IrExprKind::Fixed { x, places } => in_expr(x) || in_expr(places),
            IrExprKind::Index { index, .. } => in_expr(index),
            IrExprKind::Binary { lhs, rhs, .. } => in_expr(lhs) || in_expr(rhs),
            IrExprKind::Unary { operand, .. }
            | IrExprKind::Cast { operand, .. }
            | IrExprKind::ByteLen(operand) => in_expr(operand),
            IrExprKind::Call { args, .. } | IrExprKind::Concat(args) => args.iter().any(in_expr),
            IrExprKind::Len { .. }
            | IrExprKind::ConstF64(_)
            | IrExprKind::ConstStr(_)
            | IrExprKind::ConstI32(_)
            | IrExprKind::ConstBool(_)
            | IrExprKind::Var(_) => false,
        }
    }
    fn in_stmts(stmts: &[IrStmt]) -> bool {
        stmts.iter().any(|s| match s {
            IrStmt::If { cond, body } | IrStmt::While { cond, body } => {
                in_expr(cond) || in_stmts(body)
            }
            IrStmt::ResultLen(e) => in_expr(e),
            IrStmt::ResultSet { index, value } => in_expr(index) || in_expr(value),
            IrStmt::Return(e)
            | IrStmt::Let { value: e, .. }
            | IrStmt::Assign { value: e, .. }
            | IrStmt::AssignOut { value: e, .. } => in_expr(e),
            IrStmt::TryCall { args, .. } => args.iter().any(in_expr),
            IrStmt::Fail(_) => false,
        })
    }
    module.functions.iter().any(|f| in_stmts(&f.body))
}

/// The two helpers a span needs, and the reason they are two rather than a reuse.
///
/// Everything emitted before this slice stops at the SOURCE NUL: `ml_slen` counts to it,
/// `ml_wstr` copies to it. A span is bounded by a LENGTH instead, so counting or copying one
/// with those would take the whole suffix and ignore `to` — status 0, no warning
/// (`SPEC-string-slice` §2.5).
fn emit_span_helpers(module: &IrModule, out: &mut String) {
    if !uses_byte_slice(module) {
        return;
    }
    out.push_str(
        "fn ml_sublen(src: *const u8, from: i32, to: i32) -> i32 {\n\
         \x20   // The span's length, or -1 if it is not a span of THIS string. The caller\n\
         \x20   // turns that into ML_ST_INDEX_OUT_OF_RANGE, because a clamp would be the\n\
         \x20   // silent wrong answer D17's negative statuses exist to avoid (DP-B4).\n\
         \x20   if from < 0 || to < from { return -1; }\n\
         \x20   // `to` is compared against the string's actual length, so a span that runs\n\
         \x20   // past the NUL is refused rather than reading whatever follows it.\n\
         \x20   if to > ml_slen(src) { return -1; }\n\
         \x20   to - from\n\
         }\n\n",
    );
    if !returns_a_span(module) {
        return;
    }
    out.push_str(
        "fn ml_wsub(buf: *mut u8, off: i32, src: *const u8, from: i32, n: i32, cap: i32) -> i32 {\n\
         \x20   // `n` bytes, and the source NUL is NOT consulted: that is the whole point.\n\
         \x20   // `cap` is still a hard stop for the same aliasing case `ml_wstr` documents —\n\
         \x20   // nothing in the C ABI forbids a host from passing a string that overlaps its\n\
         \x20   // own output buffer. One byte is left for the terminator.\n\
         \x20   let mut i: i32 = 0;\n\
         \x20   while i < n {\n\
         \x20       if off + i >= cap - 1 { return off + i; }\n\
         \x20       unsafe { *buf.add((off + i) as usize) = *src.add((from + i) as usize); }\n\
         \x20       i += 1;\n\
         \x20   }\n\
         \x20   off + n\n\
         }\n\n",
    );
}

/// Does a span reach a `return` — alone, or as a piece of a concatenation?
///
/// That is the only path `ml_wsub` is called from (`emit_concat_return`), so it is the gate.
/// A module that only COMPARES spans never writes one.
fn returns_a_span(module: &IrModule) -> bool {
    // Two `if let`s rather than a match with a wildcard, and for the reason `i32_literal`
    // records: this file's rule is that a walker's every variant must ANSWER, but "does this
    // reach the buffer writer?" has one correct default for anything added later, which is
    // `no` — a new expression kind cannot be a concat piece without saying so.
    fn is_span_piece(e: &IrExpr) -> bool {
        if matches!(e.kind, IrExprKind::ByteSlice { .. }) {
            return true;
        }
        if let IrExprKind::Concat(pieces) = &e.kind {
            return pieces.iter().any(is_span_piece);
        }
        false
    }
    fn in_stmts(stmts: &[IrStmt]) -> bool {
        stmts.iter().any(|s| {
            if let IrStmt::If { body, .. } | IrStmt::While { body, .. } = s {
                return in_stmts(body);
            }
            if let IrStmt::Return(e) = s {
                return is_span_piece(e);
            }
            false
        })
    }
    module.functions.iter().any(|f| in_stmts(&f.body))
}

/// `ml_subeq` — emitted only for a module that COMPARES a span.
///
/// Split from the writer above for the reason `emit_slen_helper` was split from the concat
/// set (§9-40): a module that only cuts a span never calls this, and a module that only
/// compares one never calls `ml_wsub`. A widened gate over-emits as easily as a narrow one
/// under-emits, and dead code in the module is dead code in the shipped `.dll`.
fn emit_span_compare_helper(module: &IrModule, out: &mut String) {
    if !module
        .functions
        .iter()
        .any(|f| crate::ir::compares_a_span(&f.body))
    {
        return;
    }
    out.push_str(
        "fn ml_subeq(a: *const u8, from: i32, n: i32, b: *const u8) -> bool {\n\
         \x20   // `n` bytes of `a` against the NUL-terminated `b`, and the LENGTHS must match\n\
         \x20   // too. Two tests do that, and dropping either one makes the answer wrong in a\n\
         \x20   // different direction: the `y == 0` inside the loop catches a `b` that is\n\
         \x20   // SHORTER than the span, and the check after the loop catches a `b` that is\n\
         \x20   // LONGER. Without them `byte_slice(s, 0, 2) == \"ABC\"` would be true.\n\
         \x20   //\n\
         \x20   // `ml_streq` is not reusable here: it walks `a` to a NUL, and a span has none\n\
         \x20   // at `to`.\n\
         \x20   let mut i: i32 = 0;\n\
         \x20   while i < n {\n\
         \x20       let (x, y) = unsafe { (*a.add((from + i) as usize), *b.add(i as usize)) };\n\
         \x20       if y == 0 || x != y { return false; }\n\
         \x20       i += 1;\n\
         \x20   }\n\
         \x20   unsafe { *b.add(n as usize) == 0 }\n\
         }\n\n",
    );
}

/// Which built-in rounders this module actually calls, so an unused one is not emitted.
fn used_rounders(module: &IrModule) -> Vec<crate::typeck::Rounder> {
    fn walk_expr(e: &IrExpr, found: &mut Vec<crate::typeck::Rounder>) {
        match &e.kind {
            IrExprKind::Index { index, .. } => walk_expr(index, found),
            IrExprKind::Len { .. } => {}
            // No rounder can be its operand (a string), but the subtree is walked anyway —
            // this file's habit is that a node with children answers for its children.
            IrExprKind::ByteLen(operand) => walk_expr(operand, found),
            // The offsets are i32 and a rounder is f64 -> f64, so one can only appear through
            // a cast — which is a child, and children are walked.
            IrExprKind::ByteSlice { s, from, to } => {
                walk_expr(s, found);
                walk_expr(from, found);
                walk_expr(to, found);
            }
            // This one is not hypothetical: `fixed(round(x), 0)` is the obvious way to write
            // "rounded, no decimals", and missing it here would emit a call to `ml_round`
            // with no `fn ml_round` anywhere — #158's defect, in this helper family.
            IrExprKind::Fixed { x, places } => {
                walk_expr(x, found);
                walk_expr(places, found);
            }
            IrExprKind::Call { name, args, .. } => {
                if let Some(b) = crate::typeck::Rounder::from_name(name) {
                    if !found.contains(&b) {
                        found.push(b);
                    }
                }
                for a in args {
                    walk_expr(a, found);
                }
            }
            IrExprKind::Unary { operand, .. } | IrExprKind::Cast { operand, .. } => {
                walk_expr(operand, found)
            }
            IrExprKind::Binary { lhs, rhs, .. } => {
                walk_expr(lhs, found);
                walk_expr(rhs, found);
            }
            // A rounder can sit inside a piece: `"total " + (round(x) as i32 as string)`.
            IrExprKind::Concat(pieces) => {
                for p in pieces {
                    walk_expr(p, found);
                }
            }
            IrExprKind::ConstF64(_)
            | IrExprKind::ConstI32(_)
            | IrExprKind::ConstBool(_)
            | IrExprKind::ConstStr(_)
            | IrExprKind::Var(_) => {}
        }
    }
    fn walk_stmts(body: &[IrStmt], found: &mut Vec<crate::typeck::Rounder>) {
        for s in body {
            match s {
                IrStmt::If { cond, body } | IrStmt::While { cond, body } => {
                    walk_expr(cond, found);
                    walk_stmts(body, found);
                }
                IrStmt::ResultLen(e) => walk_expr(e, found),
                IrStmt::ResultSet { index, value } => {
                    walk_expr(index, found);
                    walk_expr(value, found);
                }
                IrStmt::Return(e)
                | IrStmt::Let { value: e, .. }
                | IrStmt::Assign { value: e, .. }
                | IrStmt::AssignOut { value: e, .. } => walk_expr(e, found),
                // Same for the rounding builtins: a try-call argument may use one, and
                // missing it here would emit a call to a helper that was never defined.
                IrStmt::TryCall { args, .. } => {
                    for a in args {
                        walk_expr(a, found);
                    }
                }
                IrStmt::Fail(_) => {}
            }
        }
    }
    let mut found = Vec::new();
    for f in &module.functions {
        walk_stmts(&f.body, &mut found);
    }
    found
}

/// Emit the rounding helpers this module needs (SPEC-rounding section 2.4).
///
/// `f64::floor` and friends live in `std`, not `core`, so a `no_std` cdylib cannot call them —
/// and taking `libm` would end this repo's zero-third-party-dependency property. These are
/// written with core operations only and are bit-exact with `std` (measured, including the
/// sign of zero).
/// Exact truncation, shared by the rounders and by `fixed`.
///
/// Split onto its own gate when `fixed` arrived, because leaving it inside
/// [`emit_rounding_helpers`] would have forced one of two wrong answers: widen `used_rounders`
/// so a `fixed`-only module also gets `ml_round`, `ml_floor` and friends it never calls, or
/// give `fixed` a second copy of the truncation — and §9-47's lesson is that a widened gate
/// over-emits as easily as a narrow one under-emits. A second copy would be worse still: the
/// half-away-from-zero rule is measured, and two copies are two chances to get it wrong.
fn emit_trunc_helpers(module: &IrModule, out: &mut String) {
    if used_rounders(module).is_empty() && !uses_fixed(module) {
        return;
    }
    // 2^53. At or above it every f64 is already an integer, so nothing needs doing — and
    // below it the i64 round-trip is exact, which is why this cannot saturate the way the
    // `as i32` workaround it replaces did.
    out.push_str(
        "const ML_INTEGRAL: f64 = 9007199254740992.0;\n\
         fn ml_trunc_raw(x: f64) -> f64 {\n\
         \x20   if x != x { return x; }\n\
         \x20   if x >= ML_INTEGRAL || x <= -ML_INTEGRAL { return x; }\n\
         \x20   (x as i64) as f64\n\
         }\n\n",
    );
}

fn emit_rounding_helpers(module: &IrModule, out: &mut String) {
    let used = used_rounders(module);
    if used.is_empty() {
        return;
    }
    // `ml_sz` stays HERE rather than moving up with the truncation: `fixed` produces an i64,
    // which has no signed zero, so it is the rounders and only the rounders that need this.
    out.push_str(
        "// Carry x's sign onto a zero result: IEEE gives a product the XOR of the operand\n\
         // signs, so `x * 0.0` is exactly \"zero with x's sign\". Without this, ceil(-0.5)\n\
         // would be +0.0 and C would disagree (DP-R3).\n\
         fn ml_sz(r: f64, x: f64) -> f64 { if r == 0.0 { x * 0.0 } else { r } }\n",
    );
    // Exhaustive over `Rounder`, with no `_` arm. It matched on `&str` and ended `_ => {}`,
    // which made a fifth builtin a silent nothing: measured, adding `"sqrt"` to the list let
    // `return sqrt(x)` pass the frontend and emit `ml_sqrt(x)` with no `fn ml_sqrt` anywhere
    // — the generated crate calling a function it does not define, which is #158's defect in
    // a different helper family. Now the same edit does not build until this match answers.
    for r in &used {
        match r {
            Rounder::Trunc => {
                out.push_str("fn ml_trunc(x: f64) -> f64 { ml_sz(ml_trunc_raw(x), x) }\n")
            }
            Rounder::Floor => out.push_str(
                "fn ml_floor(x: f64) -> f64 { let t = ml_trunc_raw(x); ml_sz(if t > x { t - 1.0 } else { t }, x) }\n",
            ),
            Rounder::Ceil => out.push_str(
                "fn ml_ceil(x: f64) -> f64 { let t = ml_trunc_raw(x); ml_sz(if t < x { t + 1.0 } else { t }, x) }\n",
            ),
            // NOT `floor(x + 0.5)`: that returns 1 for 0.49999999999999994. The fractional
            // part is computed exactly instead (`x - trunc(x)` is exact below 2^53).
            Rounder::Round => out.push_str(
                "fn ml_round(x: f64) -> f64 { let t = ml_trunc_raw(x); if t != t { return t; } let d = x - t; ml_sz(if d >= 0.5 { t + 1.0 } else if d <= -0.5 { t - 1.0 } else { t }, x) }\n",
            ),
        }
    }
    out.push('\n');
}

/// Does this module call `fixed(x, places)` anywhere?
///
/// Exhaustive, like its neighbours, and with the same consequence if it were not: a module
/// whose only `fixed` sat inside a concatenation piece would compile here and then fail
/// inside the generated crate with `cannot find function ml_fixscale` — a valid program
/// rejected by an error in code the user never wrote (#158).
fn uses_fixed(module: &IrModule) -> bool {
    fn in_expr(e: &IrExpr) -> bool {
        match &e.kind {
            IrExprKind::Fixed { .. } => true,
            IrExprKind::ByteSlice { s, from, to } => in_expr(s) || in_expr(from) || in_expr(to),
            IrExprKind::Index { index, .. } => in_expr(index),
            IrExprKind::Binary { lhs, rhs, .. } => in_expr(lhs) || in_expr(rhs),
            IrExprKind::Unary { operand, .. }
            | IrExprKind::Cast { operand, .. }
            | IrExprKind::ByteLen(operand) => in_expr(operand),
            IrExprKind::Call { args, .. } | IrExprKind::Concat(args) => args.iter().any(in_expr),
            IrExprKind::Len { .. }
            | IrExprKind::ConstF64(_)
            | IrExprKind::ConstStr(_)
            | IrExprKind::ConstI32(_)
            | IrExprKind::ConstBool(_)
            | IrExprKind::Var(_) => false,
        }
    }
    fn in_stmts(stmts: &[IrStmt]) -> bool {
        stmts.iter().any(|s| match s {
            IrStmt::If { cond, body } | IrStmt::While { cond, body } => {
                in_expr(cond) || in_stmts(body)
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
    module.functions.iter().any(|f| in_stmts(&f.body))
}

/// The three helpers `fixed(x, places)` needs, and why none of them is a reuse.
///
/// `ml_ilen`/`ml_wint` are the closest thing already here, and they take an `i32` — which is
/// exactly what `SPEC-fixed-decimals` §2.3 forbids: the measured workaround answered
/// `"21474836.47"` for fifty million because `as i32` saturates. These work in i64 and the
/// scaling is done once, before either pass, so the two passes cannot disagree.
fn emit_fixed_helpers(module: &IrModule, out: &mut String) {
    if !uses_fixed(module) {
        return;
    }
    out.push_str(
        "fn ml_fixscale(x: f64, k: i32) -> (i64, bool) {\n\
         \x20   // `x` as a whole number of 10^-k units, or `false` if there is no such\n\
         \x20   // number. The caller turns that into ML_ST_INDEX_OUT_OF_RANGE — a shared\n\
         \x20   // status, because a new reserved one costs what #238 measured (DP-M4).\n\
         \x20   //\n\
         \x20   // `k` is re-checked here and not only in the typechecker, because only a\n\
         \x20   // LITERAL count can be checked there: `fixed(x, n)` for a parameter arrives\n\
         \x20   // at run time.\n\
         \x20   if k < 0 || k > 9 { return (0, false); }\n\
         \x20   let mut p: f64 = 1.0;\n\
         \x20   let mut i: i32 = 0;\n\
         \x20   while i < k { p *= 10.0; i += 1; }\n\
         \x20   let s = x * p;\n\
         \x20   // Half away from zero — the direction `ml_round` takes, computed the way\n\
         \x20   // `ml_round` computes it. NOT `floor(s + 0.5)`: that answers 1 for\n\
         \x20   // 0.49999999999999994. `d` is exact on both sides of 2^53, for two different\n\
         \x20   // reasons: below it `s - trunc(s)` is exact, and at or above it every f64 is\n\
         \x20   // already an integer, so `ml_trunc_raw` returns `s` and `d` is exactly 0.\n\
         \x20   // (An earlier version of this comment claimed every surviving value was\n\
         \x20   // below 2^53. That is false — the range check below admits up to ~2^63 —\n\
         \x20   // and the code was right for the reason the sentence did not give.)\n\
         \x20   let t = ml_trunc_raw(s);\n\
         \x20   let d = s - t;\n\
         \x20   let r = if d >= 0.5 { t + 1.0 } else if d <= -0.5 { t - 1.0 } else { t };\n\
         \x20   // Three refusals in one test. `r != r` catches NaN, which fails the other two\n\
         \x20   // as well (every comparison with NaN is false) and would otherwise pass. 2^63\n\
         \x20   // is exactly representable, so `>=` is the true i64 ceiling and an infinity\n\
         \x20   // fails it. The floor is written one representable value ABOVE i64::MIN so\n\
         \x20   // that the `-v` in the two helpers below cannot overflow — one value refused,\n\
         \x20   // against an arithmetic overflow in the middle of formatting.\n\
         \x20   if r != r || r <= -9223372036854775808.0 || r >= 9223372036854775808.0 {\n\
         \x20       return (0, false);\n\
         \x20   }\n\
         \x20   (r as i64, true)\n\
         }\n\n\
         fn ml_fixlen(v: i64, k: i32) -> i32 {\n\
         \x20   // Width of `[-]<int>.<k digits>`. Three rules, and the measured workaround in\n\
         \x20   // `SPEC-fixed-decimals` §0.1 broke each of them separately: the integer part\n\
         \x20   // is at least one digit (`0.07`, never `.07`), the fraction is exactly `k`\n\
         \x20   // digits, and the sign is counted ONCE at the front (`-0.07`, never `0.-7`).\n\
         \x20   let neg = v < 0;\n\
         \x20   let mut m: i64 = if neg { -v } else { v };\n\
         \x20   let mut i: i32 = 0;\n\
         \x20   while i < k { m /= 10; i += 1; }\n\
         \x20   let mut n: i32 = if neg { 2 } else { 1 };\n\
         \x20   while m >= 10 { m /= 10; n += 1; }\n\
         \x20   // The point exists only when there is a fraction to separate (`places == 0`\n\
         \x20   // is \"1234\", not \"1234.\").\n\
         \x20   if k > 0 { n + k + 1 } else { n }\n\
         }\n\n\
         fn ml_wfix(buf: *mut u8, off: i32, v: i64, k: i32) -> i32 {\n\
         \x20   // Width comes from ml_fixlen, the same function pass 1 used, then the span is\n\
         \x20   // filled right-to-left — `ml_wint`'s rule, for `ml_wint`'s reason: two counts\n\
         \x20   // that could drift would be a write past the end of the host's buffer.\n\
         \x20   //\n\
         \x20   // No `cap` bound here, unlike `ml_wstr` and `ml_wsub`. Those two copy FROM a\n\
         \x20   // host pointer that may alias the output buffer; these bytes come out of an\n\
         \x20   // i64, so the destination is the only address in play and `ml_cap >= __n` was\n\
         \x20   // already checked by the caller.\n\
         \x20   let w = ml_fixlen(v, k);\n\
         \x20   let neg = v < 0;\n\
         \x20   let mut m: i64 = if neg { -v } else { v };\n\
         \x20   let mut i = off + w;\n\
         \x20   // The fraction first: exactly `k` digits, zero-filled. That zero fill is the\n\
         \x20   // `\"1234.5\"`-for-1234.05 defect, closed by construction — the loop runs `k`\n\
         \x20   // times whatever `m` holds.\n\
         \x20   let mut j: i32 = 0;\n\
         \x20   while j < k {\n\
         \x20       i -= 1;\n\
         \x20       unsafe { *buf.add(i as usize) = b'0' + (m % 10) as u8; }\n\
         \x20       m /= 10;\n\
         \x20       j += 1;\n\
         \x20   }\n\
         \x20   if k > 0 { i -= 1; unsafe { *buf.add(i as usize) = b'.'; } }\n\
         \x20   // The integer part, at least one digit even when it is zero.\n\
         \x20   loop {\n\
         \x20       i -= 1;\n\
         \x20       unsafe { *buf.add(i as usize) = b'0' + (m % 10) as u8; }\n\
         \x20       m /= 10;\n\
         \x20       if m == 0 { break; }\n\
         \x20   }\n\
         \x20   // Once, at the front, and only when the SCALED value is negative — so a `x`\n\
         \x20   // that rounds to zero prints \"0.00\" and never \"-0.00\" (DP-M6).\n\
         \x20   if neg { unsafe { *buf.add((i - 1) as usize) = b'-'; } }\n\
         \x20   off + w\n\
         }\n\n",
    );
}

fn op_str(op: IrBinOp) -> &'static str {
    match op {
        IrBinOp::Add => "+",
        IrBinOp::Sub => "-",
        IrBinOp::Mul => "*",
        // `Div` reaches here only for f64; the i32 form is emitted guarded above and never
        // takes this path. `Rem` is i32-only (DP-D4), so it never reaches here at all —
        // the arm exists so a future f64 `%` cannot be added without noticing this.
        IrBinOp::Div => "/",
        IrBinOp::Rem => "%",
        IrBinOp::Lt => "<",
        IrBinOp::Gt => ">",
        IrBinOp::Le => "<=",
        IrBinOp::Ge => ">=",
        IrBinOp::Eq => "==",
        IrBinOp::Ne => "!=",
        // Rust's `&&`/`||` short-circuit, which is what SPEC-logical-ops DP-B2 specifies.
        //
        // This used to add "and nothing in the language can observe that yet". It can now:
        // an out-of-range index RETURNS `ML_ST_INDEX_OUT_OF_RANGE` from expression position,
        // so `false && xs[99] > 0` answering 0 rather than -2 is the surface measurement
        // (`logical_ops.rs`, STATUS §5-1). The observation arrived with array input and this
        // comment kept saying it had not.
        IrBinOp::And => "&&",
        IrBinOp::Or => "||",
    }
}

/// Write `rust_src` as a `cdylib` crate under `workdir/<crate_name>` and build it,
/// returning the produced DLL path. Requires `cargo` on PATH (D19/D20). Windows target
/// (D22): the artifact is `<crate_name>.dll`.
///
/// This is a caller-supplied-dir primitive: the caller must pass a **unique** `workdir` per
/// concurrent invocation (`emit_artifacts` uses a private temp tree; the oracle tests
/// namespace theirs per process). Two calls sharing a `workdir` would race.
/// What one cdylib build leaves behind, inside the private build tree.
///
/// Both are the linker's output for the same invocation, so they are returned together
/// rather than one being reconstructed from the other's path by the caller.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CdylibArtifacts {
    /// The module itself, `<workdir>/<crate>/target/release/<crate>.dll`.
    pub dll: PathBuf,
    /// The MSVC import library, `…/<crate>.dll.lib`. Lets a host bind at LINK time from the
    /// generated header, instead of resolving every symbol through `GetProcAddress`.
    pub import_lib: PathBuf,
}

pub fn build_cdylib(
    rust_src: &str,
    crate_name: &str,
    workdir: &Path,
) -> Result<CdylibArtifacts, CodegenError> {
    // The module name now appears in TWO places that are supplied separately: baked into
    // `rust_src` by `emit`, as the fingerprint export's suffix, and passed here as the name
    // the artifact is built under. Separate arguments are exactly how they can disagree —
    // `compile_to_rust` has no file behind it and emits `ml_iface_hash_unnamed`.
    //
    // A DLL built from that would export a fingerprint naming a module that does not exist.
    // The dynamic host would fail its `GetProcAddress` and the linked host would fail at
    // LINK time, both far from the cause. So this is the join where the two names are
    // compared, and it refuses rather than writes.
    //
    // Matched on the whole emitted declaration, not on the name as a substring: `discount`
    // is a prefix of `discount4`, and `contains("ml_iface_hash_discount")` would accept
    // `discount4`'s module as `discount`'s.
    let expected = format!("pub extern \"C\" fn ml_iface_hash_{crate_name}()");
    if !rust_src.contains(&expected) {
        return Err(CodegenError::new(format!(
            "refusing to build '{crate_name}': the emitted source does not export \
             `ml_iface_hash_{crate_name}`. The fingerprint export is qualified with the \
             module name (SPEC-qualified-iface-hash), so the source must be produced by \
             `compile_to_rust_named(src, \"{crate_name}\")` — `compile_to_rust` has no name \
             and emits `ml_iface_hash_{UNNAMED_MODULE}`"
        )));
    }
    let crate_dir = workdir.join(crate_name);
    // Where cargo is TOLD to put its output, so `release` below is that same path rather than
    // cargo's default reconstructed by hand. Kept inside the crate directory, which is where
    // the default would have been, so the temp tree this caller removes is unchanged.
    let target_dir = crate_dir.join("target");
    std::fs::create_dir_all(crate_dir.join("src"))
        .map_err(|e| CodegenError::new(format!("create crate dir: {e}")))?;
    std::fs::write(
        crate_dir.join("Cargo.toml"),
        format!(
            "[package]\nname = \"{crate_name}\"\nversion = \"0.0.0\"\nedition = \"2021\"\npublish = false\n\n[lib]\ncrate-type = [\"cdylib\"]\n\n{CARGO_TOML_PROFILE}"
        ),
    )
    .map_err(|e| CodegenError::new(format!("write Cargo.toml: {e}")))?;
    std::fs::write(crate_dir.join("src").join("lib.rs"), rust_src)
        .map_err(|e| CodegenError::new(format!("write lib.rs: {e}")))?;

    // Capture cargo's output instead of inheriting stdio. Inheriting leaked the generated
    // crate's build chatter into the user's console: a `Compiling <name> (…\Temp\mlc-build-…)`
    // line, and any rustc diagnostic pointing at `src/lib.rs:<line>` — a file the user never
    // wrote, in a directory that is deleted right afterwards. On success there is nothing
    // worth showing; on failure the text belongs IN the error, where a caller can handle it,
    // rather than having already scrolled past.
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
    let out = Command::new(cargo)
        .args(["build", "--release", "--manifest-path"])
        .arg(crate_dir.join("Cargo.toml"))
        // Say where the output goes, instead of reconstructing it below and hoping. Cargo
        // honours an ambient `CARGO_TARGET_DIR`, and `mlc` is very often run from inside a
        // cargo build that has set one — so the artifacts landed somewhere else and the
        // reconstruction failed:
        //
        //     CARGO_TARGET_DIR=… mlc build ovf.mls
        //     mlc: codegen error: expected dll not found:
        //          …\mlc-build-71108-1\ovf\target\release\ovf.dll
        //
        // A build that works or not depending on an environment variable the user never
        // mentioned. `--target-dir` overrides the variable, which turns the path below from a
        // guess into the same value cargo was told.
        .arg("--target-dir")
        .arg(&target_dir)
        .output()
        .map_err(|e| CodegenError::new(format!("spawn cargo: {e}")))?;
    if !out.status.success() {
        // Say whose line numbers these are. The positions are in emitted Rust, and by the
        // time anyone reads this the file is usually gone, so a bare paste would send the
        // reader looking for a `src/lib.rs` that is not theirs.
        let stderr = String::from_utf8_lossy(&out.stderr);
        return Err(CodegenError::new(format!(
            "cargo build of generated crate failed — the positions below are in the \
             GENERATED Rust, not in your source:\n{}",
            stderr.trim_end()
        )));
    }

    // FOUND, not reconstructed. `--target-dir` above says WHERE cargo works; it does not say
    // what shape it builds underneath, and that shape is configurable by variables this
    // compiler does not own. #164 replaced a guess about the root with a value cargo was told,
    // and left the guess about the layout below it — measured, still broken afterwards:
    //
    //     CARGO_BUILD_TARGET=x86_64-pc-windows-msvc mlc build examples/discount.mls
    //     mlc: codegen error: expected dll not found: …\discount\target\release\discount.dll
    //
    // because cargo puts a `<triple>/` component in when it is cross-compiling by request.
    // Enumerating the variables that do this is the fourth hand-kept list in a repository that
    // has watched three of them fail; the directory itself is the answer, and it is fresh —
    // `target_dir` lives inside a temp crate this call just created, so nothing stale can be
    // in it and "exactly one" is a real check rather than a hope.
    let dll = single_artifact(&target_dir, &format!("{crate_name}.dll"))?;

    // The import library the linker produced alongside the DLL. Named from `crate_name`
    // rather than derived from `dll` — `Path::with_extension("dll.lib")` happens to work
    // only because it re-embeds the `dll.`, and that is a reconstruction, not a contract
    // (Grok raised this while planning the slice).
    //
    // rustc passes `/IMPLIB:<crate>.dll.lib` for a cdylib on this target, which is why the
    // name carries both extensions: it keeps the import library from colliding with the
    // `<crate>.lib` a staticlib would produce. The published artifact drops back to
    // `<module>.lib` in `emit` — cargo's name is a build detail, not a deliverable.
    let import_lib =
        single_artifact(&target_dir, &format!("{crate_name}.dll.lib")).map_err(|e| {
            CodegenError::new(format!(
                "the cdylib built but its import library is missing. Without it a host can only \
             bind through GetProcAddress/dlsym, never by linking the generated header. {}",
                e.message
            ))
        })?;

    Ok(CdylibArtifacts { dll, import_lib })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compile_to_ir;

    #[test]
    fn emit_discount_produces_extern_c_rust() {
        let ir = compile_to_ir(include_str!("../../examples/discount.mls")).unwrap();
        let rust = emit(&ir, "discount").expect("emit");
        assert!(
            rust.contains(r#"pub extern "C" fn mlx_discount(price: f64, vip: bool) -> f64"#),
            "{rust}"
        );
        assert!(rust.contains("#![no_std]"), "{rust}");
        assert!(rust.contains("ml_module_abi_version"), "{rust}");
        assert!(rust.contains("if vip"), "{rust}");
        assert!(rust.contains("return price"), "{rust}");
    }

    #[test]
    fn rejects_function_without_guaranteed_return() {
        // Only statement is an `if` (no trailing return) → some path returns no value.
        let module = IrModule {
            errors: vec![],
            functions: vec![IrFunction {
                name: "f".into(),
                params: vec![IrParam {
                    name: "b".into(),
                    ty: IrType::Bool,
                    out: false,
                }],
                ret: IrType::F64,
                fallible: false,
                exported: true,
                body: vec![IrStmt::If {
                    cond: IrExpr {
                        ty: IrType::Bool,
                        kind: IrExprKind::Var("b".into()),
                    },
                    body: vec![IrStmt::Return(IrExpr {
                        ty: IrType::F64,
                        kind: IrExprKind::ConstF64(1.0),
                    })],
                }],
            }],
        };
        let err = emit(&module, "fallthrough").unwrap_err();
        assert!(
            err.message.contains("return on all paths"),
            "{}",
            err.message
        );
    }
}

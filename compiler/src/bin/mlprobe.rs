//! `mlprobe` — snippet → build → load → call → **print the value**.
//!
//! # Why this exists
//!
//! `STATUS.md` §7 has said for a long time that the absence of this tool has a cost:
//!
//! > **컴파일 여부가 아니라 값을 재라.** `mlc build`는 한 줄이지만 "그래서 얼마?"는 로더 15줄이다.
//! > 그 비대칭 때문에 **"컴파일되고, 그럴듯하고, 틀린" 결함이 네 번 통과**했다. 표준 도구
//! > (스니펫 → 빌드 → 로드 → 호출 → 값 출력)가 없으면 다음에도 싼 쪽만 재게 된다.
//!
//! §9-46 paid that cost again: measuring six business rules meant writing a throwaway test
//! file, running it and deleting it, and one attempt did not even compile because an
//! `include_str!` path crossed drives. Compilation alone would have hidden `round`'s
//! direction (`1235.5 -> 1236`, away from zero) and the answer for an empty array.
//!
//! # How it avoids the hard part
//!
//! Calling an arbitrary C ABI signature from Rust at run time needs either a combinatorial
//! dispatch table or a foreign-function library. This does neither: it **generates** a
//! ~40-line Rust harness whose `transmute` is written with the exact types taken from the
//! module's own IR, compiles it with a single `rustc` invocation (no cargo, no dependencies —
//! it declares `LoadLibraryA`/`GetProcAddress` itself), and runs it.
//!
//! So the tool never has to know the shapes; the compiler already does.
//!
//! # Usage
//!
//! ```text
//! mlprobe <file.mls> <function> [arg ...]
//! mlprobe --src '<source>' <function> [arg ...]
//! ```
//!
//! Arguments are positional and match the function's non-`out` parameters in order.
//! An array is written as a comma-separated list: `1,2,3` (or `-` for empty).
//! `out` parameters and the fallible `out_value` are supplied by the harness and printed.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use mlc::ir::{IrArrayElem, IrFunction, IrType};

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match run(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(msg) => {
            eprintln!("mlprobe: {msg}");
            eprintln!("usage: mlprobe <file.mls> <function> [arg ...]");
            eprintln!("       mlprobe --src '<source>' <function> [arg ...]");
            ExitCode::FAILURE
        }
    }
}

fn run(args: &[String]) -> Result<(), String> {
    let (src, origin, rest) = match args.first().map(String::as_str) {
        Some("--src") => {
            let s = args.get(1).ok_or("--src needs a source string")?;
            (s.clone(), "<--src>".to_string(), &args[2..])
        }
        Some(path) => {
            let p = Path::new(path);
            let s = std::fs::read_to_string(p).map_err(|e| format!("{path}: {e}"))?;
            (s, path.to_string(), &args[1..])
        }
        None => return Err("no input given".into()),
    };
    let fname = rest.first().ok_or("no function name given")?;
    let call_args = &rest[1..];

    let ir = mlc::compile_to_ir(&src).map_err(|e| format!("{origin}: {e}"))?;
    let func = ir
        .functions
        .iter()
        .find(|f| &f.name == fname)
        .ok_or_else(|| {
            let names: Vec<&str> = ir
                .functions
                .iter()
                .filter(|f| f.exported)
                .map(|f| f.name.as_str())
                .collect();
            format!("no function '{fname}' — this module exports {names:?}")
        })?;
    if !func.exported {
        return Err(format!(
            "'{fname}' is internal, so it is not in the export table and cannot be called \
             across the ABI"
        ));
    }

    // A module name the fingerprint symbol can carry, derived from the input the way `mlc`
    // does it, so a probe of a file and a probe of the same text behave the same.
    //
    // The fallback is the FUNCTION name, not a fixed word, and that is not cosmetic. The
    // first version used `probe`, and on this development machine a DLL named `probe.dll`
    // cannot be written at all — `mlc build` fails identically with `os error 5` (access
    // denied) while the same source under any other stem succeeds. An endpoint-security
    // agent, not the compiler; measured by renaming and re-running, twice each way. The
    // lesson is cheap to apply: do not bake a name a scanner might dislike into a tool.
    let module = Path::new(&origin)
        .file_stem()
        .and_then(|s| s.to_str())
        .filter(|s| !s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'))
        .unwrap_or(fname)
        .to_string();

    let work = scratch_dir();
    std::fs::create_dir_all(&work).map_err(|e| format!("{}: {e}", work.display()))?;
    // Held for the rest of `run`: every `?` below unwinds through this, and so does a panic.
    let _scratch = Scratch(work.clone());
    let arts = mlc::emit::emit_artifacts(&src, &module, &work).map_err(|e| e.to_string())?;

    let harness = generate_harness(
        &arts.dll,
        &module,
        mlc::iface::fingerprint(&ir),
        func,
        call_args,
    )?;
    let hs = work.join("probe_harness.rs");
    std::fs::write(&hs, &harness).map_err(|e| format!("{}: {e}", hs.display()))?;

    let exe = work.join(if cfg!(windows) {
        "probe_harness.exe"
    } else {
        "probe_harness"
    });
    let out = std::process::Command::new("rustc")
        .arg("-O")
        .arg(&hs)
        .arg("-o")
        .arg(&exe)
        .output()
        .map_err(|e| format!("run rustc: {e}"))?;
    if !out.status.success() {
        // The generated source is printed on failure. A harness that does not compile is a
        // bug in THIS tool, and the only way to see it is to read what it wrote.
        return Err(format!(
            "the generated harness did not compile — that is a bug in mlprobe:\n{}\n\
             ---- harness ----\n{harness}",
            String::from_utf8_lossy(&out.stderr)
        ));
    }

    let run = std::process::Command::new(&exe)
        .output()
        .map_err(|e| format!("run the harness: {e}"))?;
    print!("{}", String::from_utf8_lossy(&run.stdout));
    eprint!("{}", String::from_utf8_lossy(&run.stderr));
    if !run.status.success() {
        return Err("the harness exited non-zero".into());
    }
    Ok(())
}

/// A scratch tree that removes itself on drop — including while a panic unwinds.
///
/// **The first version of this tool did not have it, and left 27 trees in one afternoon.**
/// That is `STATUS.md` §5-5.7 for the third time, made by the same hand that had just spent a
/// change converting nineteen test files away from it — in a file the new guard did not scan,
/// because the guard looked at `tests/` and this is a `src/bin`. The guard's scope was the
/// defect; it now covers this directory too.
///
/// Not `common::TempOut`: that helper lives under `tests/`, and a binary cannot import it.
/// Duplicating twenty lines is the smaller cost. What must NOT be duplicated is the mistake.
struct Scratch(PathBuf);

impl Drop for Scratch {
    fn drop(&mut self) {
        // Retried for the reason the test helper records: on Windows a DLL this process just
        // loaded — or a child `rustc` just wrote — can stay locked for a moment after the
        // handle goes, and one best-effort removal silently loses that race.
        for _ in 0..20 {
            if std::fs::remove_dir_all(&self.0).is_ok() || !self.0.exists() {
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(25));
        }
    }
}

fn scratch_dir() -> PathBuf {
    std::env::temp_dir().join(format!("mlprobe_{}", std::process::id()))
}

/// The Rust type each parameter lowers to, in ABI order.
///
/// Mirrors what `codegen` emits rather than guessing: an array becomes TWO parameters (the
/// borrowed pointer and the length the compiler appends — `SPEC-array-input` DP-A2), a
/// declared `out` becomes `*mut T`, and a string is a borrowed `*const u8` (DP-S1).
fn param_types(f: &IrFunction) -> Vec<String> {
    let mut v = Vec::new();
    for p in &f.params {
        if p.out {
            v.push(format!("*mut {}", scalar_rust(&p.ty)));
            continue;
        }
        match &p.ty {
            IrType::Array(e) => {
                v.push(format!("*const {}", elem_rust(e)));
                v.push("i32".to_string());
            }
            IrType::Str => v.push("*const u8".to_string()),
            // Spelled out rather than `_`, the rule this crate denies a wildcard for: a new
            // parameter type must say what it lowers to instead of inheriting "scalar".
            IrType::F64 | IrType::I32 | IrType::Bool => v.push(scalar_rust(&p.ty).to_string()),
        }
    }
    v
}

fn scalar_rust(t: &IrType) -> &'static str {
    match t {
        IrType::F64 => "f64",
        IrType::I32 => "i32",
        IrType::Bool => "bool",
        IrType::Str => "u8",
        IrType::Array(_) => "u8",
    }
}

fn elem_rust(e: &IrArrayElem) -> &'static str {
    match e {
        IrArrayElem::F64 => "f64",
        IrArrayElem::I32 => "i32",
        IrArrayElem::Bool => "bool",
    }
}

/// Everything the harness needs, generated from the signature.
fn generate_harness(
    dll: &Path,
    module: &str,
    iface_hash: u64,
    f: &IrFunction,
    args: &[String],
) -> Result<String, String> {
    let inputs: Vec<&mlc::ir::IrParam> = f.params.iter().filter(|p| !p.out).collect();
    if args.len() != inputs.len() {
        let want: Vec<String> = inputs
            .iter()
            .map(|p| format!("{}: {}", p.name, p.ty))
            .collect();
        return Err(format!(
            "'{}' takes {} argument(s) — {:?} — and {} were given",
            f.name,
            inputs.len(),
            want,
            args.len()
        ));
    }

    let mut setup = String::new();
    let mut call: Vec<String> = Vec::new();
    let mut report: Vec<String> = Vec::new();
    let mut i = 0usize;
    for p in &f.params {
        if p.out {
            let t = scalar_rust(&p.ty);
            setup.push_str(&format!(
                "    let mut out_{n}: {t} = {zero};\n",
                n = p.name,
                zero = zero_of(&p.ty)
            ));
            call.push(format!("&mut out_{}", p.name));
            report.push(format!(
                "    println!(\"  out {n} = {{:?}}\", out_{n});\n",
                n = p.name
            ));
            continue;
        }
        let a = &args[i];
        i += 1;
        match &p.ty {
            IrType::Str => {
                setup.push_str(&format!(
                    "    let mut s_{n}: Vec<u8> = {lit}.to_vec(); s_{n}.push(0);\n",
                    n = p.name,
                    lit = byte_string(a)
                ));
                call.push(format!("s_{}.as_ptr()", p.name));
            }
            IrType::Array(e) => {
                let t = elem_rust(e);
                let items = parse_list(a, e)?;
                setup.push_str(&format!(
                    "    let v_{n}: Vec<{t}> = vec![{items}];\n",
                    n = p.name
                ));
                call.push(format!("v_{}.as_ptr()", p.name));
                call.push(format!("v_{}.len() as i32", p.name));
            }
            IrType::F64 => {
                let v: f64 = a.parse().map_err(|_| format!("'{a}' is not an f64"))?;
                call.push(format!("{v:?}f64"));
            }
            IrType::I32 => {
                let v: i32 = a.parse().map_err(|_| format!("'{a}' is not an i32"))?;
                call.push(format!("{v}i32"));
            }
            IrType::Bool => {
                let v: bool = a.parse().map_err(|_| format!("'{a}' is not a bool"))?;
                call.push(v.to_string());
            }
        }
    }

    let mut types = param_types(f);
    let ret_rust;
    let tail;
    if f.fallible {
        match &f.ret {
            IrType::Str => {
                setup.push_str(
                    "    let mut buf = vec![0u8; 4096];\n    let mut needed: i32 = -7;\n",
                );
                types.push("*mut u8".into());
                types.push("i32".into());
                types.push("*mut i32".into());
                call.push("buf.as_mut_ptr()".into());
                call.push("buf.len() as i32".into());
                call.push("&mut needed".into());
                ret_rust = "i32";
                tail = "    println!(\"  needed = {}\", needed);\n\
                        \x20   if status == 0 {\n\
                        \x20       let n = buf.iter().position(|&b| b == 0).unwrap_or(0);\n\
                        \x20       println!(\"  value  = {:?}\", String::from_utf8_lossy(&buf[..n]));\n\
                        \x20   }\n"
                    .to_string();
            }
            IrType::Array(e) => {
                let t = elem_rust(e);
                setup.push_str(&format!(
                    "    let mut buf = vec![{z}; 256];\n    let mut needed: i32 = -7;\n",
                    z = match e {
                        IrArrayElem::F64 => "0.0f64",
                        IrArrayElem::I32 => "0i32",
                        IrArrayElem::Bool => "false",
                    }
                ));
                types.push(format!("*mut {t}"));
                types.push("i32".into());
                types.push("*mut i32".into());
                call.push("buf.as_mut_ptr()".into());
                call.push("buf.len() as i32".into());
                call.push("&mut needed".into());
                ret_rust = "i32";
                tail = "    println!(\"  needed = {}\", needed);\n\
                        \x20   if status == 0 {\n\
                        \x20       println!(\"  value  = {:?}\", &buf[..needed.max(0) as usize]);\n\
                        \x20   }\n"
                    .to_string();
            }
            // Same rule: a new RETURN type has to choose a protocol rather than be assumed
            // to fit in one `*mut T`.
            scalar @ (IrType::F64 | IrType::I32 | IrType::Bool) => {
                let t = scalar_rust(scalar);
                setup.push_str(&format!(
                    "    let mut value: {t} = {z};\n",
                    z = zero_of(scalar)
                ));
                types.push(format!("*mut {t}"));
                call.push("&mut value".into());
                ret_rust = "i32";
                tail = "    if status == 0 { println!(\"  value  = {:?}\", value); }\n".to_string();
            }
        }
    } else {
        ret_rust = scalar_rust(&f.ret);
        tail = String::new();
    }

    let sym = format!("mlx_{}", f.name);
    let hash_sym = format!("ml_iface_hash_{module}");
    let head = if f.fallible {
        format!(
            "    let status = f({});\n    println!(\"status = {{}}\", status);\n",
            call.join(", ")
        )
    } else {
        format!(
            "    let value = f({});\n    println!(\"value  = {{:?}}\", value);\n",
            call.join(", ")
        )
    };

    Ok(format!(
        "// Generated by mlprobe. Do not edit; it is rewritten on every run.\n\
         #![allow(unused_mut, unused_variables)]\n\
         type HModule = *mut core::ffi::c_void;\n\
         extern \"system\" {{\n\
         \x20   fn LoadLibraryA(path: *const u8) -> HModule;\n\
         \x20   fn GetProcAddress(m: HModule, name: *const u8) -> *const core::ffi::c_void;\n\
         }}\n\
         fn main() {{\n\
         \x20   let m = unsafe {{ LoadLibraryA({dll}.as_ptr()) }};\n\
         \x20   if m.is_null() {{ eprintln!(\"mlprobe: LoadLibrary failed\"); std::process::exit(2); }}\n\
         \x20   // The ABI gate every host in this repository performs before it calls\n\
         \x20   // anything (D18). Without it the transmute below is taken on faith, and a\n\
         \x20   // module built by a different compiler than the one that read the IR would\n\
         \x20   // be undefined behaviour rather than a reported mismatch. It does NOT check\n\
         \x20   // the SIGNATURE — nothing across a C ABI can — so it closes the identity\n\
         \x20   // half of that risk and no more.\n\
         \x20   let vp = unsafe {{ GetProcAddress(m, b\"ml_module_abi_version\\0\".as_ptr()) }};\n\
         \x20   if vp.is_null() {{ eprintln!(\"mlprobe: no ml_module_abi_version\"); std::process::exit(2); }}\n\
         \x20   let ver: extern \"C\" fn() -> u32 = unsafe {{ core::mem::transmute(vp) }};\n\
         \x20   if ver() != {abi_version} {{\n\
         \x20       eprintln!(\"mlprobe: module ABI {{}} != compiler {abi_version}\", ver());\n\
         \x20       std::process::exit(2);\n\
         \x20   }}\n\
         \x20   // …and the interface fingerprint, COMPARED rather than merely resolved. The\n\
         \x20   // first version only checked the symbol existed, which proves nothing about\n\
         \x20   // WHICH module answered. This is the check every generated header tells a\n\
         \x20   // host to make, and it is what turns a stale `.dll` left on disk from an\n\
         \x20   // undefined call into a reported mismatch.\n\
         \x20   let hp = unsafe {{ GetProcAddress(m, {hash_lit}.as_ptr()) }};\n\
         \x20   if hp.is_null() {{ eprintln!(\"mlprobe: no {hash_sym}\"); std::process::exit(2); }}\n\
         \x20   let iface: extern \"C\" fn() -> u64 = unsafe {{ core::mem::transmute(hp) }};\n\
         \x20   if iface() != {iface_hash}u64 {{\n\
         \x20       eprintln!(\"mlprobe: interface {{:#X}} != the source read here {iface_hash:#X}\", iface());\n\
         \x20       std::process::exit(2);\n\
         \x20   }}\n\
         \x20   let p = unsafe {{ GetProcAddress(m, {sym_lit}.as_ptr()) }};\n\
         \x20   if p.is_null() {{ eprintln!(\"mlprobe: {sym} not found\"); std::process::exit(2); }}\n\
         \x20   let f: extern \"C\" fn({types}) -> {ret_rust} = unsafe {{ core::mem::transmute(p) }};\n\
         {setup}{head}{report}{tail}}}\n",
        dll = byte_string_nul(&dll.to_string_lossy()),
        sym_lit = byte_string_nul(&sym),
        abi_version = mlc::abi::ML_MODULE_ABI_VERSION,
        hash_sym = hash_sym,
        hash_lit = byte_string_nul(&hash_sym),
        iface_hash = iface_hash,
        types = types.join(", "),
        report = report.concat(),
    ))
}

fn zero_of(t: &IrType) -> &'static str {
    match t {
        IrType::F64 => "0.0",
        IrType::I32 => "0",
        IrType::Bool => "false",
        IrType::Str | IrType::Array(_) => "0",
    }
}

/// A Rust byte-string literal for arbitrary text — escaped rather than interpolated, because
/// a Windows path is full of backslashes and an argument is whatever the user typed.
fn byte_string(s: &str) -> String {
    let mut out = String::from("b\"");
    for b in s.as_bytes() {
        match b {
            b'"' => out.push_str("\\\""),
            b'\\' => out.push_str("\\\\"),
            0x20..=0x7e => out.push(*b as char),
            other => out.push_str(&format!("\\x{other:02x}")),
        }
    }
    out.push('"');
    out
}

fn byte_string_nul(s: &str) -> String {
    let mut lit = byte_string(s);
    lit.truncate(lit.len() - 1);
    lit.push_str("\\0\"");
    lit
}

fn parse_list(a: &str, e: &IrArrayElem) -> Result<String, String> {
    if a == "-" || a.is_empty() {
        return Ok(String::new());
    }
    let mut items = Vec::new();
    for part in a.split(',') {
        let p = part.trim();
        items.push(match e {
            IrArrayElem::F64 => format!(
                "{:?}f64",
                p.parse::<f64>()
                    .map_err(|_| format!("'{p}' is not an f64"))?
            ),
            IrArrayElem::I32 => format!(
                "{}i32",
                p.parse::<i32>()
                    .map_err(|_| format!("'{p}' is not an i32"))?
            ),
            IrArrayElem::Bool => p
                .parse::<bool>()
                .map_err(|_| format!("'{p}' is not a bool"))?
                .to_string(),
        });
    }
    Ok(items.join(", "))
}

//! `mlc` — the Mathless compiler CLI (Phase 1). Thin argv wrapper over the library
//! (`mlc::emit::emit_artifacts`); all real work lives in the crate so it stays unit-tested.
//!
//! Usage:
//!   mlc build <file.mls> [-o <out_dir>]
//!
//! `build` packages the module into four files in `<out_dir>` (default: current dir):
//!   <name>.dll   native C-ABI module
//!   <name>.h     C header       (verified against a real MSVC C host — acceptance D)
//!   <name>.pas   Delphi import unit (gated by MATHLESS_GATE_DELPHI, which does not run in CI)
//!   <name>.lib   MSVC import library, for a host that links instead of GetProcAddress
//! where `<name>` is the input file's stem.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use mlc::emit::emit_artifacts;

/// Why `run` stopped. Only the first kind is about how `mlc` was invoked, so only it earns the
/// usage line. It used to follow EVERY error — a type error, an unreadable input, and the one
/// message that tells the user to move files back by hand — answering a question nobody asked,
/// and in the last case sitting below an instruction the user has to act on.
enum Failure {
    /// The command line itself was wrong.
    Invocation(String),
    /// The command line was fine; reading, compiling or writing failed.
    Build(String),
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match run(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(Failure::Invocation(msg)) => {
            eprintln!("mlc: {msg}");
            eprintln!("usage: mlc build <file.mls> [-o <out_dir>]");
            ExitCode::FAILURE
        }
        Err(Failure::Build(msg)) => {
            eprintln!("mlc: {msg}");
            ExitCode::FAILURE
        }
    }
}

fn run(args: &[String]) -> Result<(), Failure> {
    let usage = |msg: &str| Failure::Invocation(msg.to_string());
    let mut it = args.iter();
    match it.next().map(String::as_str) {
        Some("build") => {}
        Some(other) => return Err(usage(&format!("unknown command '{other}'"))),
        None => return Err(usage("no command given")),
    }

    let mut input: Option<PathBuf> = None;
    let mut out_dir: Option<PathBuf> = None;
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "-o" | "--out" => {
                let dir = it.next().ok_or_else(|| usage("`-o` needs a directory"))?;
                out_dir = Some(PathBuf::from(dir));
            }
            flag if flag.starts_with('-') => {
                return Err(usage(&format!("unknown flag '{flag}'")));
            }
            positional => {
                if input.is_some() {
                    return Err(usage("more than one input file given"));
                }
                input = Some(PathBuf::from(positional));
            }
        }
    }

    let input = input.ok_or_else(|| usage("no input .mls file given"))?;
    let out_dir = out_dir.unwrap_or_else(|| PathBuf::from("."));
    let module = module_name(&input).map_err(Failure::Build)?;

    let src = std::fs::read_to_string(&input)
        .map_err(|e| Failure::Build(format!("cannot read {}: {e}", input.display())))?;
    let arts = emit_artifacts(&src, &module, &out_dir).map_err(|e| match e {
        // The library doesn't know the name came from a filename; say where to fix it.
        mlc::emit::EmitError::InvalidModuleName(msg) => format!(
            "{msg}\n       the module name is the input file's stem — rename {}",
            input.display()
        ),
        // Enumerated rather than `other =>`, under the crate's `wildcard_enum_match_arm` deny.
        // These three carry everything the user needs in the library's own `Display`; the one
        // above does not, because only the CLI knows the module name came from a filename.
        // A future variant has to answer the same question here instead of quietly taking the
        // bare message — which is exactly the kind of "silence reads as a decision" this deny
        // exists to end.
        e @ (mlc::emit::EmitError::Compile(_)
        | mlc::emit::EmitError::Io { .. }
        | mlc::emit::EmitError::RollbackIncomplete { .. }) => e.to_string(),
    });
    let arts = arts.map_err(Failure::Build)?;

    println!("mlc: wrote");
    println!("  {}", arts.dll.display());
    println!(
        "  {}  (C header — verified against a real MSVC C host, acceptance D)",
        arts.header.display()
    );
    println!(
        "  {}  (Delphi: checked by MATHLESS_GATE_DELPHI, which does not run in CI)",
        arts.delphi_unit.display()
    );
    println!(
        "  {}  (MSVC import library — link against it instead of GetProcAddress)",
        arts.import_lib.display()
    );
    Ok(())
}

/// The module name is the input file's stem (`examples/discount.mls` → `discount`).
fn module_name(input: &Path) -> Result<String, String> {
    input
        .file_stem()
        .and_then(|s| s.to_str())
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .ok_or_else(|| format!("cannot derive module name from {}", input.display()))
}

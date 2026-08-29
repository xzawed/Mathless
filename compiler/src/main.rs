//! `mlc` — the Mathless compiler CLI (Phase 1). Thin argv wrapper over the library
//! (`mlc::emit::emit_artifacts`); all real work lives in the crate so it stays unit-tested.
//!
//! Usage:
//!   mlc build <file.mls> [-o <out_dir>]
//!
//! `build` packages the module into three files in `<out_dir>` (default: current dir):
//!   <name>.dll   native C-ABI module
//!   <name>.h     C header       (verified against a real MSVC C host — acceptance D)
//!   <name>.pas   Delphi import unit (DRAFT — same)
//! where `<name>` is the input file's stem.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use mlc::emit::emit_artifacts;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match run(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(msg) => {
            eprintln!("mlc: {msg}");
            eprintln!("usage: mlc build <file.mls> [-o <out_dir>]");
            ExitCode::FAILURE
        }
    }
}

fn run(args: &[String]) -> Result<(), String> {
    let mut it = args.iter();
    match it.next().map(String::as_str) {
        Some("build") => {}
        Some(other) => return Err(format!("unknown command '{other}'")),
        None => return Err("no command given".into()),
    }

    let mut input: Option<PathBuf> = None;
    let mut out_dir: Option<PathBuf> = None;
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "-o" | "--out" => {
                let dir = it.next().ok_or("`-o` needs a directory")?;
                out_dir = Some(PathBuf::from(dir));
            }
            flag if flag.starts_with('-') => return Err(format!("unknown flag '{flag}'")),
            positional => {
                if input.is_some() {
                    return Err("more than one input file given".into());
                }
                input = Some(PathBuf::from(positional));
            }
        }
    }

    let input = input.ok_or("no input .mls file given")?;
    let out_dir = out_dir.unwrap_or_else(|| PathBuf::from("."));
    let module = module_name(&input)?;

    let src = std::fs::read_to_string(&input)
        .map_err(|e| format!("cannot read {}: {e}", input.display()))?;
    let arts = emit_artifacts(&src, &module, &out_dir).map_err(|e| match e {
        // The library doesn't know the name came from a filename; say where to fix it.
        mlc::emit::EmitError::InvalidModuleName(msg) => format!(
            "{msg}\n       the module name is the input file's stem — rename {}",
            input.display()
        ),
        other => other.to_string(),
    })?;

    println!("mlc: wrote");
    println!("  {}", arts.dll.display());
    println!(
        "  {}  (C header — verified against a real MSVC C host, acceptance D)",
        arts.header.display()
    );
    println!(
        "  {}  (DRAFT: Delphi host-load not verified — D14 gate BLOCKED)",
        arts.delphi_unit.display()
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

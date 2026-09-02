//! **Acceptance D** (`docs/phase1/SPEC.md` §3-D) — the gate that was BLOCKED for all of
//! Phase 1: load the same `mlc`-produced `.dll` from a *real C host*, not from our own Rust
//! oracle, and call it over the plain C ABI.
//!
//! The host is `hosts/c-host/host.c`, compiled here with MSVC `cl` (D22 target, toolchain
//! chosen 2026-08-29). It includes the generated `.h` files and derives its function-pointer
//! types from their declarations, so a change in a generated signature breaks the build.
//!
//! **Skipping must never read as passing.** With no MSVC on the machine this test prints
//! `GATE_D_SKIPPED` and returns — but if `MATHLESS_GATE_D=require` is set (CI does set it)
//! a missing toolchain is a *failure*. Only a run that prints `GATE_D_OK` closes the gate.
#![cfg(windows)]

use std::path::{Path, PathBuf};
use std::process::Command;

use ml_oracle::pe;

mod common;
use mlc::emit::emit_artifacts;

/// `vcvars64.bat` for the newest installed MSVC, or `None` if MSVC isn't installed.
fn vcvars64() -> Option<PathBuf> {
    let program_files_x86 =
        std::env::var("ProgramFiles(x86)").unwrap_or_else(|_| r"C:\Program Files (x86)".into());
    let vswhere = Path::new(&program_files_x86)
        .join("Microsoft Visual Studio")
        .join("Installer")
        .join("vswhere.exe");
    if !vswhere.exists() {
        return None;
    }
    let out = Command::new(&vswhere)
        .args([
            "-latest",
            "-products",
            "*",
            "-requires",
            "Microsoft.VisualStudio.Component.VC.Tools.x86.x64",
            "-property",
            "installationPath",
        ])
        .output()
        .ok()?;
    let root = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if root.is_empty() {
        return None;
    }
    let bat = Path::new(&root)
        .join("VC")
        .join("Auxiliary")
        .join("Build")
        .join("vcvars64.bat");
    bat.exists().then_some(bat)
}

/// Run `body` inside a `vcvars64` environment. Everything MSVC needs (INCLUDE, LIB, PATH)
/// comes from that script, so we drive it through a one-shot batch file rather than trying
/// to reproduce the environment ourselves. `VSLANG=1033` keeps tool output English so the
/// parsing below doesn't depend on the machine's locale.
fn run_in_msvc_env(vcvars: &Path, workdir: &Path, body: &str) -> std::process::Output {
    let bat = workdir.join("run.bat");
    std::fs::write(
        &bat,
        format!(
            "@echo off\r\nset VSLANG=1033\r\ncall \"{}\" >nul 2>&1\r\nif errorlevel 1 exit /b 90\r\ncd /d \"{}\"\r\n{body}\r\n",
            vcvars.display(),
            workdir.display()
        ),
    )
    .expect("write run.bat");
    Command::new("cmd")
        .args(["/c".as_ref(), bat.as_os_str()])
        .output()
        .expect("spawn cmd")
}

/// Export names from `dumpbin /exports`, parsed without depending on the header wording:
/// an export row is `<ordinal> <hint> <rva> <name>`.
fn dumpbin_exports(vcvars: &Path, workdir: &Path, dll: &Path) -> Vec<String> {
    let out = run_in_msvc_env(
        vcvars,
        workdir,
        &format!("dumpbin /nologo /exports \"{}\"", dll.display()),
    );
    assert!(
        out.status.success(),
        "dumpbin failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let text = String::from_utf8_lossy(&out.stdout);
    let mut names: Vec<String> = text
        .lines()
        .filter_map(|line| {
            let f: Vec<&str> = line.split_whitespace().collect();
            if f.len() == 4 && f[0].parse::<u32>().is_ok() && u32::from_str_radix(f[2], 16).is_ok()
            {
                Some(f[3].to_string())
            } else {
                None
            }
        })
        .collect();
    names.sort();
    names
}

/// The same second opinion for the import table, in the `"dll!function"` form our reader uses.
///
/// `dumpbin /imports` prints one indented DLL name, then its functions as `<hint> <name>`
/// pairs (or `Ordinal <n>` when there is no name). The `Summary` section at the end also has
/// two-field lines (`1000 .rdata`), and its first field parses as hex — so parsing stops there
/// rather than inventing imports named after sections.
fn dumpbin_imports(vcvars: &Path, workdir: &Path, dll: &Path) -> Vec<String> {
    let out = run_in_msvc_env(
        vcvars,
        workdir,
        &format!("dumpbin /nologo /imports \"{}\"", dll.display()),
    );
    assert!(
        out.status.success(),
        "dumpbin /imports failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let text = String::from_utf8_lossy(&out.stdout);
    let mut names: Vec<String> = Vec::new();
    let mut current: Option<String> = None;
    for line in text.lines() {
        let t = line.trim();
        if t == "Summary" {
            break;
        }
        if t.to_ascii_lowercase().ends_with(".dll") && !t.contains(char::is_whitespace) {
            current = Some(t.to_ascii_lowercase());
            continue;
        }
        let Some(dll_name) = current.as_deref() else {
            continue;
        };
        let f: Vec<&str> = t.split_whitespace().collect();
        if f.len() != 2 {
            continue;
        }
        if f[0] == "Ordinal" {
            if let Ok(n) = f[1].parse::<u32>() {
                names.push(format!("{dll_name}!#{n}"));
            }
        } else if u32::from_str_radix(f[0], 16).is_ok() {
            names.push(format!("{dll_name}!{}", f[1]));
        }
    }
    names.sort();
    names
}

#[test]
fn a_real_c_host_loads_and_calls_the_module() {
    let Some(vcvars) = vcvars64() else {
        if std::env::var("MATHLESS_GATE_D").as_deref() == Ok("require") {
            panic!(
                "MATHLESS_GATE_D=require but MSVC was not found — acceptance D cannot be \
                 verified. Install the VS Build Tools with the C++ workload, or unset the var."
            );
        }
        // Deliberately loud: a skipped gate is NOT a passed gate.
        println!(
            "GATE_D_SKIPPED: no MSVC toolchain found (vswhere/vcvars64 absent). \
             Acceptance D is NOT verified by this run."
        );
        return;
    };

    // A guard, not a bare path: this test builds 14 DLLs and runs a child host process that
    // loads them, so its tree is the biggest one and the most likely to lose the unlock race
    // (measured — it leaked despite removing at the end). `TempOut` retries.
    let work = common::TempOut::new("gate_d");

    // The very same artifacts `mlc build` gives a user.
    let discount = emit_artifacts(
        include_str!("../../../examples/discount.mls"),
        "discount",
        &work,
    )
    .expect("emit discount");
    let safe_div = emit_artifacts(
        include_str!("../../../examples/safe_div.mls"),
        "safe_div",
        &work,
    )
    .expect("emit safe_div");
    let sum_to = emit_artifacts(
        include_str!("../../../examples/sum_to.mls"),
        "sum_to",
        &work,
    )
    .expect("emit sum_to");
    let negate_if = emit_artifacts(
        include_str!("../../../examples/negate_if.mls"),
        "negate_if",
        &work,
    )
    .expect("emit negate_if");
    let count_bounded = emit_artifacts(
        include_str!("../../../examples/count_bounded.mls"),
        "count_bounded",
        &work,
    )
    .expect("emit count_bounded");
    let discount4 = emit_artifacts(
        include_str!("../../../examples/discount4.mls"),
        "discount4",
        &work,
    )
    .expect("emit discount4");
    let pack = emit_artifacts(include_str!("../../../examples/pack.mls"), "pack", &work)
        .expect("emit pack");
    let commission = emit_artifacts(
        include_str!("../../../examples/commission.mls"),
        "commission",
        &work,
    )
    .expect("emit commission");
    let deduction = emit_artifacts(
        include_str!("../../../examples/deduction.mls"),
        "deduction",
        &work,
    )
    .expect("emit deduction");
    let line_total = emit_artifacts(
        include_str!("../../../examples/line_total.mls"),
        "line_total",
        &work,
    )
    .expect("emit line_total");

    let vat =
        emit_artifacts(include_str!("../../../examples/vat.mls"), "vat", &work).expect("emit vat");

    let carrier = emit_artifacts(
        include_str!("../../../examples/carrier.mls"),
        "carrier",
        &work,
    )
    .expect("emit carrier");

    let quote = emit_artifacts(include_str!("../../../examples/quote.mls"), "quote", &work)
        .expect("emit quote");

    // A drifted `pack`: the two parameters of `boxes` are swapped and nothing else changes.
    // Both versions are `int32_t mlx_boxes(int32_t, int32_t)` in C, so this is the drift the
    // ABI cannot see and the host's `_Static_assert` cannot catch — the measured case where
    // `boxes(100, 3)` quietly returned 0 instead of 33. The host must refuse it (WH6).
    let drifted_pack = "\
export fn boxes(per_box: i32, qty: i32) -> i32 { return qty / per_box }
export fn loose(qty: i32, per_box: i32) -> i32 { return qty % per_box }
error E_EMPTY_BOX = 1
export fn boxes_checked(qty: i32, per_box: i32) -> i32! {
  if per_box == 0 { fail E_EMPTY_BOX }
  return qty / per_box
}
";
    let drift = emit_artifacts(drifted_pack, "pack_drift", &work).expect("emit drifted pack");
    assert!(drift.dll.exists());

    let host_c = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("c-host")
        .join("host.c");
    assert!(host_c.exists(), "missing {}", host_c.display());

    // Compile the C host against the GENERATED headers (`/I` the artifact dir).
    let compile = run_in_msvc_env(
        &vcvars,
        &work,
        &format!(
            "cl /nologo /W4 /WX /std:c11 /I\"{}\" \"{}\" /Fe:host.exe /Fo:host.obj",
            work.display(),
            host_c.display()
        ),
    );
    assert!(
        compile.status.success(),
        "cl failed to build the C host against the generated headers:\n{}\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr)
    );

    // Run it: LoadLibrary + GetProcAddress + call, in a process that is not ours. Invoke by
    // absolute path — `NoDefaultCurrentDirectoryInExePath` is set on some machines (it is on
    // this one), so cmd will not find `host.exe` in the working directory.
    let run = run_in_msvc_env(
        &vcvars,
        &work,
        &format!(
            "\"{}\" \"{}\" {} pack_drift.dll",
            work.join("host.exe").display(),
            work.display(),
            mlc::ML_MODULE_ABI_VERSION
        ),
    );
    let stdout = String::from_utf8_lossy(&run.stdout);
    // Print it: this transcript IS the acceptance-D evidence (`cargo test -- --nocapture`).
    println!("{stdout}");
    assert!(
        run.status.success() && stdout.contains("GATE_D_OK"),
        "the C host did not pass:\n{stdout}\n{}",
        String::from_utf8_lossy(&run.stderr)
    );

    // Cross-check our own PE reader against Microsoft's dumpbin on the same file: until now
    // the export measurement (acceptance C) had exactly one implementation — ours.
    for dll in [
        &discount.dll,
        &safe_div.dll,
        &sum_to.dll,
        &negate_if.dll,
        &count_bounded.dll,
        &discount4.dll,
        &line_total.dll,
        &pack.dll,
        &commission.dll,
        &deduction.dll,
        &vat.dll,
        &carrier.dll,
        &quote.dll,
    ] {
        let mut ours = pe::read_exports(dll).expect("our PE reader");
        ours.sort();
        let theirs = dumpbin_exports(&vcvars, &work, dll);
        assert_eq!(
            ours,
            theirs,
            "our PE reader and dumpbin disagree about {}",
            dll.display()
        );

        // The IMPORT reader is new (SPEC-string-input section 3-C measures "the import set is
        // unchanged"), so give it the same second opinion the export reader gets. Without this
        // a reader that silently returned an empty set would make every import assertion pass.
        let ours = pe::read_imports(dll).expect("our PE import reader");
        let theirs = dumpbin_imports(&vcvars, &work, dll);
        assert_eq!(
            ours,
            theirs,
            "our PE reader and dumpbin disagree about the imports of {}",
            dll.display()
        );
        assert!(!ours.is_empty(), "an empty import set would be suspicious");
    }
}

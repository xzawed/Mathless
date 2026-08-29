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

    let work = std::env::temp_dir().join(format!("mlc_gate_d_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&work);
    std::fs::create_dir_all(&work).unwrap();

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
            "\"{}\" \"{}\" {}",
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
    for dll in [&discount.dll, &safe_div.dll, &sum_to.dll, &negate_if.dll] {
        let mut ours = pe::read_exports(dll).expect("our PE reader");
        ours.sort();
        let theirs = dumpbin_exports(&vcvars, &work, dll);
        assert_eq!(
            ours,
            theirs,
            "our PE reader and dumpbin disagree about {}",
            dll.display()
        );
    }

    let _ = std::fs::remove_dir_all(&work);
}

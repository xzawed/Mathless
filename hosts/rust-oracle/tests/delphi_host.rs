//! The Delphi half of D14 — staged, and skipped until a compiler exists.
//!
//! **Nothing here has ever run.** `dcc64` is absent from this machine (measured: not on
//! PATH, not on disk, and the registry's `Embarcadero\Studio\15.0` is the leftover of a
//! removed install). So this test looks for a compiler, says loudly that it did not find
//! one, and returns — the same skip-gate shape `c_host.rs` used while acceptance D was
//! blocked, and for the same reason: **a skipped gate is not a passed gate.**
//!
//! Set `MATHLESS_GATE_DELPHI=require` to turn a missing compiler into a failure. CI does
//! NOT set it, because requiring a toolchain nobody has would just paint the build red.
//! The day `dcc64` is installed, that variable is the whole switch.
//!
//! What it will prove when it runs: that the generated `.pas` units are valid Object
//! Pascal and their declarations are right. `hosts/c-host` cannot speak to that — it
//! compiles the `.h`. Until then the generated `.pas` stays DRAFT and every document says
//! so (D21, STATUS §1).
#![cfg(windows)]

use std::path::{Path, PathBuf};
use std::process::Command;

use mlc::emit::emit_artifacts;

mod common;

/// Find `dcc64`: PATH first, then the usual Embarcadero layout. `None` if absent.
fn dcc64() -> Option<PathBuf> {
    if let Ok(out) = Command::new("where").arg("dcc64").output() {
        if out.status.success() {
            if let Some(first) = String::from_utf8_lossy(&out.stdout).lines().next() {
                let p = PathBuf::from(first.trim());
                if p.is_file() {
                    return Some(p);
                }
            }
        }
    }
    // The default install puts it under `<Studio>\<ver>\bin\dcc64.exe`. Probing a couple of
    // roots is cheaper than a registry crawl and does not need a dependency.
    for root in [
        r"C:\Program Files (x86)\Embarcadero\Studio",
        r"C:\Program Files\Embarcadero\Studio",
    ] {
        let Ok(entries) = std::fs::read_dir(root) else {
            continue;
        };
        for e in entries.flatten() {
            let candidate = e.path().join("bin").join("dcc64.exe");
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    None
}

#[test]
fn a_real_delphi_host_loads_and_calls_the_module() {
    let Some(dcc) = dcc64() else {
        if std::env::var("MATHLESS_GATE_DELPHI").as_deref() == Ok("require") {
            panic!(
                "MATHLESS_GATE_DELPHI=require but dcc64 was not found — the Delphi half of \
                 D14 cannot be verified. Install RAD Studio / Delphi with the Win64 \
                 compiler, or unset the variable."
            );
        }
        // Deliberately loud, and deliberately not a pass.
        println!(
            "GATE_DELPHI_SKIPPED: no dcc64 found. The generated .pas is STILL unverified — \
             D14's Delphi arm remains open, and hosts/delphi-host/host.dpr has never been \
             compiled."
        );
        return;
    };

    let work = common::TempOut::new("gate_delphi");

    // The same artifacts `mlc build` gives a user — the units this host `uses`.
    for (src, name) in [
        (
            include_str!("../../../examples/discount.mls") as &str,
            "discount",
        ),
        (include_str!("../../../examples/safe_div.mls"), "safe_div"),
        (include_str!("../../../examples/carrier.mls"), "carrier"),
    ] {
        emit_artifacts(src, name, &work).unwrap_or_else(|e| panic!("emit {name}: {e}"));
    }

    let host_dpr = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("delphi-host")
        .join("host.dpr");
    assert!(host_dpr.exists(), "missing {}", host_dpr.display());

    // `-U<dir>` puts the generated units on the unit search path; `-E<dir>` sends the exe
    // next to the DLLs, which the implicit imports need at load time.
    let compile = Command::new(&dcc)
        .arg(format!("-U{}", work.path().display()))
        .arg(format!("-E{}", work.path().display()))
        .arg(format!("-N{}", work.path().display()))
        .arg(&host_dpr)
        .current_dir(work.path())
        .output()
        .expect("run dcc64");
    assert!(
        compile.status.success(),
        "dcc64 failed to build the Delphi host against the generated units:\n{}\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr)
    );

    let exe = work.path().join("host.exe");
    let run = Command::new(&exe)
        .arg(mlc::ML_MODULE_ABI_VERSION.to_string())
        .current_dir(work.path())
        .output()
        .expect("run the Delphi host");
    let stdout = String::from_utf8_lossy(&run.stdout);
    // This transcript IS the evidence, exactly as acceptance D's is.
    println!("{stdout}");
    assert!(
        run.status.success() && stdout.contains("GATE_DELPHI_OK"),
        "the Delphi host did not pass:\n{stdout}\n{}",
        String::from_utf8_lossy(&run.stderr)
    );
}

#[test]
fn the_staged_host_exists_and_says_it_is_unverified() {
    // The one thing that CAN be checked without a compiler: that the staged file is here and
    // does not pretend to be evidence. A draft that stops calling itself a draft is how a
    // "verified" claim gets made by accident.
    let host_dpr = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("delphi-host")
        .join("host.dpr");
    let text = std::fs::read_to_string(&host_dpr).expect("staged Delphi host");
    assert!(
        text.contains("NEVER BEEN COMPILED"),
        "the staged host must keep saying it is unverified: {}",
        host_dpr.display()
    );
    assert!(
        text.contains("GATE_DELPHI_OK"),
        "the success marker is missing"
    );
}

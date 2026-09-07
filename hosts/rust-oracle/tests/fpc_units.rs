//! Are the generated `.pas` units valid Object Pascal at all?
//!
//! **This is NOT the Delphi gate.** D14's Delphi arm needs `dcc64`, stays open, and the
//! generated unit stays DRAFT — see `delphi_host.rs`, which is the test that closes it. This
//! one asks a strictly smaller question, the same one the C++ gate asks of the header: does a
//! compiler accept the text? Nothing here loads a module, calls a function, or observes the
//! ABI, and a green run says nothing about Delphi.
//!
//! It earns its place the way the C++ gate did. Until it existed, the golden file pinned 535
//! lines of Object Pascal that **no compiler of any kind had ever read** — every assertion
//! about the unit was a string comparison against text the repo wrote itself.
//!
//! It found something on the first run: 11 of the 19 example units warned. Free Pascal types
//! a bare `$8120E9C099B13F94` as a signed Int64 before assigning it to `UInt64`, so every
//! fingerprint with the top bit set tripped "range check error while evaluating constants".
//! The stored value was right either way (measured), but a consumer building with
//! warnings-as-errors could not build — which is the standard this repo holds its own C
//! header to. `header.rs` now emits `UInt64($...)`, and `-Sew` below keeps it that way.
#![cfg(windows)]

use std::path::PathBuf;
use std::process::Command;

use mlc::emit::emit_artifacts;

mod common;

/// Find `fpc`: PATH first, then SEARCH a couple of install roots rather than enumerating a
/// layout.
///
/// The enumerating version is what CI rejected, and it is the shape #172 already fixed once
/// in this repository: it listed `C:\FPC\<version>\bin\i386-win32\fpc.exe`, which is where the
/// official installer puts it and where winget put it here. Chocolatey deploys to
/// `C:\tools\freepascal\`, so the gate reported "no fpc found" on a runner that had just
/// installed one. Listing the layouts means adding a third the next time a packager differs.
///
/// The driver is a 32-bit binary even for an x64 job -- it reaches x86_64 through
/// `ppcrossx64` -- so the search must not filter on the host-arch directory name either.
fn fpc() -> Option<PathBuf> {
    if let Ok(out) = Command::new("where").arg("fpc").output() {
        if out.status.success() {
            if let Some(first) = String::from_utf8_lossy(&out.stdout).lines().next() {
                let p = PathBuf::from(first.trim());
                if p.is_file() {
                    return Some(p);
                }
            }
        }
    }
    for root in [r"C:\FPC", r"C:\tools\freepascal", r"C:\tools\FPC"] {
        if let Some(found) = find_fpc_under(std::path::Path::new(root), 4) {
            return Some(found);
        }
    }
    None
}

/// Depth-limited hunt for a `bin/.../fpc.exe`. Bounded because an unbounded walk of a wrong
/// root is a slow way to answer "absent".
fn find_fpc_under(dir: &std::path::Path, depth: usize) -> Option<PathBuf> {
    if depth == 0 {
        return None;
    }
    let entries = std::fs::read_dir(dir).ok()?;
    let mut dirs = Vec::new();
    for e in entries.flatten() {
        let path = e.path();
        if path.is_file() {
            if path.file_name().and_then(|n| n.to_str()) == Some("fpc.exe") {
                return Some(path);
            }
        } else if path.is_dir() {
            dirs.push(path);
        }
    }
    for d in dirs {
        if let Some(found) = find_fpc_under(&d, depth - 1) {
            return Some(found);
        }
    }
    None
}

#[test]
fn every_generated_unit_is_valid_object_pascal() {
    let Some(fpc) = fpc() else {
        if std::env::var("MATHLESS_GATE_FPC").as_deref() == Ok("require") {
            panic!(
                "MATHLESS_GATE_FPC=require but fpc was not found — the generated .pas units \
                 cannot be compiled by anything. Install Free Pascal 3.2.2 (winget install \
                 FreePascal.FreePascalCompiler), or unset the variable."
            );
        }
        // Loud, and not a pass. A skipped gate is not a passed gate.
        println!(
            "GATE_FPC_SKIPPED: no fpc found. The generated .pas is compiled by NOTHING in \
             this run — every assertion about it is a string comparison against text this \
             repo wrote itself."
        );
        return;
    };

    // Which target? The units import an x64 module, so x86_64 is the closer match and this
    // asks for it first. But the claim this gate makes is about the TEXT -- syntax, `cdecl`,
    // `external`, `UInt64`, `PAnsiChar` -- and none of those declarations depend on pointer
    // size, so a driver without the x64 cross backend still answers the question it is here
    // to answer. Chocolatey's package fetches the i386-only installer, so that case is real.
    //
    // It falls back, and it SAYS which it used. A gate that quietly answers a smaller
    // question is the shape this repository keeps removing.
    let mut probe = Command::new(&fpc);
    probe.arg("-Px86_64").arg("-iTP");
    let probe_out = common::output_with_deadline(
        probe,
        std::time::Duration::from_secs(60),
        "fpc -Px86_64 -iTP",
    );
    let x64_ok =
        probe_out.status.success() && String::from_utf8_lossy(&probe_out.stdout).trim() == "x86_64";
    let target = if x64_ok { Some("x86_64") } else { None };

    let work = common::TempOut::new("gate_fpc");
    let units_dir = work.path().join("units");
    std::fs::create_dir_all(&units_dir).expect("create unit output dir");

    // Enumerate `examples/` rather than listing names: a new example that does not compile
    // must break this, and a hardcoded list would let it through (the lesson of A5).
    let examples = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("examples");
    let mut compiled = 0usize;
    let mut failures = Vec::new();

    for entry in std::fs::read_dir(&examples)
        .expect("read examples/")
        .flatten()
    {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("mls") {
            continue;
        }
        let stem = path.file_stem().unwrap().to_string_lossy().to_string();
        let src = std::fs::read_to_string(&path).expect("read example");
        let arts =
            emit_artifacts(&src, &stem, work.path()).unwrap_or_else(|e| panic!("emit {stem}: {e}"));

        // `-Mdelphi` is the dialect the unit is written for. `-Sew` turns warnings into
        // errors: the defect this gate found was a warning, and a warning nobody fails on is
        // a warning that ships. `-Px86_64` matches the module the unit imports.
        let mut cmd = Command::new(&fpc);
        if let Some(t) = target {
            cmd.arg(format!("-P{t}"));
        }
        cmd.arg("-Mdelphi")
            .arg("-Sew")
            .arg(format!("-FU{}", units_dir.display()))
            .arg(&arts.delphi_unit);
        let out = common::output_with_deadline(
            cmd,
            std::time::Duration::from_secs(120),
            &format!("fpc on {stem}"),
        );

        if out.status.success() {
            compiled += 1;
        } else {
            failures.push(format!(
                "--- {stem} ---\n{}{}",
                String::from_utf8_lossy(&out.stdout),
                String::from_utf8_lossy(&out.stderr)
            ));
        }
    }

    assert!(
        failures.is_empty(),
        "generated units are not valid Free Pascal under -Mdelphi -Sew:\n{}",
        failures.join("\n")
    );

    let example_count = std::fs::read_dir(&examples)
        .expect("read examples/")
        .flatten()
        .filter(|e| e.path().extension().and_then(|x| x.to_str()) == Some("mls"))
        .count();
    assert_eq!(
        compiled, example_count,
        "every example must contribute a unit that compiles; the floor is derived from \
         examples/ so adding one cannot loosen it"
    );
    assert!(
        compiled > 0,
        "the corpus is empty, so this gate proved nothing"
    );

    let shown = target.unwrap_or("the driver's default (no x86_64 backend here)");
    println!(
        "GATE_FPC_OK: {compiled} generated units compiled with -Mdelphi -Sew, target {shown},          using {}",
        fpc.display()
    );
}

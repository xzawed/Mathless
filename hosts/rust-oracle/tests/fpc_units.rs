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

/// Build and RUN `hosts/delphi-host/host.dpr` with Free Pascal, against real modules.
///
/// **This is not the Delphi gate and must never be read as one.** D14 names `dcc64`;
/// `-Mdelphi` is a dialect emulation, and the Embarcadero-only hazard the generated units
/// warn about — a `UnicodeString` passed where `PAnsiChar` is expected, which compiles and
/// silently matches nothing — cannot be shown here at all. `delphi_host.rs` is the test that
/// closes X1, and it still cannot run.
///
/// What this one adds is what nothing else could: an Object Pascal host that actually LOADS
/// the modules and CALLS them. The `.pas` compile check above proves the text parses; this
/// proves the declarations describe the module that ships. The two are different questions,
/// and the second found a defect the first could not.
///
/// The defect, measured the first time the file was ever compiled (2026-09-07): every
/// generated unit declares the same two reserved functions, so an UNQUALIFIED
/// `ml_iface_hash` binds to whichever unit came last in the `uses` clause. The gate compared
/// carrier's fingerprint against `ML_DISCOUNT_IFACE_HASH` and could never pass —
/// `C8D1191E339AAF6E` against `05697A6FAFD68344`. `ml_module_abi_version` had the same bug
/// and hid it, because every module answers 1. The C host cannot meet this: it resolves per
/// module handle. It is a hazard of the import-unit binding, and it took a compiler to see.
///
/// **Its own variable, `MATHLESS_GATE_FPC_HOST`, and CI does not set it.** Unlike the units
/// above, this host LOADS x64 modules, so it needs an fpc that can target x86_64.
/// Chocolatey's package -- what the windows job installs -- fetches the i386-only installer;
/// measured on CI: `GATE_FPC_OK ... target the driver's default (no x86_64 backend here)`.
/// Requiring it there would paint the build red over a runner's packaging, so it requires
/// nothing and skips loudly instead. It still RUNS wherever the backend exists, which is
/// every full local run here (the official win32.and.win64 installer carries ppcrossx64,
/// and that is the one winget installs).
///
/// Closing that gap means putting an x86_64-capable Free Pascal on the runner. That is not
/// done, and this is where a person finds out why instead of guessing.
///
/// **The split turns out to buy something nobody designed for.** The units test above runs
/// in BOTH places and takes whatever target the local driver offers, so between them the
/// generated `.pas` is compiled at two pointer widths on every push -- i386 on CI
/// (chocolatey's package) and x86_64 here (winget's). Both lines are from real runs:
///
///     CI     GATE_FPC_OK: 19 ... target the driver's default (no x86_64 backend here)
///     local  GATE_FPC_OK: 19 ... target x86_64
///
/// That is luck, not design. It is written down so a later change which makes both
/// environments agree knows what it would be giving up.
#[test]
fn the_staged_pascal_host_builds_and_calls_the_modules() {
    let Some(fpc) = fpc() else {
        if std::env::var("MATHLESS_GATE_FPC_HOST").as_deref() == Ok("require") {
            panic!(
                "MATHLESS_GATE_FPC_HOST=require but fpc was not found — the staged Pascal host \
                 cannot be built by anything."
            );
        }
        println!(
            "GATE_FPC_HOST_SKIPPED: no fpc found. hosts/delphi-host/host.dpr is compiled by \
             NOTHING in this run."
        );
        return;
    };

    let work = common::TempOut::new("gate_fpc_host");

    // The units this host `uses`, read from the host itself rather than listed again
    // here. Two tests build it and each used to keep its own list; they drifted the
    // moment `shapes` was added (see `common::delphi_host_units`).
    let examples = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("examples");
    for unit in common::delphi_host_units() {
        let src =
            std::fs::read_to_string(examples.join(format!("{unit}.mls"))).unwrap_or_else(|e| {
                panic!("host.dpr uses `{unit}`, but examples/{unit}.mls could not be read: {e}")
            });
        emit_artifacts(&src, &unit, work.path()).unwrap_or_else(|e| panic!("emit {unit}: {e}"));
    }

    let host_dpr = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("delphi-host")
        .join("host.dpr");
    assert!(host_dpr.exists(), "missing {}", host_dpr.display());

    let mut cmd = Command::new(&fpc);
    cmd.arg("-Mdelphi")
        .arg(format!("-FU{}", work.path().display()))
        .arg(format!("-FE{}", work.path().display()))
        .arg(&host_dpr)
        .current_dir(work.path());
    // Target x86_64 when the backend exists: unlike the units, this host LOADS x64 DLLs, so
    // a 32-bit build cannot run at all. Without the backend there is nothing to measure.
    let mut probe = Command::new(&fpc);
    probe.arg("-Px86_64").arg("-iTP");
    let probe_out = common::output_with_deadline(
        probe,
        std::time::Duration::from_secs(60),
        "fpc -Px86_64 -iTP",
    );
    if !(probe_out.status.success()
        && String::from_utf8_lossy(&probe_out.stdout).trim() == "x86_64")
    {
        if std::env::var("MATHLESS_GATE_FPC_HOST").as_deref() == Ok("require") {
            panic!(
                "MATHLESS_GATE_FPC_HOST=require but this fpc has no x86_64 backend — the staged \
                 host loads x64 modules, so a 32-bit build could not run them. Install a \
                 Free Pascal that carries ppcrossx64."
            );
        }
        println!(
            "GATE_FPC_HOST_SKIPPED: this fpc cannot target x86_64, and the staged host loads \
             x64 modules. The units still compiled (see the test above); the host did not run."
        );
        return;
    }
    cmd.arg("-Px86_64");

    let build = common::output_with_deadline(cmd, std::time::Duration::from_secs(180), "fpc host");
    let build_out = format!(
        "{}{}",
        String::from_utf8_lossy(&build.stdout),
        String::from_utf8_lossy(&build.stderr)
    );
    let exe = work.path().join("host.exe");
    assert!(
        exe.is_file(),
        "fpc exited {} but produced no {} — an exit code is not proof that anything was \
         built:\n{build_out}",
        build.status,
        exe.display()
    );

    let mut run = Command::new(&exe);
    run.arg(mlc::ML_MODULE_ABI_VERSION.to_string())
        .current_dir(work.path());
    let out = common::output_with_deadline(run, std::time::Duration::from_secs(120), "host.exe");
    let stdout = String::from_utf8_lossy(&out.stdout);
    // The transcript IS the evidence, exactly as acceptance D's is.
    println!("{stdout}");
    assert!(
        out.status.success() && stdout.contains("GATE_DELPHI_OK"),
        "the staged Pascal host did not pass under Free Pascal:\n{stdout}\n{}",
        String::from_utf8_lossy(&out.stderr)
    );

    // ---- the claim host.dpr makes about itself, which nothing was checking ----
    //
    // Its header says the two official hosts differ in a way that matters, and BOTH SIDES
    // are measured (the first version of that paragraph described the C host from memory
    // and was wrong about it). Park carrier.dll and run each:
    //
    //   hosts/c-host   78 lines of checks first, then `FAIL LoadLibraryA(...) -> error
    //                  126`, counts a failure, carries on, exits 1
    //   this host      0 bytes, exit 0xC0000135 (STATUS_DLL_NOT_FOUND)
    //
    // LoadLibrary makes a missing module DATA the C host can report. `external
    // ML_MODULE` is bound when the PROGRAM loads, so this one cannot report anything --
    // the loader refuses first. That is why host.dpr's fingerprint check is written as
    // "refuse to USE" rather than "refuse to load".
    //
    // It was a header comment with no guard. Measured now (2026-09-07): with one module
    // renamed away the host produces ZERO bytes of output and dies with 0xC0000135,
    // STATUS_DLL_NOT_FOUND. The emptiness is the load-bearing part -- it is what "never
    // reaches begin" means -- so that is what is asserted. The status is printed as evidence
    // rather than pinned: it is Windows', not ours, and a future one changing it would break
    // this test for a reason that has nothing to do with the claim.
    //
    // This is an axis only the Pascal host can exercise. No amount of C-host coverage reaches
    // it, because GetProcAddress binds nothing at load time.
    let hidden = work.path().join("carrier.dll");
    let parked = work.path().join("carrier.dll.parked");
    std::fs::rename(&hidden, &parked).expect("park carrier.dll");

    let mut missing = Command::new(&exe);
    missing
        .arg(mlc::ML_MODULE_ABI_VERSION.to_string())
        .current_dir(work.path());
    let dead = common::output_with_deadline(
        missing,
        std::time::Duration::from_secs(120),
        "host.exe with a module removed",
    );
    std::fs::rename(&parked, &hidden).expect("restore carrier.dll");

    let said = String::from_utf8_lossy(&dead.stdout);
    assert!(
        said.is_empty(),
        "the host reached `begin` with a module missing, so the imports are NOT bound at load \
         time and host.dpr's header is wrong about how it differs from the C host. It \
         printed:\n{said}"
    );
    assert!(
        !dead.status.success(),
        "the host exited 0 with a module missing"
    );
    println!(
        "GATE_FPC_HOST_LOADBIND_OK: a missing module killed the host before `begin` (no \
         output, status {})",
        dead.status
    );
}

//! The Delphi half of D14 — gated here, never in CI.
//!
//! **This runs, and it is what verifies the Delphi arm.** `MATHLESS_GATE_DELPHI=require`
//! builds `hosts/delphi-host/host.dpr` against the generated units and calls the modules
//! across the C ABI. The `dcc64` on this machine refuses command-line builds (an EDITION
//! limit, measured), so the gate falls back to driving the IDE with `bds.exe -b`, which the
//! same edition allows — see `bds_exe` below for how that correction came about.
//!
//! Without a Delphi it prints `GATE_DELPHI_SKIPPED` and returns: **a skipped gate is not a
//! passed gate.** CI does not and cannot set `require` — the runner has neither Delphi nor
//! an interactive desktop session, so nothing here protects a push.
//!
//! What it proves: that the generated `.pas` units are valid Object Pascal, that their
//! declarations are right, and that a real Delphi host calls the modules across the C ABI.
//! `hosts/c-host` cannot speak to any of that — it compiles the `.h`. What it does NOT give
//! is CI coverage: the runner has neither Delphi nor an interactive session (STATUS §9-20).
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

/// Can this `dcc64` actually compile from a command line?
///
/// Finding the file is not the precondition; producing an artifact is. Measured on a real
/// install (2026-09-07): a dcc64 whose licence forbids command-line builds prints
/// "This version of the product does not support command line compiling", writes nothing,
/// and **exits 0**. Treating the file as the precondition made the gate fail on a machine
/// that simply cannot run it, and fail two statements later with `NotFound` on host.exe --
/// a true message about the wrong thing.
///
/// So the probe compiles three lines of Pascal and looks for the exe. `Err(reason)` means
/// this toolchain cannot serve the gate, with a reason a person can act on.
///
/// **MSBuild is not a way around it, and that was measured rather than assumed.** Delphi
/// ships MSBuild targets, so driving those instead is the obvious next idea. All four
/// routes give the same line and the same empty output directory: `dcc64` directly,
/// `dcc32` directly, and `msbuild` on a `.dproj` for Win64 and for Win32. The licence
/// check lives in the compiler, not in the way it is invoked -- and MSBuild reports
/// **exit 0** over it too, which is this same trap one layer up.
fn dcc_can_compile(dcc: &Path) -> Result<(), String> {
    let probe = common::TempOut::new("dcc_probe");
    let src = probe.path().join("mlprobe.dpr");
    std::fs::write(&src, "program mlprobe;\nbegin\nend.\n").expect("write probe source");

    let mut cmd = Command::new(dcc);
    cmd.arg(format!("-E{}", probe.path().display()))
        .arg(format!("-N{}", probe.path().display()))
        .arg(&src)
        .current_dir(probe.path());
    let out = common::output_with_deadline(cmd, std::time::Duration::from_secs(120), "dcc64 probe");
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    if probe.path().join("mlprobe.exe").is_file() {
        return Ok(());
    }
    if text.contains("does not support command line compiling") {
        return Err(format!(
            "this dcc64 refuses command-line builds -- an EDITION limit, not a defect here. \
             Community Edition compiles only from the IDE; D14 needs an edition whose dcc64 \
             builds from a command line. It exited {}: {}",
            out.status,
            text.trim()
        ));
    }
    Err(format!(
        "this dcc64 exited {} and produced no probe executable, so it cannot build the \
         gate host either: {}",
        out.status,
        text.trim()
    ))
}

/// Which of the two build paths this run will use.
enum Builder {
    Dcc,
    Bds(PathBuf),
}

/// `bds.exe`, the IDE launcher, next to a `dcc64` that refuses command-line builds.
///
/// **The IDE can build what the command-line compiler will not, and `-b` drives it.**
/// Measured 2026-09-09 on the Community Edition here: `bds.exe -b host.dproj` produced a
/// Win64 `host.exe` in 11-13 seconds, EXITED ON ITS OWN with code 0, and the host then
/// printed GATE_DELPHI_OK. The binary carries `Embarcadero` and `System.SysUtils` markers,
/// so it is dcc64's output reached a different way -- not Free Pascal's.
///
/// That correction matters more than the mechanism. STATUS said the Delphi arm needed a
/// paid edition, and it said so from ONE generalisation: dcc64, dcc32 and msbuild are all
/// blocked (measured, 9-12), therefore automation is impossible (never measured). The IDE's
/// own build path was never tried.
///
/// What it still does NOT give is CI: this needs Delphi installed and an interactive desktop
/// session, and the runner has neither.
fn bds_exe(dcc: &Path) -> Option<PathBuf> {
    // dcc64 lives in <Studio>\<ver>in; so does bds.exe.
    let candidate = dcc.parent()?.join("bds.exe");
    candidate.is_file().then_some(candidate)
}

/// The IDE writes its compiler transcript to `<project>.err`, and writes **nothing** to stdout
/// or stderr (measured 2026-09-10: both streams are 0 bytes on a successful `-b`). So this
/// gate could see whether the build worked and not a word of what the compiler said about it
/// — which is how "does Delphi warn about this cast?" stayed an open question with a Delphi
/// sitting right here.
///
/// The file is UTF-8 with a BOM. That was measured too, after the first version of this
/// function assumed UTF-16 — the IDE writes plenty of UTF-16 elsewhere — and quietly produced
/// mojibake that matched none of the patterns the caller looks for. A decoder that "works"
/// and finds nothing looks exactly like a compiler that said nothing.
fn read_ide_transcript(err_file: &Path) -> String {
    let Ok(bytes) = std::fs::read(err_file) else {
        return String::new();
    };
    String::from_utf8_lossy(bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(&bytes)).into_owned()
}

/// Build `host.dproj` through the IDE. `Err` carries what a person can act on.
fn build_with_bds(bds: &Path, dproj: &Path, exe: &Path) -> Result<String, String> {
    let mut cmd = Command::new(bds);
    cmd.arg("-b").arg(dproj);
    let out = common::output_with_deadline(cmd, std::time::Duration::from_secs(300), "bds -b");
    let text = format!(
        "{}{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr),
        read_ide_transcript(&dproj.with_extension("err")),
    );
    // The artifact is the evidence, not the status -- the same rule this file already learned
    // from a dcc64 that exits 0 and writes nothing.
    if exe.is_file() {
        return Ok(text);
    }
    Err(format!(
        "bds.exe -b exited {} and produced no {}: {}",
        out.status,
        exe.display(),
        text.trim()
    ))
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
            "GATE_DELPHI_SKIPPED: no dcc64 found, so THIS RUN proves nothing about Delphi \
             -- a skipped gate is not a passed gate. The arm itself is not open: the gate \
             passes where a Delphi is installed (STATUS 9-20). It is this environment \
             that cannot check it, which is also why CI never can."
        );
        return;
    };

    // Found is not enough -- it has to be able to BUILD. Two ways it can:
    //
    //   dcc64 <file>              fast, and what an unrestricted edition offers
    //   bds.exe -b <project>      the IDE's own build, which Community Edition permits
    //
    // The second was missing until 2026-09-09, and its absence is why STATUS said this arm
    // needed a paid edition. That came from one generalisation: dcc64, dcc32 and msbuild are
    // all blocked (measured), therefore automation is impossible (never measured). Measured
    // now: `bds.exe -b host.dproj` builds a Win64 host.exe in about 12 seconds, exits 0 on
    // its own, and the host prints GATE_DELPHI_OK.
    let mut builder = Builder::Dcc;
    if let Err(reason) = dcc_can_compile(&dcc) {
        match bds_exe(&dcc) {
            Some(bds) => {
                println!(
                    "GATE_DELPHI_VIA_IDE: {reason}\n  falling back to {} -b, which this",
                    bds.display()
                );
                builder = Builder::Bds(bds);
            }
            None => {
                if std::env::var("MATHLESS_GATE_DELPHI").as_deref() == Ok("require") {
                    panic!("MATHLESS_GATE_DELPHI=require, and {reason} There is no bds.exe beside",);
                }
                println!(
                    "GATE_DELPHI_SKIPPED: a dcc64 was found at {} but cannot serve the gate, \
                     and there is no bds.exe beside it to build through the IDE instead. {}",
                    dcc.display(),
                    reason
                );
                return;
            }
        }
    }

    let work = common::TempOut::new("gate_delphi");

    // The units this host `uses`, read from the host rather than listed again here.
    //
    // This list WAS hand-written, and it went stale the day `shapes` was added to host.dpr:
    // the FPC test emitted four units and this one still emitted three, so the day dcc64
    // arrives this gate would have failed to find a unit -- for a reason that has nothing to
    // do with Delphi. Nothing caught it, because **a gate that cannot run cannot tell you it
    // is also broken.** Deriving the list is what stops that from happening again.
    let examples = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("examples");
    for unit in common::delphi_host_units() {
        let src =
            std::fs::read_to_string(examples.join(format!("{unit}.mls"))).unwrap_or_else(|e| {
                panic!("host.dpr uses `{unit}`, but examples/{unit}.mls could not be read: {e}")
            });
        emit_artifacts(&src, &unit, &work).unwrap_or_else(|e| panic!("emit {unit}: {e}"));
    }

    let host_dpr = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("delphi-host")
        .join("host.dpr");
    assert!(host_dpr.exists(), "missing {}", host_dpr.display());
    // Read once, and used below to ask the compiler transcript about specific source lines.
    let host_src = std::fs::read_to_string(&host_dpr).expect("read host.dpr");

    // The artifact is the evidence, not the exit code. Measured 2026-09-07: a dcc64 whose
    // licence forbids command-line builds prints one line, compiles NOTHING and exits 0.
    // Trusting the status made this gate die two statements later on `NotFound` for
    // host.exe -- a true message about the wrong thing.
    let exe = work.path().join("host.exe");

    let compile_out = match &builder {
        Builder::Dcc => {
            // `-U<dir>` puts the generated units on the search path; `-E<dir>` sends the exe
            // next to the DLLs, which the load-time imports need.
            let mut cmd = Command::new(&dcc);
            cmd.arg(format!("-U{}", work.path().display()))
                .arg(format!("-E{}", work.path().display()))
                .arg(format!("-N{}", work.path().display()))
                .arg(&host_dpr)
                .current_dir(work.path());
            let out =
                common::output_with_deadline(cmd, std::time::Duration::from_secs(300), "dcc64");
            format!(
                "{}{}",
                String::from_utf8_lossy(&out.stdout),
                String::from_utf8_lossy(&out.stderr)
            )
        }
        Builder::Bds(bds) => {
            // The IDE builds a PROJECT, so host.dpr is copied beside the artifacts with its
            // .dproj -- that file is the one Delphi itself generated for this host, and it
            // carries no machine-specific path (checked before committing it).
            let staged_dpr = work.path().join("host.dpr");
            let staged_dproj = work.path().join("host.dproj");
            std::fs::copy(&host_dpr, &staged_dpr).expect("stage host.dpr");
            std::fs::copy(host_dpr.with_extension("dproj"), &staged_dproj)
                .expect("stage host.dproj");
            build_with_bds(bds, &staged_dproj, &exe).unwrap_or_else(|e| panic!("{e}"))
        }
    };
    // The build path was chosen ABOVE by asking whether each one works, so reaching here
    // with no executable means the chosen one failed for some other reason -- and the
    // artifact, not a status, is what says so.
    assert!(
        exe.is_file(),
        "the build produced no {} -- no exit code is taken as proof that anything was \
         built:\n{compile_out}",
        exe.display()
    );
    // What the compiler SAID, not just whether it succeeded. The host deliberately writes
    // all three spellings of `UnicodeString -> PAnsiChar` (HOST_ABI.md string rule 5), and
    // this is where the other half of that measurement lives: the runtime checks show the
    // bytes each spelling sends, and these lines show which ones dcc64 objects to.
    let said: Vec<&str> = compile_out
        .lines()
        .map(str::trim)
        // Hints count. The claim about `PAnsiChar(Pointer(S))` is that the compiler says
        // NOTHING, and a hint is the compiler saying something -- leaving them out of this
        // filter would let a hint sit on that line while the gate reported silence.
        .filter(|l| {
            l.contains("Warning]")
                || l.contains("Error]")
                || l.contains("Hint]")
                || l.contains("Fatal")
        })
        .collect();
    println!("dcc64 said:");
    for line in &said {
        println!("  {line}");
    }
    // Which spelling draws a diagnostic and which does not is the WHOLE finding, so it is
    // pinned by the source line rather than by a count -- edit host.dpr freely, this still
    // asks the same question of whatever it now says.
    let lines_with = |needle: &str| -> Vec<usize> {
        host_src
            .lines()
            .enumerate()
            .filter(|(_, l)| l.contains(needle))
            .map(|(i, _)| i + 1)
            .collect()
    };
    // Markers, not the cast text: `PAnsiChar(S)` also appears in this file's prose and in
    // the messages the checks print, and a line that merely TALKS about the cast draws no
    // warning. The first version of this matched those and failed on a comment.
    let flagged = lines_with("{ ML_W1044 }");
    let silent = lines_with("{ ML_NO_DIAGNOSTIC }");
    assert!(
        !flagged.is_empty() && !silent.is_empty(),
        "host.dpr must still write both spellings for this to measure anything"
    );
    for n in &flagged {
        assert!(
            said.iter()
                .any(|l| l.contains(&format!("host.dpr({n})")) && l.contains("W1044")),
            "dcc64 said nothing about `PAnsiChar(S)` at host.dpr({n}). Measured 2026-09-10: it \
             answers W1044 (Suspicious typecast of string to PAnsiChar), and HOST_ABI.md's \
             string rule 5 now cites that as the reason this spelling is wrong-but-not-silent. \
             If a newer Delphi dropped the warning, the advice this project gives Delphi hosts \
             changed and someone has to say so. It said:\n{}",
            compile_out.trim()
        );
    }
    for n in &silent {
        assert!(
            !said.iter().any(|l| l.contains(&format!("host.dpr({n})"))),
            "dcc64 DID say something about `PAnsiChar(Pointer(S))` at host.dpr({n}). That \
             spelling is the one this project calls silent -- wrong bytes, no diagnostic -- \
             and the whole reason it is singled out. A warning here is good news and a \
             documentation change, not a test failure to paper over. It said:\n{}",
            compile_out.trim()
        );
    }

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
    // This guard has now forced this file to state something false TWICE, in opposite
    // directions. It first demanded "NEVER COMPILED BY DELPHI" after 9-15 had compiled it;
    // the fix then demanded "NOT GATED" -- and 9-20 made the gate run, so that became the
    // second lie. Both times the guard was green while the file was wrong, because it pinned
    // a PHRASE rather than a FACT.
    //
    // What is durable is the SHAPE of the limit, not its current wording: this host is
    // checked by a gate that runs on a developer machine and not in CI. Assert that, and
    // assert the superseded claim is gone -- a substring like "NOT GATED in CI" would keep
    // the old pin green while proving nothing.
    assert!(
        text.contains("MATHLESS_GATE_DELPHI"),
        "the staged host must name the gate that builds and runs it, so a reader knows what \
         to run: {}",
        host_dpr.display()
    );
    assert!(
        text.contains("not in CI"),
        "the staged host must say its gate does not run in CI -- the runner has neither \
         Delphi nor an interactive session: {}",
        host_dpr.display()
    );
    // Both halves are needed. Verification review caught that asserting only the two above
    // still let a file saying "NOT GATED" through, which is the very phrase this guard used
    // to demand -- the old lie could have walked straight back in beside the new truth.
    assert!(
        !text.contains("NOT GATED") && !text.contains("nothing repeats"),
        "the staged host still carries a superseded claim that its Delphi verification does \
         not repeat. It does: this very test drives it via `bds.exe -b` (9-20): {}",
        host_dpr.display()
    );
    assert!(
        text.contains("GATE_DELPHI_OK"),
        "the success marker is missing"
    );
}

/// **Every unit the Pascal host `uses` must also pass its load-time gate.**
///
/// `GateOk` calls `OneModule` once per module, and that list was maintained by hand while a
/// paragraph above it said the gate "covers every module used, not one". It had stopped
/// being true: `basket` was added to the `uses` clause for the array-input slice and never
/// gated, so the host called a module it had not checked — the same shape of defect the
/// fingerprint exists to prevent, in the host that checks fingerprints.
///
/// Deriving the list is what stops it happening a third time. It costs nothing and needs no
/// Delphi, so unlike the gate itself this DOES run in CI (windows-latest).
#[test]
fn the_delphi_host_gates_every_unit_it_uses() {
    let host_dpr = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("delphi-host")
        .join("host.dpr");
    let text = std::fs::read_to_string(&host_dpr).expect("read host.dpr");
    // Comments out first: a `OneModule('x'` written inside one is prose, not a check, and
    // would let a unit look gated while nothing called it.
    let code = common::strip_pascal_comments(&text);

    let units = common::delphi_host_units();
    assert!(
        units.len() >= 5,
        "recovered {units:?} from the uses clause — has its shape changed?"
    );
    for unit in &units {
        let call = format!("OneModule('{unit}'");
        assert!(
            code.contains(&call),
            "host.dpr uses `{unit}` but GateOk never calls {call}…): the host would call a \
             module whose abi version and interface fingerprint it never checked. The units \
             it uses are {units:?}"
        );
    }
}

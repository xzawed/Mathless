//! STATUS 3b-#4 — `mlc build` robustness:
//!
//! 1. **Module names are validated up front.** The name is interpolated raw into the
//!    generated `Cargo.toml` (`name = "<module>"`), the C header guard and the Delphi unit
//!    name, so a stem like `if`, `my-mod` or `2fast` used to surface as a confusing *cargo*
//!    failure instead of a clear frontend error — and a quoted stem could break out of the
//!    TOML string entirely.
//! 2. **Output lands all-or-nothing.** The **four** deliverables are staged and only then moved
//!    into `out_dir`, so a failure part-way through does not leave a `.dll` with no bindings
//!    next to it — a partial set a host could still load. (It said "three" until 2026-09-05;
//!    the `.lib` arrived in #124 and this sentence did not move.)
//!
//!    **Against I/O errors, not against process death.** The rollback runs in memory, so a
//!    kill during the four renames leaves whatever the OS had already done — a mixed old/new
//!    set. Nothing in-process can close that window, and this file does not pretend to: the
//!    tests below inject FAILURES, which is the case the design does handle.

use std::path::Path;

use mlc::emit::emit_artifacts;

mod common;
use common::TempOut;

const SRC: &str = "export fn f(a: f64) -> f64 { return a }";

/// The returned guard must be held for the body of the test: dropping it deletes the tree.
/// It used to be a bare `PathBuf` cleaned only on the way IN, which leaked one tree per run
/// forever (see `common/mod.rs`).
fn fresh_out(tag: &str) -> TempOut {
    TempOut::new(&format!("rb_{tag}"))
}

fn entries(dir: &Path) -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    v.sort();
    v
}

/// `emit_artifacts` keeps its staging directory in exactly one case — a rollback it could not
/// complete, where the stage holds the only copy of a displaced file. Every other path, error
/// paths included, must remove it. That invariant had no test on the error side (STATUS
/// §9-A A7), and a stage left behind in `out_dir` is litter the user has to recognise as ours.
///
/// `#[cfg(windows)]` because only the Windows tests reach a failure that stages anything;
/// without it the ubuntu job fails on `dead_code` under `clippy -D warnings`.
#[cfg(windows)]
fn no_stage_left(dir: &Path) -> bool {
    entries(dir).iter().all(|e| !e.starts_with(".mlc-stage-"))
}

#[test]
fn rejects_a_module_name_that_is_a_target_reserved_word() {
    // `if.mls` -> crate named `if` -> cargo rejects it as a Rust keyword, far from the cause.
    let out = fresh_out("kw");
    let err = emit_artifacts(SRC, "if", &out).unwrap_err();
    let shown = err.to_string();
    assert!(shown.contains("reserved"), "{shown}");
    assert!(shown.contains("if"), "{shown}");
    assert!(entries(&out).is_empty(), "nothing may be written: {out:?}");
}

#[test]
fn rejects_module_names_that_are_not_identifiers() {
    let out = fresh_out("ident");
    for bad in ["my-mod", "2fast", "", "a b", "mod.name", "üñî"] {
        let err = emit_artifacts(SRC, bad, &out)
            .unwrap_err()
            .to_string()
            .to_lowercase();
        assert!(
            err.contains("module name"),
            "'{bad}' should be rejected as a module name, got: {err}"
        );
    }
    assert!(entries(&out).is_empty(), "nothing may be written");
}

#[test]
fn rejects_a_module_name_that_would_escape_the_generated_cargo_toml() {
    // The name is interpolated into `name = "<module>"`; a quote must never reach that.
    let out = fresh_out("inject");
    let err = emit_artifacts(SRC, "x\", build = \"evil.rs", &out).unwrap_err();
    assert!(
        err.to_string().to_lowercase().contains("module name"),
        "{err}"
    );
    assert!(entries(&out).is_empty());
}

#[cfg(windows)]
#[test]
fn a_successful_build_leaves_exactly_the_four_artifacts() {
    // No staging directory, no build litter.
    let out = fresh_out("ok");
    emit_artifacts(SRC, "okmod", &out).expect("emit");
    assert_eq!(
        entries(&out),
        vec!["okmod.dll", "okmod.h", "okmod.lib", "okmod.pas"],
        "only the deliverables remain"
    );
}

#[cfg(windows)]
#[test]
fn a_failure_moving_the_bindings_leaves_no_partial_output() {
    // Force the `.h` to be unwritable by making its destination a directory. The `.dll` is
    // built successfully first, so without staging it would be left behind on its own.
    let out = fresh_out("partial");
    std::fs::create_dir(out.join("pmod.h")).unwrap();

    let err = emit_artifacts(SRC, "pmod", &out).unwrap_err();
    assert!(matches!(err, mlc::emit::EmitError::Io { .. }), "{err:?}");
    assert!(
        !out.join("pmod.dll").exists(),
        "a failed build must not leave a lone .dll: {:?}",
        entries(&out)
    );
    assert!(
        !out.join("pmod.pas").exists(),
        "nor a lone .pas: {:?}",
        entries(&out)
    );
    assert!(
        no_stage_left(&out),
        "the staging directory must be removed on the error path too: {:?}",
        entries(&out)
    );
}

#[cfg(windows)]
#[test]
fn an_io_failure_says_which_path_and_what_it_was_doing() {
    // STATUS §5-5.8. The message used to be `io error: <os text>` and nothing else. Measured
    // on this machine that reads:
    //
    //     mlc: io error: 액세스가 거부되었습니다. (os error 5)
    //
    // — no path, no operation, and the OS text is localised, so it is not even searchable.
    // The retrospective that filed this item took eight steps to find the cause; the path
    // makes it one.
    let out = fresh_out("iomsg");
    std::fs::create_dir(out.join("emsg.h")).unwrap();

    let err = emit_artifacts(SRC, "emsg", &out).unwrap_err();
    let shown = err.to_string();
    assert!(
        shown.contains("emsg.h"),
        "the message must name the file it failed on: {shown}"
    );
    assert!(
        shown.contains(&out.path().display().to_string()),
        "and where that file is: {shown}"
    );
    // The operation matters as much as the path: "could not write it" and "could not move it
    // into place" send you to different places.
    assert!(
        shown.contains("moving") || shown.contains("writing") || shown.contains("creating"),
        "the message must say what it was doing: {shown}"
    );
}

#[cfg(windows)]
#[test]
fn a_failed_rebuild_does_not_destroy_the_previous_good_artifacts() {
    // Build once successfully, then break the `.pas` destination and rebuild from DIFFERENT
    // source. The failed rebuild must leave the earlier artifacts intact — same paths, same
    // OLD contents. (Using different source matters: with identical output the assertion
    // would pass even with no rollback at all.)
    let out = fresh_out("rollback");
    emit_artifacts(SRC, "rmod", &out).expect("first emit");
    let old_header = std::fs::read_to_string(out.join("rmod.h")).unwrap();
    assert!(old_header.contains("mlx_f"), "{old_header}");

    std::fs::remove_file(out.join("rmod.pas")).unwrap();
    std::fs::create_dir(out.join("rmod.pas")).unwrap();

    let err = emit_artifacts("export fn g(a: f64) -> f64 { return a }", "rmod", &out).unwrap_err();
    assert!(matches!(err, mlc::emit::EmitError::Io { .. }), "{err:?}");
    assert!(
        out.join("rmod.dll").is_file(),
        "the previous .dll must be restored: {:?}",
        entries(&out)
    );
    let header_now = std::fs::read_to_string(out.join("rmod.h")).expect("the previous .h");
    assert_eq!(
        header_now, old_header,
        "the failed rebuild must not leave its own half-written .h behind"
    );
    assert!(
        no_stage_left(&out),
        "the staging directory must be removed after a completed rollback: {:?}",
        entries(&out)
    );
}

/// The deepest rollback there is: the LAST artifact fails to move.
///
/// `.lib` is `names[3]` in `emit_artifacts`, so a failure there has three completed moves to
/// undo, each with a displaced predecessor to put back. Every other error-path test in this
/// file breaks `.h` (index 1) or `.pas` (index 2) — measured, and it is the whole of
/// STATUS §9-A A7: the branch that unwinds a full set had never run.
///
/// The `.lib` also arrived last, in the linkable-bindings slice, which is exactly the kind of
/// addition that extends a loop without extending the test that covers it.
#[cfg(windows)]
#[test]
fn a_failure_moving_the_import_library_unwinds_all_three_earlier_moves() {
    // The name of this test claims `.lib` is the LAST move, and that is only true while it is
    // last in `emit_artifacts`' `names`. Append a fifth artifact after it and this test stays
    // green while quietly ceasing to cover the deepest unwind — the same shape as the guards
    // §9-A is about (Grok raised it verifying this change), so the order is read, not assumed.
    let emit_rs = include_str!("../src/emit.rs");
    let from = emit_rs
        .find("let names = [")
        .expect("emit.rs no longer has the `names` array this test reads the order from");
    let block = &emit_rs[from..from + emit_rs[from..].find("];").expect("unterminated")];
    let marker = "format!(\"{module_name}";
    let exts: Vec<&str> = block
        .match_indices(marker)
        .filter_map(|(i, _)| {
            let rest = &block[i + marker.len()..];
            rest.find('"').map(|c| &rest[..c])
        })
        .collect();
    assert_eq!(
        exts.last().copied(),
        Some(".lib"),
        "the artifact published LAST is no longer `.lib` but {:?}; this test breaks the last \
         destination on purpose, so point it at the new one",
        exts.last()
    );

    let out = fresh_out("lastmove");
    emit_artifacts(SRC, "lmod", &out).expect("first emit");
    let old_dll = std::fs::read(out.join("lmod.dll")).unwrap();
    let old_header = std::fs::read_to_string(out.join("lmod.h")).unwrap();
    let old_unit = std::fs::read_to_string(out.join("lmod.pas")).unwrap();
    assert!(old_header.contains("mlx_f"), "{old_header}");

    // Break only the LAST destination: a directory cannot be renamed over.
    std::fs::remove_file(out.join("lmod.lib")).unwrap();
    std::fs::create_dir(out.join("lmod.lib")).unwrap();

    // DIFFERENT source, so a missing rollback is visible in the CONTENTS. With identical
    // output the assertions below would pass with no rollback at all — the trap the `.pas`
    // test next door documents.
    let err = emit_artifacts("export fn g(a: f64) -> f64 { return a }", "lmod", &out).unwrap_err();
    assert!(matches!(err, mlc::emit::EmitError::Io { .. }), "{err:?}");
    assert!(
        err.to_string().contains("lmod.lib"),
        "the message must name the artifact that failed: {err}"
    );

    // Compared as a boolean, not with assert_eq!: the DLL is ~9 KB and a failing assert_eq!
    // prints both vectors as decimal bytes, which buries the sentence that says what broke.
    assert!(
        std::fs::read(out.join("lmod.dll")).expect("the previous .dll") == old_dll,
        "move 1 of 3 (.dll) must be undone — the .dll in place is not the one that was there \
         before the failed rebuild"
    );
    assert_eq!(
        std::fs::read_to_string(out.join("lmod.h")).expect("the previous .h"),
        old_header,
        "move 2 of 3 (.h) must be undone"
    );
    assert_eq!(
        std::fs::read_to_string(out.join("lmod.pas")).expect("the previous .pas"),
        old_unit,
        "move 3 of 3 (.pas) must be undone"
    );
    assert!(
        no_stage_left(&out),
        "a completed rollback must take its staging directory with it: {:?}",
        entries(&out)
    );
}

#[test]
fn the_cli_explains_a_bad_module_name_instead_of_failing_in_cargo() {
    // `if.mls` used to reach cargo and die there ("the name `if` cannot be used as a package
    // name"). The CLI must name the real problem and point at the file to rename.
    let dir = fresh_out("cli_name");
    let src = dir.join("if.mls");
    std::fs::write(&src, SRC).unwrap();

    let out = std::process::Command::new(env!("CARGO_BIN_EXE_mlc"))
        .args(["build".as_ref(), src.as_os_str()])
        .arg("-o")
        .arg(dir.path())
        .output()
        .expect("run mlc");

    assert!(!out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("module name"), "{stderr}");
    assert!(stderr.contains("reserved word"), "{stderr}");
    assert!(
        stderr.contains("rename"),
        "should point at the file to rename — {stderr}"
    );
    assert!(
        !stderr.contains("cargo"),
        "must not have reached cargo — {stderr}"
    );
    assert_eq!(entries(&dir), vec!["if.mls"], "no artifacts written");
}

#[test]
fn rejects_windows_device_names_as_module_names() {
    // A module name also becomes file names, and Windows resolves these as devices whatever
    // the extension. `nul.mls` used to die with "create crate dir: the system cannot find the
    // path specified" — an OS error nowhere near the cause.
    let out = fresh_out("device");
    for bad in ["nul", "NUL", "con", "Aux", "com1", "LPT9"] {
        let err = emit_artifacts(SRC, bad, &out).unwrap_err().to_string();
        assert!(
            err.contains("device name"),
            "'{bad}' should be rejected by name, got: {err}"
        );
    }
    // …while an ordinary name that merely starts the same way must NOT be rejected. Point
    // `out_dir` at an existing *file* so the call fails at I/O instead of building a DLL:
    // what matters is that the failure is not about the name.
    let blocked = out.join("not-a-dir");
    std::fs::write(&blocked, "").unwrap();
    let err = emit_artifacts(SRC, "console", &blocked).unwrap_err();
    assert!(
        !matches!(err, mlc::emit::EmitError::InvalidModuleName(_)),
        "'console' is a fine module name, got: {err}"
    );

    assert_eq!(entries(&out), vec!["not-a-dir"], "no artifacts written");
}

#[test]
fn rejects_a_module_name_that_delphi_reserves() {
    // `on` and `at` are Delphi reserved words (exception handling). They were missing from
    // `reserved.rs`, so `on.mls` built happily and emitted `unit on;` — which Delphi would
    // reject. Evidence level E1 (documented reserved word); dcc64 is absent here.
    let out = fresh_out("delphi_kw");
    for bad in ["on", "at", "ON"] {
        let err = emit_artifacts(SRC, bad, &out).unwrap_err().to_string();
        assert!(err.contains("Pascal"), "'{bad}': {err}");
    }
    assert!(entries(&out).is_empty());
}

/// `mlc build` must not depend on the environment it happens to be run from — and the check
/// must vary **more than one key**, because closing them one at a time is what left the class
/// open twice.
///
/// #164 found that `build_cdylib` reconstructed cargo's output directory instead of
/// controlling it, and closed `CARGO_TARGET_DIR` by passing `--target-dir`. That named where
/// cargo works; it did not fix the shape underneath. Measured after #164 shipped:
///
/// ```text
/// CARGO_BUILD_TARGET=x86_64-pc-windows-msvc  mlc build examples/discount.mls
/// mlc: codegen error: expected dll not found: …\discount\target\release\discount.dll
/// ```
///
/// byte-for-byte the failure #164's own comment describes, from a different variable — cargo
/// inserts a `<triple>/` component when it is cross-compiling by request.
///
/// The artifact is now FOUND under the directory cargo was given rather than reconstructed
/// from an assumed layout (`single_artifact`), so the list of variables that reshape it does
/// not have to be known. This test is the standing evidence for that: a new key added here
/// should pass without touching the compiler, and if one does not, the class is open again.
///
/// The child process is the real binary, because the variable has to reach the CHILD cargo,
/// and because this test binary's environment is shared with every other test in it.
#[cfg(windows)]
#[test]
fn a_build_ignores_whatever_cargo_variables_are_already_set() {
    let dir = std::env::temp_dir().join(format!("mlc_amb_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("temp dir");
    let src = dir.join("amb.mls");
    std::fs::write(&src, "export fn bump(x: i32) -> i32 { return x + 1 }\n").expect("write src");
    let hostile_dir = dir.join("someone_elses_target");

    // Each row is a way an ambient cargo setting reshapes the build. They are not a list the
    // compiler consults — they are a sample of an open set, which is the point: the fix has to
    // work without knowing them.
    let cases: Vec<(&str, String)> = vec![
        ("CARGO_TARGET_DIR", hostile_dir.display().to_string()),
        ("CARGO_BUILD_TARGET_DIR", hostile_dir.display().to_string()),
        // The one #164 missed: adds a `<triple>/` level under the target dir.
        ("CARGO_BUILD_TARGET", "x86_64-pc-windows-msvc".to_string()),
        ("CARGO_PROFILE_RELEASE_DEBUG", "true".to_string()),
        ("CARGO_INCREMENTAL", "1".to_string()),
        ("RUSTFLAGS", "-C overflow-checks=on".to_string()),
    ];

    for (key, value) in &cases {
        let out = dir.join(format!("out_{key}"));
        let r = std::process::Command::new(env!("CARGO_BIN_EXE_mlc"))
            .args(["build".as_ref(), src.as_os_str()])
            .arg("-o")
            .arg(&out)
            .env(key, value)
            .output()
            .expect("run mlc");
        assert!(
            r.status.success(),
            "{key}={value} changed whether `mlc build` works. The artifact must be found under \
             the directory cargo was given, not reconstructed from an assumed layout.\n{}\n{}",
            String::from_utf8_lossy(&r.stdout),
            String::from_utf8_lossy(&r.stderr)
        );
        for ext in ["dll", "h", "pas", "lib"] {
            let a = out.join(format!("amb.{ext}"));
            assert!(
                a.exists(),
                "{key}={value}: missing artifact {}",
                a.display()
            );
        }
    }

    let _ = std::fs::remove_dir_all(&dir);
}

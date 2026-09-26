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

/// Whether a file placed the way `publish` places one — created in a subfolder, renamed into
/// `out` — can then be deleted under the ACL the test has set. `false` means this process
/// deletes THROUGH the DACL, so the state the test needs cannot be built here.
#[cfg(windows)]
fn delete_is_blocked_for_a_placed_file(out: &Path) -> Result<(), String> {
    let probe_dir = out.join("probe.d");
    std::fs::create_dir(&probe_dir).unwrap();
    std::fs::write(probe_dir.join("p"), b"p").unwrap();
    std::fs::rename(probe_dir.join("p"), out.join("probe")).unwrap();
    let acl = |p: &Path| {
        std::process::Command::new("icacls")
            .arg(p)
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
            .unwrap_or_default()
    };
    let before = format!("{}{}", acl(out), acl(&out.join("probe")));
    match std::fs::remove_file(out.join("probe")) {
        Err(_) => Ok(()),
        Ok(()) => Err(before),
    }
}

/// `whoami /priv` and the integrity label, for a message that has to say WHY a token passed.
#[cfg(windows)]
fn token_report() -> String {
    let run = |args: &[&str]| {
        std::process::Command::new("whoami")
            .args(args)
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
            .unwrap_or_default()
    };
    format!(
        "{}\n{}",
        run(&["/priv", "/fo", "csv", "/nh"]).trim(),
        run(&["/groups", "/fo", "csv", "/nh"]).trim()
    )
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

/// **The rollback that cannot undo — `RollbackIncomplete`, end to end, with no race**
/// (STATUS §5-5.6 · R3).
///
/// The test above is the twin that succeeds: `.lib` fails last and all three earlier moves are
/// undone. Here the undo itself fails, which is the one situation in which `mlc` leaves files
/// behind on purpose — the displaced previous deliverables then exist ONLY in the staging
/// directory, so `emit_artifacts` must not delete it. `rollback`'s bookkeeping and the error's
/// `Display` are unit-tested inside `emit.rs`; the wiring between them — `publish` choosing the
/// variant, `emit_artifacts` keeping the stage — had never run.
///
/// **It was recorded as needing a race, and a DENY does not reach it** (measured 2026-09-24): a
/// deny-DELETE that stops the rollback from removing a newly placed file also stops the rename
/// that places it — the parent's "delete child" does not override an explicit deny. What does
/// reach it is an ACL that never GRANTS delete on files under `out_dir`. The stage, a subfolder,
/// grants "delete child", so a file may LEAVE it; `out_dir` itself does not, so the same file
/// cannot then be REMOVED from `out_dir`. The previous deliverables get an explicit DELETE so
/// they can still be moved aside.
#[cfg(windows)]
#[test]
fn a_rollback_that_cannot_undo_keeps_the_stage_and_the_only_copies() {
    let tmp = fresh_out("noundo");
    // A subfolder, so the `/reset` below has a parent of ours to recompute from.
    let out = tmp.join("acl");
    std::fs::create_dir(&out).unwrap();
    emit_artifacts(SRC, "umod", &out).expect("first emit");
    let previous: Vec<(&str, Vec<u8>)> = ["umod.dll", "umod.h", "umod.pas"]
        .into_iter()
        .map(|name| (name, std::fs::read(out.join(name)).unwrap()))
        .collect();

    // A later move must fail, as in the test above: a directory where the `.lib` goes.
    std::fs::remove_file(out.join("umod.lib")).unwrap();
    std::fs::create_dir(out.join("umod.lib")).unwrap();

    let _restore = common::InheritedAclRestored(out.clone());
    let me = format!("*{}", common::current_user_sid());
    // `/reset` first. On the CI runner a new directory under `%TEMP%` carries full control for
    // SYSTEM, Administrators and the user as plain copies — not flagged inherited — so
    // `/inheritance:r` leaves them, and they grant the very DELETE this test withholds
    // (measured on windows-latest, twice: once on the TempOut directory, once on this
    // subfolder). `/reset` drops every entry that is not flagged inherited and recomputes the
    // flagged ones from the parent; `/inheritance:r` then removes those, whatever SIDs the
    // environment added.
    common::icacls(&out, &["/reset"]);
    common::icacls(
        &out,
        &[
            "/inheritance:r",
            "/grant:r",
            // out_dir itself: may add files and folders, may NOT delete its children.
            &format!("{me}:(RX,W)"),
            // subfolders (the stage): full, so a file may be renamed out of one.
            &format!("{me}:(CI)(IO)(F)"),
            // files: read and write, never DELETE.
            &format!("{me}:(OI)(IO)(RX,W)"),
        ],
    );
    // The one exception to "never DELETE": the PREVIOUS deliverables, which `publish` must
    // still move aside (and would move back). Only the files built after this point lack it.
    for (name, _) in &previous {
        common::icacls(&out.join(name), &["/grant", &format!("{me}:(D)")]);
    }
    if let Err(acls) = delete_is_blocked_for_a_placed_file(&out) {
        panic!(
            "this process deleted a placed file THROUGH the ACL, so RollbackIncomplete cannot \
             be built here. ACLs before the delete:\n{acls}\nToken:\n{}",
            token_report()
        );
    }

    // DIFFERENT source, so the stage's copies can be told apart from the new ones.
    let err = emit_artifacts("export fn g(a: f64) -> f64 { return a }", "umod", &out).unwrap_err();
    let mlc::emit::EmitError::RollbackIncomplete {
        stage_dir,
        stranded,
        ..
    } = &err
    else {
        panic!(
            "the undo could not remove the files it had placed, so this must be \
             RollbackIncomplete: {err:?}"
        );
    };
    assert!(
        stage_dir.is_dir(),
        "the stage holds the only copies of the previous deliverables and must be kept: {:?}",
        entries(&out)
    );
    assert_eq!(stranded.len(), previous.len(), "{stranded:?}");
    for (name, bytes) in &previous {
        let kept = stage_dir.join(format!("{name}.prev"));
        assert!(
            stranded.contains(&kept),
            "{} is not reported as stranded: {stranded:?}",
            kept.display()
        );
        assert!(
            std::fs::read(&kept).is_ok_and(|b| b == *bytes),
            "{} must be byte-for-byte the {name} that was there before the failed rebuild",
            kept.display()
        );
    }
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
    let dir = common::TempOut::new("amb");
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

/// **SPEC-helper-check-propagation acceptance G(2): checked copies leave no dead code.**
///
/// One helper is called from both kinds of body (it needs its plain body AND its checked copy),
/// one only from a fallible body (its plain body would be dead). Built under an ambient
/// `RUSTFLAGS=-D warnings`, any body nobody calls is a hard error. The prototype emitted a copy
/// for every helper and kept every plain body, and `examples/discount4.mls` stopped building this
/// way (measured). The snake_case names are deliberate: a non-snake_case user name trips
/// `non_snake_case` under this flag today, which is recorded in the SPEC §5 and is not this
/// test's subject.
#[cfg(windows)]
#[test]
fn checked_copies_build_under_an_ambient_deny_warnings() {
    let dir = common::TempOut::new("ckdw");
    let src = dir.join("ckdw.mls");
    std::fs::write(
        &src,
        "fn sq(x: i32) -> i32 { return x * x }\n\
         fn cube(x: i32) -> i32 { return x * x * x }\n\
         export fn f(x: i32) -> i32! { return sq(x) + cube(x) }\n\
         export fn g(x: i32) -> i32 { return sq(x) }\n",
    )
    .expect("write src");
    let out = dir.join("out");
    let r = std::process::Command::new(env!("CARGO_BIN_EXE_mlc"))
        .args(["build".as_ref(), src.as_os_str()])
        .arg("-o")
        .arg(&out)
        .env("RUSTFLAGS", "-D warnings")
        .output()
        .expect("run mlc");
    assert!(
        r.status.success(),
        "a module with checked helpers must build under RUSTFLAGS=-D warnings:\n{}\n{}",
        String::from_utf8_lossy(&r.stdout),
        String::from_utf8_lossy(&r.stderr)
    );
    assert!(out.join("ckdw.dll").exists(), "missing ckdw.dll");
}

/// A module name has a maximum length, and the boundary is checked from both sides.
///
/// The name becomes the suffix of a reserved export (`ml_iface_hash_<module>`) since
/// `SPEC-qualified-iface-hash`, so a DYNAMIC host has to BUILD that symbol name and needs a
/// buffer for it. Measured before `ML_MAX_MODULE_NAME` existed: `hosts/c-host/host.c` could
/// gate a 65-character name and refused at 66, while `mlc build` accepted 70 and exited 0 —
/// the compiler happily produced a module its own reference host could not gate.
///
/// Refusing here is what moves the failure to where the diagnostic is good.
#[test]
fn rejects_a_module_name_longer_than_the_abi_bound() {
    let out = fresh_out("toolong");
    let max = mlc::abi::ML_MAX_MODULE_NAME;

    // Both sides of the edge, or this passes while the bound is off by one.
    //
    // The accepted side asserts that the name is not refused AS A NAME, rather than that the
    // whole build succeeds. `check_module_name` runs first — cheapest check first, by its own
    // comment — so that is the property this test owns, and it is the same on every platform.
    //
    // Demanding a finished artifact here was wrong and CI said so: on Linux `build_cdylib`
    // looks for `<name>.dll` while cargo writes `lib<name>.so`, which is the D22 gap
    // `generated_crate_output.rs` already documents and deliberately leaves unstarted. The
    // ubuntu insurance job caught it; a Windows-only run cannot.
    //
    // The end-to-end build at the bound is not lost — acceptance C of
    // `SPEC-module-name-length` builds a module with this exact name and has the reference C
    // host gate it (`hosts/rust-oracle/tests/c_host.rs`, Windows).
    let at_bound = "m".repeat(max);
    if let Err(e) = emit_artifacts(SRC, &at_bound, &out) {
        let msg = e.to_string();
        assert!(
            !msg.to_lowercase().contains("module name"),
            "a name of exactly {max} characters must not be refused as a module name: {msg}"
        );
    }

    let over = "m".repeat(max + 1);
    let err = emit_artifacts(SRC, &over, &out).unwrap_err().to_string();
    assert!(
        err.to_lowercase().contains("module name"),
        "a name of {} characters must be refused as a module name, got: {err}",
        max + 1
    );
    // The refusal says the limit and why there is one — the same standard as the other
    // module-name refusals, which name the target that reserves the word.
    assert!(
        err.contains(&max.to_string()),
        "the refusal must state the limit so the author can act on it: {err}"
    );
    assert!(
        err.contains("ml_iface_hash_"),
        "the refusal must say WHY a module name is bounded — it becomes a reserved export \
         symbol a dynamic host has to build: {err}"
    );
}

/// What `mlc build` PRINTS, read from the binary rather than from the source that prints it.
///
/// `doc_claims` already asserts the success output names every artifact — but by searching
/// `compiler/src/main.rs` for `arts.<field>.display()`. That is a guard on the code that makes
/// the output, which is the exact shape STATUS §7 records getting wrong three times on
/// `header.rs`: **guard the artifact, not the code that makes it.** The rule was applied there
/// and never here.
///
/// Measured, not argued. Replacing `println!("  {}", arts.dll.display())` with
/// `let _unused = format!("  {}", arts.dll.display())` drops the MODULE's own path from what a
/// user reads, and `doc_claims` (20), `emit_robustness` (24) and `diagnostics` (14) all stayed
/// green — while that guard's own failure message reads "A file written and not reported is one
/// the user does not know they have". The text it searches for is still there; the line is not.
///
/// The expectation is DERIVED from the directory rather than listed, so a fifth artifact has to
/// appear here the day it is written, without anyone remembering to add it (STATUS §7: a
/// hand-written floor loosens itself every time the corpus grows).
///
/// `#[cfg(windows)]` for the reason every SUCCESS-path CLI test in this file carries it: the
/// build produces a `.dll` and a `.lib` and needs the MSVC toolchain, so on the ubuntu job it
/// fails before there is any output to read. **The first version of this test did not have the
/// attribute, the whole Windows suite was green locally, and the 18-second ubuntu job caught
/// it** — which is what that job is for.
#[cfg(windows)]
#[test]
fn the_success_output_names_every_file_the_build_wrote() {
    let dir = fresh_out("cli_transcript");
    let src = dir.join("t.mls");
    std::fs::write(&src, SRC).unwrap();

    let out = dir.join("artifacts");
    let r = std::process::Command::new(env!("CARGO_BIN_EXE_mlc"))
        .args(["build".as_ref(), src.as_os_str()])
        .arg("-o")
        .arg(&out)
        .output()
        .expect("run mlc");
    assert!(
        r.status.success(),
        "the build must succeed before its output means anything:\n{}",
        String::from_utf8_lossy(&r.stderr)
    );

    let stdout = String::from_utf8_lossy(&r.stdout);
    assert!(stdout.contains("mlc: wrote"), "{stdout}");

    let written = entries(&out);
    assert!(
        !written.is_empty(),
        "nothing was written, so this test would pass without asserting anything"
    );
    for name in &written {
        assert!(
            stdout.contains(name.as_str()),
            "`mlc build` wrote {name} and did not name it — a file written and not reported is \
             one the user does not know they have:\n{stdout}"
        );
    }
}

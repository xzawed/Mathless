//! `STATUS.md` §5-5.7 — does a failed `mlc build` leave its temp build tree behind?
//!
//! The debt was opened because the documents said the tree is cleaned *"성공·실패와 무관하게"*
//! while the code says only that it *tries* — `remove_dir_all`'s result is discarded. Nine
//! trees were counted on this machine when it was written, and later the test suite was found
//! leaking far more (1,720 / 976 MB) for a different reason: helpers that cleaned on the way
//! IN, so every run used a new name and the old one stayed forever (§9-48).
//!
//! That is fixed, and until now the only evidence was someone counting by hand. §7 says the
//! quiet part out loud: *"문서와 코드를 대조하는 것으로는 부족하다 — 실행해 봐야 한다"*, and a
//! hand count does not survive the next change. This runs the CLI and counts.
//!
//! **The count is scoped to the CHILD's pid, and that is what makes it race-free.**
//! `emit.rs` names each tree `mlc-build-<pid>-<seq>`. Tests in this binary run in parallel and
//! several of them call `emit_artifacts`, so counting every `mlc-build-*` in `%TEMP%` would
//! see their in-flight trees and fail for reasons that have nothing to do with the CLI.
//! Spawning `mlc` gives a pid nobody else can be using.
//!
//! **What this does NOT cover**, so nobody reads it as more: the branch where `remove_dir_all`
//! itself fails — a locked destination — is unreachable from a filesystem arrangement and
//! needs a race, which is the same wall §5-5.6 documents for `RollbackIncomplete`.
//!
//! **Windows-only, and the first version of this file was not.** A successful `mlc build` is
//! what creates the tree this measures, and `codegen::build_cdylib` looks for
//! `target/release/<name>.dll` — on Linux cargo writes `lib<name>.so`, so the build fails and
//! the case that matters cannot run. That is the unstarted D22 gap
//! (`compiler/tests/generated_crate_output.rs`). The 24-second ubuntu job caught it, which is
//! what that job is for, and `every_module_building_test_is_windows_gated` now catches it
//! before the push.
#![cfg(windows)]

use std::path::{Path, PathBuf};
use std::process::Command;

mod common;

fn temp_trees_for(pid: u32) -> Vec<PathBuf> {
    let prefix = format!("mlc-build-{pid}-");
    std::fs::read_dir(std::env::temp_dir())
        .expect("%TEMP% is readable")
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with(&prefix))
        })
        .collect()
}

/// Run `mlc build` and return (succeeded, its own stderr, trees left behind by THAT process).
///
/// stderr is captured rather than discarded because a case here is identified by WHICH exit
/// it drives, and "exited non-zero" does not say which. Grok raised it on the `nul` case:
/// `nul.mls` could be the Windows NUL device rather than a file, in which case `mlc` would be
/// reading an empty source and that case would be testing a parse error under another name.
/// Measured instead of argued — the diagnostic is asserted below, so the case cannot quietly
/// become a different one.
fn build(mls: &Path, out: &Path) -> (bool, String, Vec<PathBuf>) {
    let child = Command::new(env!("CARGO_BIN_EXE_mlc"))
        .arg("build")
        .arg(mls)
        .arg("-o")
        .arg(out)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("spawn mlc");
    let pid = child.id();
    let done = child.wait_with_output().expect("wait for mlc");
    (
        done.status.success(),
        String::from_utf8_lossy(&done.stderr).into_owned(),
        temp_trees_for(pid),
    )
}

/// Every way `mlc build` can end leaves `%TEMP%` as it found it.
///
/// **Two of these five cases are the ones that can leak, and the other three are labelled so
/// nobody counts them as coverage.** A parse or type error fails in the front end, before a
/// build tree exists; a rejected module name fails at the entry check. Planting a skipped
/// cleanup makes only the last two go red, which is how the split below was measured rather
/// than assumed.
#[test]
fn a_build_leaves_no_temp_tree_behind_however_it_ends() {
    // Floor: the naming this scan depends on. If `emit.rs` renames its trees, the prefix
    // matches nothing and every assertion below passes by looking at an empty set.
    let emit_rs = include_str!("../src/emit.rs");
    assert!(
        emit_rs.contains(r#"format!("mlc-build-{}-{n}", std::process::id())"#),
        "compiler/src/emit.rs no longer names its build trees `mlc-build-<pid>-<seq>`. This \
         test finds them by that prefix and by the child's pid — with a different name it \
         would scan an empty set and pass forever"
    );

    // `common::TempOut`, not a hand-rolled directory: it removes the tree on Drop, so the
    // work dir goes even when an assertion below panics. The first version of this file did
    // it by hand and `no_test_creates_a_temp_directory_by_hand` caught it — a test about
    // leaking temp trees, leaking a temp tree exactly while someone iterates on it.
    let work = common::TempOut::new("clitemp");
    const GOOD: &str = "export fn f(a: f64) -> f64 { return a }\n";

    // Front-end and entry-check exits. These never reach the build tree — kept because they
    // are real CLI outcomes and because a change that moved the tree EARLIER would start
    // being covered here, but they are not what closes §5-5.7.
    for (stem, src, says) in [
        (
            "parse_err",
            "export fn f(a: f64) -> f64 { return a +\n",
            "parse error",
        ),
        (
            "type_err",
            "export fn f(a: f64) -> i32 { return a }\n",
            "type error",
        ),
        // A reserved Windows device name, rejected at the entry check. The expected text is
        // what proves it: if `nul.mls` resolved to the NUL device the source would read back
        // empty and this would be a parse error wearing another case's name.
        ("nul", GOOD, "invalid module name"),
    ] {
        let mls = work.join(format!("{stem}.mls"));
        std::fs::write(&mls, src).expect("write source");
        let (ok, err, left) = build(&mls, &work.join(format!("out_{stem}")));
        assert!(
            !ok,
            "`{stem}` was supposed to fail; it no longer drives that CLI exit"
        );
        assert!(
            err.contains(says),
            "`{stem}` was supposed to fail with `{says}`, and which exit it drives is what \
             makes it a distinct case. Got:\n{err}"
        );
        assert!(left.is_empty(), "`{stem}` left {left:?}");
    }

    // The case that builds. This is the only path that runs cargo, so it is the only one with
    // an artifact that could still be locked when the cleanup runs.
    let good = work.join("good.mls");
    std::fs::write(&good, GOOD).expect("write source");
    let out = work.join("out_good");
    let (ok, _err, left) = build(&good, &out);
    assert!(
        ok,
        "a valid module must build; this case is the one that creates a temp tree"
    );
    assert!(
        left.is_empty(),
        "a SUCCESSFUL `mlc build` left {} temp build tree(s): {left:?}. docs/STATUS.md §5-5.7 \
         is about exactly this — the documents say the tree goes whether the build succeeds \
         or fails, and `emit.rs` discards `remove_dir_all`'s result, so the claim needs a \
         measurement rather than a reading",
        left.len()
    );

    // And the case that builds and THEN fails. A directory cannot be renamed over, so putting
    // one where the last artifact must land breaks the publish step after cargo has already
    // produced everything — the technique `emit_robustness.rs` uses for the rollback tests.
    // Without this, "however it ends" would mean "when it ends well".
    let lib = out.join("good.lib");
    std::fs::remove_file(&lib).expect("the successful build left a .lib");
    std::fs::create_dir(&lib).expect("replace it with a directory");
    let (ok, _err, left) = build(&good, &out);
    assert!(
        !ok,
        "with a directory in place of `good.lib` the publish step must fail; if it now \
         succeeds, this case has stopped exercising a failure after the build tree exists"
    );
    assert!(
        left.is_empty(),
        "`mlc build` failing AFTER cargo ran left {} temp build tree(s): {left:?}. This is the \
         path §5-5.7 was written about — the tree exists by then, and the failure is what the \
         documents claim does not change the cleanup",
        left.len()
    );
}

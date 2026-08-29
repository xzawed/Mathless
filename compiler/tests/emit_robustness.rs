//! STATUS 3b-#4 — `mlc build` robustness:
//!
//! 1. **Module names are validated up front.** The name is interpolated raw into the
//!    generated `Cargo.toml` (`name = "<module>"`), the C header guard and the Delphi unit
//!    name, so a stem like `if`, `my-mod` or `2fast` used to surface as a confusing *cargo*
//!    failure instead of a clear frontend error — and a quoted stem could break out of the
//!    TOML string entirely.
//! 2. **Output lands all-or-nothing.** The three deliverables are staged and only then moved
//!    into `out_dir`, so a failure part-way through does not leave a `.dll` with no bindings
//!    next to it — a partial set a host could still load.

use std::path::{Path, PathBuf};

use mlc::emit::emit_artifacts;

const SRC: &str = "export fn f(a: f64) -> f64 { return a }";

fn fresh_out(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("mlc_rb_{tag}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn entries(dir: &Path) -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    v.sort();
    v
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
fn a_successful_build_leaves_exactly_the_three_artifacts() {
    // No staging directory, no build litter.
    let out = fresh_out("ok");
    emit_artifacts(SRC, "okmod", &out).expect("emit");
    assert_eq!(
        entries(&out),
        vec!["okmod.dll", "okmod.h", "okmod.pas"],
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
    assert!(matches!(err, mlc::emit::EmitError::Io(_)), "{err:?}");
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
    assert!(matches!(err, mlc::emit::EmitError::Io(_)), "{err:?}");
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
        .arg(&dir)
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

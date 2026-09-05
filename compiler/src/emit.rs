//! STEP 1 (Gate-D prep): package a `.mls` module into the four consumable artifacts on
//! disk — `<name>.dll` (native module), `<name>.h` (C header), `<name>.pas` (Delphi import
//! unit), `<name>.lib` (MSVC import library). This is the library entrypoint behind the
//! `mlc build` CLI (`src/main.rs`).
//!
//! Honesty split: producing the `.dll` and loading it via the Rust oracle is E2 (see
//! `hosts/rust-oracle/tests/emit_artifacts.rs`). The **`.h` is now E2 as well** — a real C
//! host built with MSVC compiles it and calls the module (`hosts/rust-oracle/tests/c_host.rs`,
//! acceptance D). The **`.pas` is still unverified**: there is no `dcc64` here, so nothing has
//! ever compiled the Delphi unit, and it keeps its DRAFT note.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::{codegen, compile_to_ir, header, CompileError};

/// The four files [`emit_artifacts`] writes, by `out_dir`-joined path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Artifacts {
    /// The native module, `<out_dir>/<module>.dll`.
    pub dll: PathBuf,
    /// The C ABI header, `<out_dir>/<module>.h`.
    pub header: PathBuf,
    /// The Delphi import unit, `<out_dir>/<module>.pas`.
    pub delphi_unit: PathBuf,
    /// The MSVC import library, `<out_dir>/<module>.lib`.
    ///
    /// The fourth artifact, added by `SPEC-linkable-bindings`. With it a C host can
    /// `#include` the generated header and LINK, instead of resolving every symbol through
    /// `GetProcAddress`. Published as `<module>.lib`; cargo's own `<crate>.dll.lib` is a
    /// build detail that does not leave the build tree.
    pub import_lib: PathBuf,
}

/// Why packaging failed: the source did not compile, or an I/O step (build, copy, write)
/// failed.
#[derive(Debug)]
pub enum EmitError {
    Compile(CompileError),
    /// A filesystem step failed, with the path and the operation that failed.
    ///
    /// Both halves are load-bearing (STATUS §5-5.8). The message used to be
    /// `io error: <os text>`, which on a localised Windows reads
    /// `액세스가 거부되었습니다. (os error 5)` — no path, no operation, and not even
    /// searchable. The retrospective that filed the item spent eight steps finding the cause.
    Io {
        /// What was being attempted, as a gerund: "moving", "writing", "creating".
        doing: &'static str,
        path: PathBuf,
        source: std::io::Error,
    },
    /// The module name is not usable as a crate / header-guard / unit name.
    InvalidModuleName(String),
    /// A move into `out_dir` failed **and** the artifacts that were displaced could not be
    /// put back. `stage_dir` is deliberately left on disk so they can be recovered by hand;
    /// this is the one path where the staging directory survives.
    RollbackIncomplete {
        source: std::io::Error,
        stage_dir: PathBuf,
        stranded: Vec<PathBuf>,
    },
}

impl std::fmt::Display for EmitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            // `CompileError` names its own stage and position, so don't re-prefix it.
            EmitError::Compile(e) => write!(f, "{e}"),
            EmitError::Io {
                doing,
                path,
                source,
            } => write!(f, "io error {doing} {}: {source}", path.display()),
            EmitError::InvalidModuleName(msg) => write!(f, "{msg}"),
            EmitError::RollbackIncomplete {
                source,
                stage_dir,
                stranded,
            } => write!(
                f,
                "io error: {source}; and the previous artifacts could NOT be put back — {} file(s) \
                 are left in {} and must be moved back by hand",
                stranded.len(),
                stage_dir.display()
            ),
        }
    }
}

impl std::error::Error for EmitError {}

impl From<CompileError> for EmitError {
    fn from(e: CompileError) -> Self {
        EmitError::Compile(e)
    }
}

/// Attach the path and the operation to a filesystem error.
///
/// There is deliberately **no** `From<std::io::Error>` impl: a bare `?` would produce an
/// error with no context, which is the defect this replaces. Every call site has to say what
/// it was doing and to which file — and the compiler now makes it.
fn io<T>(
    r: std::io::Result<T>,
    doing: &'static str,
    path: impl Into<PathBuf>,
) -> Result<T, EmitError> {
    r.map_err(|source| EmitError::Io {
        doing,
        path: path.into(),
        source,
    })
}

static BUILD_SEQ: AtomicU64 = AtomicU64::new(0);

/// Reject a module name that can't be used verbatim as a crate name, a C header guard or a
/// Delphi unit name.
///
/// The name is interpolated **raw** into the generated `Cargo.toml` (`name = "<module>"`),
/// into `ML_<MODULE>_H` and into `unit <module>;`. Without this check a stem like `if` or
/// `my-mod` surfaced as a confusing *cargo* failure far from the cause, and a stem
/// containing a quote could break out of the TOML string altogether.
fn check_module_name(name: &str) -> Result<(), EmitError> {
    let mut chars = name.chars();
    let is_identifier = match chars.next() {
        Some(c) if c.is_ascii_alphabetic() || c == '_' => {
            chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
        }
        _ => false,
    };
    if !is_identifier {
        return Err(EmitError::InvalidModuleName(format!(
            "invalid module name {name:?} — a module name must be an identifier: ASCII letters, \
             digits and `_`, not starting with a digit"
        )));
    }
    // Same policy as parameter and local names: safe in every codegen target at once.
    let targets = crate::reserved::reserving_targets(name);
    if !targets.is_empty() {
        return Err(EmitError::InvalidModuleName(format!(
            "invalid module name '{name}' — reserved word in {}; it becomes the generated crate \
             name, the C header guard and the Delphi unit name",
            targets.join(", ")
        )));
    }
    // Unlike parameters and locals, a module name also becomes FILE names. Windows resolves
    // these device names whatever the extension, so `nul.mls` died deep in codegen with
    // "create crate dir: the system cannot find the path specified" (measured) instead of
    // naming the real problem.
    if WINDOWS_DEVICE_NAMES
        .iter()
        .any(|d| d.eq_ignore_ascii_case(name))
    {
        return Err(EmitError::InvalidModuleName(format!(
            "invalid module name '{name}' — a reserved Windows device name; it becomes the file \
             names '{name}.dll' / '.h' / '.pas' / '.lib', which the OS resolves to the device"
        )));
    }
    Ok(())
}

/// Windows resolves these as devices regardless of extension (`nul.h` is the NUL device),
/// so they can never be module names on the Phase 1 target (D22).
static WINDOWS_DEVICE_NAMES: &[&str] = &[
    "con", "prn", "aux", "nul", "com1", "com2", "com3", "com4", "com5", "com6", "com7", "com8",
    "com9", "lpt1", "lpt2", "lpt3", "lpt4", "lpt5", "lpt6", "lpt7", "lpt8", "lpt9",
];

/// Move `names` from `stage` into `out_dir`, leaving `out_dir` as it was if any move fails.
///
/// Each destination that already exists is moved aside into `stage` first, so a later
/// failure can put it back. This is the "all-or-nothing" part: by the time it runs, all
/// four files exist complete in `stage`, so the only remaining failure window is the moves
/// themselves — and those are undone.
///
/// **The guarantee is against I/O ERRORS, not against process death.** The undo is in-memory
/// state (`done`), so a kill in the middle of these renames leaves whatever the OS already
/// committed — a mixed old/new set that nothing here can repair, because the code that would
/// repair it is the code that died. Making that survivable needs an on-disk journal, which is
/// out of proportion to a four-file publish; the honest move is to say so rather than let
/// "all-or-nothing" be read as crash-safe (Grok verify raised it).
///
/// If a restore *itself* fails, the displaced file is still sitting in `stage`, and silently
/// deleting the staging directory afterwards would destroy it (Grok verify). So the error
/// reports those paths and the caller keeps the directory.
fn publish(stage: &Path, out_dir: &Path, names: &[String]) -> Result<(), EmitError> {
    // (destination, its backup) for each completed move, most recent last.
    let mut done: Vec<(PathBuf, Option<PathBuf>)> = Vec::new();
    // `path` is the destination the move was aiming at — the one thing a user needs in order
    // to know what to look at (STATUS §5-5.8).
    let fail = |e: std::io::Error, path: PathBuf, stranded: Vec<PathBuf>| {
        if stranded.is_empty() {
            EmitError::Io {
                doing: "moving into place",
                path,
                source: e,
            }
        } else {
            EmitError::RollbackIncomplete {
                source: e,
                stage_dir: stage.to_path_buf(),
                stranded,
            }
        }
    };
    for name in names {
        let dest = out_dir.join(name);
        let backup = if dest.is_file() {
            let b = stage.join(format!("{name}.prev"));
            if let Err(e) = std::fs::rename(&dest, &b) {
                let mut stranded = rollback(&mut done);
                // Defensive: a rename that reports failure *after* moving the file would
                // otherwise leave the only copy in the stage, which we are about to delete.
                if b.exists() {
                    stranded.push(b);
                }
                return Err(fail(e, dest.clone(), stranded));
            }
            Some(b)
        } else {
            None
        };
        if let Err(e) = std::fs::rename(stage.join(name), &dest) {
            // Put this destination's own backup back before unwinding the earlier moves.
            let mut stranded = Vec::new();
            if let Some(b) = backup {
                if std::fs::rename(&b, &dest).is_err() {
                    stranded.push(b);
                }
            }
            stranded.extend(rollback(&mut done));
            return Err(fail(e, dest.clone(), stranded));
        }
        done.push((dest, backup));
    }
    Ok(())
}

/// Undo completed moves, newest first: drop what we put there, restore what we displaced.
///
/// Returns the backups it could **not** put back. An empty result means `out_dir` is exactly
/// as it was; a non-empty one means those files exist only inside the staging directory, so
/// the caller must not delete it.
fn rollback(done: &mut Vec<(PathBuf, Option<PathBuf>)>) -> Vec<PathBuf> {
    let mut stranded = Vec::new();
    while let Some((dest, backup)) = done.pop() {
        // `remove_file` failing on a path that no longer exists is fine; failing on one that
        // does (locked, or replaced by a directory) means the restore below cannot happen.
        let cleared = std::fs::remove_file(&dest).is_ok() || !dest.exists();
        if let Some(b) = backup {
            if !cleared || std::fs::rename(&b, &dest).is_err() {
                stranded.push(b);
            }
        }
    }
    stranded
}

/// A private, process-unique build root under the OS temp dir, so two concurrent
/// `emit_artifacts` calls never share a build directory (fixed-name race). Removed by
/// [`emit_artifacts`] once the DLL is copied out.
fn unique_build_root() -> PathBuf {
    let n = BUILD_SEQ.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!("mlc-build-{}-{n}", std::process::id()))
}

/// Compile `src` and write the four deliverables for `module_name` into `out_dir`,
/// returning their paths. `out_dir` is created if missing. The DLL is built in a private
/// temp tree that is deleted before returning (no build litter in `out_dir`).
///
/// Errors: [`EmitError::Compile`] if `src` fails to parse/typecheck/codegen (nothing is
/// written), [`EmitError::Io`] if a filesystem step fails.
pub fn emit_artifacts(
    src: &str,
    module_name: &str,
    out_dir: &Path,
) -> Result<Artifacts, EmitError> {
    // Cheapest check first: a bad name would otherwise surface as a cargo/Delphi failure.
    check_module_name(module_name)?;

    // Front + middle end: source → typed IR (feeds both the DLL and the bindings).
    let ir = compile_to_ir(src)?;

    // Back end: IR → extern "C" Rust → cdylib DLL, in a private self-cleaning build tree.
    let rust = codegen::emit(&ir).map_err(|e| EmitError::Compile(CompileError::Codegen(e)))?;

    // Stage all four deliverables next to their destination, then move them into place.
    // Nothing reaches `out_dir` until everything has been produced, so a failure part-way
    // through can't leave a `.dll` with no bindings beside it — a partial set a host could
    // still load. The stage lives INSIDE `out_dir` so the moves stay on one volume (a
    // cross-volume rename would fail), and it is removed on every path.
    io(std::fs::create_dir_all(out_dir), "creating", out_dir)?;
    let stage = out_dir.join(format!(
        ".mlc-stage-{}-{}",
        std::process::id(),
        BUILD_SEQ.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&stage);
    io(std::fs::create_dir_all(&stage), "creating", &stage)?;

    let names = [
        format!("{module_name}.dll"),
        format!("{module_name}.h"),
        format!("{module_name}.pas"),
        format!("{module_name}.lib"),
    ];

    let staged = || -> Result<(), EmitError> {
        // Build the DLL in a private temp tree and copy it into the stage. The tree is
        // removed whether the build/copy succeeds OR fails, so an error path never leaks a
        // temp directory (Grok verify: temp-leak on error). `remove_dir_all` failure is
        // ignored (a loaded/locked file on Windows shouldn't fail the build).
        let build_root = unique_build_root();
        let build_and_copy = || -> Result<(), EmitError> {
            let built = codegen::build_cdylib(&rust, module_name, &build_root)
                .map_err(|e| EmitError::Compile(CompileError::Codegen(e)))?;
            let dest = stage.join(&names[0]);
            io(
                std::fs::copy(&built.dll, &dest),
                "copying the module into",
                &dest,
            )?;
            // The import library comes out of the same build tree, and that tree is deleted
            // as soon as this closure returns — so it is copied here, beside the DLL, not
            // later from the caller.
            let lib_dest = stage.join(&names[3]);
            io(
                std::fs::copy(&built.import_lib, &lib_dest),
                "copying the import library into",
                &lib_dest,
            )?;
            Ok(())
        };
        let result = build_and_copy();
        let _ = std::fs::remove_dir_all(&build_root);
        result?;

        // Bindings: the `.h` is verified by the C host (acceptance D); the `.pas` is not.
        // unit name to equal the file stem, so the unit name IS the module name.
        let header_path = stage.join(&names[1]);
        io(
            std::fs::write(&header_path, header::emit_c_header(&ir, module_name)),
            "writing",
            &header_path,
        )?;
        let unit_path = stage.join(&names[2]);
        io(
            std::fs::write(
                &unit_path,
                header::emit_delphi_unit(&ir, module_name, module_name),
            ),
            "writing",
            &unit_path,
        )?;

        publish(&stage, out_dir, &names)
    };
    let result = staged();
    // Keep the staging directory in exactly one case: it is the only remaining copy of
    // artifacts the rollback could not put back. Deleting it there would be the data loss
    // this whole path exists to prevent.
    if !matches!(result, Err(EmitError::RollbackIncomplete { .. })) {
        let _ = std::fs::remove_dir_all(&stage);
    }
    result?;

    Ok(Artifacts {
        dll: out_dir.join(&names[0]),
        header: out_dir.join(&names[1]),
        delphi_unit: out_dir.join(&names[2]),
        import_lib: out_dir.join(&names[3]),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!(
            "mlc_rbk_{tag}_{}_{}",
            std::process::id(),
            BUILD_SEQ.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn rollback_restores_a_displaced_file_and_reports_nothing_stranded() {
        let d = tmp("ok");
        let dest = d.join("m.dll");
        let backup = d.join("m.dll.prev");
        std::fs::write(&dest, "new").unwrap();
        std::fs::write(&backup, "old").unwrap();

        let mut done = vec![(dest.clone(), Some(backup))];
        assert!(rollback(&mut done).is_empty(), "nothing should be stranded");
        assert_eq!(std::fs::read_to_string(&dest).unwrap(), "old");

        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn rollback_reports_a_backup_it_could_not_put_back() {
        // The destination is a directory, so it cannot be removed to make room — the backup
        // stays in the staging dir and MUST be reported, or deleting that dir loses it.
        let d = tmp("stranded");
        let dest = d.join("m.dll");
        let backup = d.join("m.dll.prev");
        std::fs::create_dir(&dest).unwrap();
        std::fs::write(&backup, "old").unwrap();

        let mut done = vec![(dest, Some(backup.clone()))];
        assert_eq!(
            rollback(&mut done),
            vec![backup.clone()],
            "the un-restorable backup must be reported"
        );
        assert!(backup.is_file(), "and must still exist to be recovered");

        let _ = std::fs::remove_dir_all(&d);
    }

    /// STATUS §5-5.6 said this branch's `Display` had no test. It does now.
    ///
    /// The message is the only instruction a user gets in the one situation where `mlc`
    /// leaves files behind ON PURPOSE, so it has to name the directory and say the files must
    /// be moved back by hand. Getting that wrong turns a recoverable state into a lost one.
    ///
    /// **What is still not covered, and why:** constructing this variant END-TO-END through
    /// `emit_artifacts`. Reaching it needs `rollback` to fail, which needs a destination that
    /// exists and cannot be removed — and every path that fills `done` puts a plain, unlocked
    /// file there. Producing that state from outside would take a race (something locking the
    /// file between the move and the unwind), not a filesystem arrangement. The branch is
    /// defensive; the logic under it is tested directly above.
    #[test]
    fn the_rollback_incomplete_message_names_the_directory_and_the_count() {
        let stage = PathBuf::from("C:/tmp/.mlc-stage-1-0");
        let e = EmitError::RollbackIncomplete {
            source: std::io::Error::new(std::io::ErrorKind::PermissionDenied, "denied"),
            stage_dir: stage.clone(),
            stranded: vec![stage.join("m.dll.prev"), stage.join("m.h.prev")],
        };
        let shown = e.to_string();
        assert!(
            shown.contains(&stage.display().to_string()),
            "the user cannot recover the files without the directory: {shown}"
        );
        assert!(
            shown.contains('2'),
            "the count tells them how many to look for: {shown}"
        );
        assert!(
            shown.contains("by hand"),
            "and that nothing else will do it for them: {shown}"
        );
    }

    #[test]
    fn rollback_without_a_backup_just_drops_what_we_added() {
        let d = tmp("nobackup");
        let dest = d.join("m.h");
        std::fs::write(&dest, "new").unwrap();

        let mut done = vec![(dest.clone(), None)];
        assert!(rollback(&mut done).is_empty());
        assert!(!dest.exists(), "our own file is removed");

        let _ = std::fs::remove_dir_all(&d);
    }
}

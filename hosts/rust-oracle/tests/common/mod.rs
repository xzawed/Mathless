//! Shared test scaffolding: a temp directory that actually removes itself.
//!
//! **Why this exists (measured 2026-09-02).** The helpers in these test files cleared their
//! output directory at the START of a test and never at the end. The directory name carries
//! the process id, so the next `cargo test` got a different name and the old tree stayed —
//! forever. On this development machine that had accumulated **1,720 trees / 976 MB**, at
//! about 13 per full run, while `STATUS.md` §5-5.7 recorded only the CLI's 9.
//!
//! Two things follow, and both are in `TempOut`:
//!
//!   - cleanup happens on **drop**, so it also runs when a test panics;
//!   - removal **retries**, because on Windows a DLL a test just loaded — or one a child
//!     host process was holding — can stay locked for a moment after the handle is dropped,
//!     and a single best-effort `remove_dir_all` silently loses that race. `c_host.rs` did
//!     remove its tree at the end and still leaked, which is how the race was noticed.
#![allow(dead_code)] // not every test binary uses every helper here

use std::path::{Path, PathBuf};

/// A temp directory under the system temp dir, removed when this value is dropped.
///
/// Hold it for as long as the directory is needed: dropping it deletes the tree.
pub struct TempOut(PathBuf);

impl TempOut {
    /// Create `%TEMP%/mlc_<tag>_<pid>`, empty. Any leftover from an earlier run of this same
    /// process id is removed first.
    pub fn new(tag: &str) -> TempOut {
        let dir = std::env::temp_dir().join(format!("mlc_{tag}_{}", std::process::id()));
        let _ = remove_with_retry(&dir);
        std::fs::create_dir_all(&dir).expect("create temp out dir");
        TempOut(dir)
    }

    pub fn path(&self) -> &Path {
        &self.0
    }
}

impl std::ops::Deref for TempOut {
    type Target = Path;
    fn deref(&self) -> &Path {
        &self.0
    }
}

impl std::fmt::Debug for TempOut {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl AsRef<Path> for TempOut {
    fn as_ref(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempOut {
    fn drop(&mut self) {
        // A tree we could not delete must not fail an otherwise green test — but it must not
        // be invisible either, which is exactly how 976 MB accumulated unnoticed.
        if let Err(e) = remove_with_retry(&self.0) {
            eprintln!(
                "warning: leaked temp tree {} ({e}) — see STATUS 5-5",
                self.0.display()
            );
        }
    }
}

/// `remove_dir_all` with a bounded retry. Returns `Ok` if the directory is gone.
fn remove_with_retry(dir: &Path) -> std::io::Result<()> {
    const ATTEMPTS: u32 = 10;
    for attempt in 0..ATTEMPTS {
        match std::fs::remove_dir_all(dir) {
            Ok(()) => return Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(e) if attempt + 1 == ATTEMPTS => return Err(e),
            Err(_) => std::thread::sleep(std::time::Duration::from_millis(50)),
        }
    }
    Ok(())
}

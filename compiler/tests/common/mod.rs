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
        // A tree we could not delete must not fail an otherwise green test.
        //
        // The warning is NOT a reliable alarm, and saying otherwise would be the kind of
        // overclaim this repository keeps catching: cargo captures stderr for a test that
        // passes, so this line is only seen under `--nocapture` or when the test fails. It
        // was measured being swallowed exactly that way. Counting the trees before and after
        // a run is what actually detects a leak; this message only explains one you already
        // suspect.
        if let Err(e) = remove_with_retry(&self.0) {
            eprintln!(
                "warning: leaked temp tree {} ({e}) — see STATUS 5-5",
                self.0.display()
            );
        }
    }
}

/// This file exists **twice** — once per crate's `tests/` tree — because an integration test
/// can only include a module from its own crate, and a shared helper crate would be a new
/// workspace member for four call sites (`CLAUDE.md`: build only what is actually used).
///
/// The duplication is deliberate; the drift is not. This test is what makes it safe: edit one
/// copy and the suite fails until the other matches.
#[test]
fn the_two_copies_of_this_helper_are_identical() {
    let root = workspace_root();
    let a = root.join("compiler/tests/common/mod.rs");
    let b = root.join("hosts/rust-oracle/tests/common/mod.rs");
    let (ta, tb) = (
        std::fs::read_to_string(&a).expect("compiler copy"),
        std::fs::read_to_string(&b).expect("oracle copy"),
    );
    if ta == tb {
        return;
    }
    // Report the first differing LINE, not the two files. `assert_eq!` on the whole text
    // prints both copies in full — several screens of noise for a one-line drift.
    let (mut la, mut lb) = (ta.lines(), tb.lines());
    let mut n = 0;
    loop {
        n += 1;
        match (la.next(), lb.next()) {
            (Some(x), Some(y)) if x == y => continue,
            (x, y) => panic!(
                "the two copies of tests/common/mod.rs have diverged at line {n}:\n  \
                 {}\n    {:?}\n  {}\n    {:?}\n\
                 Copy one over the other; they are meant to be byte-identical.",
                a.display(),
                x.unwrap_or("<end of file>"),
                b.display(),
                y.unwrap_or("<end of file>"),
            ),
        }
    }
}

/// Walk up from this crate's manifest dir until the directory holding `Cargo.lock`.
fn workspace_root() -> PathBuf {
    let mut dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    while !dir.join("Cargo.lock").is_file() {
        assert!(dir.pop(), "no Cargo.lock above CARGO_MANIFEST_DIR");
    }
    dir
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

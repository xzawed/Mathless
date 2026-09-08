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

/// Run a child to completion, or kill it and say so.
///
/// **This is the workspace's only instrument that can tell "slow" from "never returns".**
/// Before it there was none: a `grep` for `Duration|timeout|try_wait|recv_timeout` across
/// every test file returned one hit, and that one was a retry sleep. The consequence is not
/// a missing test but a blind spot with a shape — every generated module ships
/// `#[panic_handler] fn ml_panic(_) -> ! { loop {} }`, so the module's universal failure
/// channel is an infinite loop in the CALLING HOST'S thread with the process still alive
/// (`STATUS.md` §5-4). A test that reproduced one would hang CI instead of failing it, which
/// is why two fixes in this repository settled for asserting emitted text and said so in
/// their own comments (`compiler/tests/i32_type.rs`, `compiler/tests/string_concat.rs`).
///
/// Every acceptance gate spawns children through one helper, so putting the deadline there
/// converts that whole class from "hangs the job" into "fails the test with a message".
///
/// The deadline is a LIVENESS assertion, not a performance one. It is set far above anything
/// these gates take — the slowest, which builds 19 DLLs and runs a host over them, measured
/// about 210 s end to end while each individual child is seconds — so it cannot fire on a
/// slow machine and mean nothing. If it ever fires, something stopped making progress.
///
/// Output is drained on threads rather than after waiting, because a child that fills a pipe
/// blocks forever and the deadline would then be measuring our own deadlock.
///
/// **What is and is not break-tested, stated rather than left to be discovered.** The helper
/// itself is: `the_liveness_deadline_actually_kills_a_child_that_never_returns` runs a child
/// that never returns and watches it get killed, and runs one that does and watches its
/// output survive. The **wiring** is not — putting a bare `.output()` back into
/// `run_in_msvc_env` was measured to fail no test, because nothing asserts which call the
/// gates go through. Closing that would need either a source-text assertion (the weak shape
/// this repository keeps moving away from) or a real hang in a gate, which is the thing being
/// prevented. It is left open, and written down here instead of assumed.
pub fn output_with_deadline(
    mut cmd: std::process::Command,
    deadline: std::time::Duration,
    what: &str,
) -> std::process::Output {
    use std::io::Read as _;
    use std::process::Stdio;

    let mut child = cmd
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap_or_else(|e| panic!("spawn {what}: {e}"));

    let mut out_pipe = child.stdout.take().expect("piped stdout");
    let mut err_pipe = child.stderr.take().expect("piped stderr");
    let out_reader = std::thread::spawn(move || {
        let mut b = Vec::new();
        let _ = out_pipe.read_to_end(&mut b);
        b
    });
    let err_reader = std::thread::spawn(move || {
        let mut b = Vec::new();
        let _ = err_pipe.read_to_end(&mut b);
        b
    });

    let start = std::time::Instant::now();
    let status = loop {
        match child.try_wait().expect("try_wait") {
            Some(s) => break s,
            None if start.elapsed() >= deadline => {
                let _ = child.kill();
                let _ = child.wait();
                panic!(
                    "{what} made no progress for {}s and was killed. This is the liveness \
                     deadline, not a slow-machine timeout: nothing in these gates takes more \
                     than a few seconds per child. A generated module that reaches its panic \
                     handler spins in `loop {{}}` and hangs the calling host thread with the \
                     process alive (STATUS §5-4) — that is what this looks like from outside.",
                    deadline.as_secs()
                );
            }
            None => std::thread::sleep(std::time::Duration::from_millis(25)),
        }
    };

    std::process::Output {
        status,
        stdout: out_reader.join().unwrap_or_default(),
        stderr: err_reader.join().unwrap_or_default(),
    }
}

/// The liveness deadline for one child in an acceptance gate.
pub const CHILD_DEADLINE: std::time::Duration = std::time::Duration::from_secs(600);

/// The modules `hosts/delphi-host/host.dpr` imports, read from its own `uses` clause.
///
/// Two tests build that host -- the Delphi gate and the Free Pascal one -- and each used to
/// carry its own hand-written list of examples to emit. They drifted the moment a unit was
/// added: `shapes` went into the host and into the FPC test, and the Delphi test kept
/// emitting three. **Nothing caught it, because that gate cannot run on this machine** -- a
/// gate that is blocked cannot tell you it is also broken.
///
/// So the list is derived from the host instead of repeated beside it. Adding a `uses` entry
/// now emits that example in both tests, and naming one with no `examples/<name>.mls` fails
/// loudly rather than at some later Pascal compile.
pub fn delphi_host_units() -> Vec<String> {
    // workspace_root, not CARGO_MANIFEST_DIR: this file exists twice, once per crate, and a
    // relative path from the manifest would resolve to two different places. The identity
    // test requires the copies to be byte-identical, so the path has to be too.
    let dpr = workspace_root()
        .join("hosts")
        .join("delphi-host")
        .join("host.dpr");
    let text =
        std::fs::read_to_string(&dpr).unwrap_or_else(|e| panic!("read {}: {e}", dpr.display()));

    let after = text
        .split_once(
            "
uses",
        )
        .unwrap_or_else(|| panic!("no `uses` clause in {}", dpr.display()))
        .1;
    let clause = after
        .split_once(';')
        .unwrap_or_else(|| panic!("unterminated `uses` clause in {}", dpr.display()))
        .0;

    let units: Vec<String> = clause
        .split(',')
        .map(|u| u.trim().to_string())
        // SysUtils is the RTL, not one of ours.
        .filter(|u| !u.is_empty() && u != "SysUtils")
        .collect();
    assert!(
        !units.is_empty(),
        "parsed no module units out of the `uses` clause in {}",
        dpr.display()
    );
    units
}

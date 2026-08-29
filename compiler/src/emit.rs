//! STEP 1 (Gate-D prep): package a `.mls` module into the three consumable artifacts on
//! disk — `<name>.dll` (native module), `<name>.h` (C header), `<name>.pas` (Delphi import
//! unit). This is the library entrypoint behind the `mlc build` CLI (`src/main.rs`).
//!
//! Honesty split (Grok cross-check #3): producing the `.dll` and *loading it via the Rust
//! oracle* is E2 (measured — see `hosts/rust-oracle/tests/emit_artifacts.rs`). The `.h`/
//! `.pas` are generated text whose real host-load — a C compiler consuming the header, or
//! Delphi consuming the unit — stays **BLOCKED** (no `cl`/`gcc`/`dcc64` on the build
//! machine). The generated files carry that DRAFT/BLOCKED note verbatim; nothing here
//! claims the bindings "work" against a real host.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::{codegen, compile_to_ir, header, CompileError};

/// The three files [`emit_artifacts`] writes, by `out_dir`-joined path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Artifacts {
    /// The native module, `<out_dir>/<module>.dll`.
    pub dll: PathBuf,
    /// The C ABI header, `<out_dir>/<module>.h`.
    pub header: PathBuf,
    /// The Delphi import unit, `<out_dir>/<module>.pas`.
    pub delphi_unit: PathBuf,
}

/// Why packaging failed: the source did not compile, or an I/O step (build, copy, write)
/// failed.
#[derive(Debug)]
pub enum EmitError {
    Compile(CompileError),
    Io(std::io::Error),
}

impl std::fmt::Display for EmitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            // `CompileError` names its own stage and position, so don't re-prefix it.
            EmitError::Compile(e) => write!(f, "{e}"),
            EmitError::Io(e) => write!(f, "io error: {e}"),
        }
    }
}

impl std::error::Error for EmitError {}

impl From<CompileError> for EmitError {
    fn from(e: CompileError) -> Self {
        EmitError::Compile(e)
    }
}

impl From<std::io::Error> for EmitError {
    fn from(e: std::io::Error) -> Self {
        EmitError::Io(e)
    }
}

static BUILD_SEQ: AtomicU64 = AtomicU64::new(0);

/// A private, process-unique build root under the OS temp dir, so two concurrent
/// `emit_artifacts` calls never share a build directory (fixed-name race). Removed by
/// [`emit_artifacts`] once the DLL is copied out.
fn unique_build_root() -> PathBuf {
    let n = BUILD_SEQ.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!("mlc-build-{}-{n}", std::process::id()))
}

/// Compile `src` and write the three deliverables for `module_name` into `out_dir`,
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
    // Front + middle end: source → typed IR (feeds both the DLL and the bindings).
    let ir = compile_to_ir(src)?;

    // Back end: IR → extern "C" Rust → cdylib DLL, in a private self-cleaning build tree.
    let rust = codegen::emit(&ir).map_err(|e| EmitError::Compile(CompileError::Codegen(e)))?;

    // Build the DLL in a private temp tree and copy it out. The tree is removed whether the
    // build/copy succeeds OR fails, so an error path never leaks a temp directory (Grok
    // verify: temp-leak on error). `remove_dir_all` failure is ignored (a loaded/locked
    // file on Windows shouldn't fail the build).
    let build_root = unique_build_root();
    let dll = out_dir.join(format!("{module_name}.dll"));
    let build_and_copy = || -> Result<(), EmitError> {
        let built = codegen::build_cdylib(&rust, module_name, &build_root)
            .map_err(|e| EmitError::Compile(CompileError::Codegen(e)))?;
        std::fs::create_dir_all(out_dir)?;
        std::fs::copy(&built, &dll)?;
        Ok(())
    };
    let result = build_and_copy();
    let _ = std::fs::remove_dir_all(&build_root);
    result?;

    // Bindings (DRAFT: not host-load-verified — D14 gate BLOCKED). Delphi requires the
    // unit name to equal the file stem, so the unit name IS the module name.
    let header = out_dir.join(format!("{module_name}.h"));
    std::fs::write(&header, header::emit_c_header(&ir, module_name))?;
    let delphi_unit = out_dir.join(format!("{module_name}.pas"));
    std::fs::write(
        &delphi_unit,
        header::emit_delphi_unit(&ir, module_name, module_name),
    )?;

    Ok(Artifacts {
        dll,
        header,
        delphi_unit,
    })
}

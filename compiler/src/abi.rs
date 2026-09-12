//! D18 module ABI contract — the single source of truth for the ABI version.
//!
//! Codegen interpolates [`ML_MODULE_ABI_VERSION`] into every emitted module's
//! `ml_module_abi_version()`, and the oracle tests assert against the same constant, so
//! there is exactly one place to change. Hosts are required to reject a **major** mismatch
//! (D18). The reference C host does reject, on every module it loads (`hosts/c-host`,
//! acceptance D); for a third-party host it stays a contract, since nothing in the emitted
//! module enforces it and the oracle only asserts the value is equal.

/// The ABI version every Mathless module exports as `ml_module_abi_version() -> u32`.
///
/// Bump this — and document the change — only when the module ABI changes incompatibly.
pub const ML_MODULE_ABI_VERSION: u32 = 1;

/// D17 reserves the NEGATIVE status space for runtime and ABI conditions, as opposed to the
/// positive codes a module declares with `error NAME = N`.
///
/// Both reserved negatives live here, for the same reason: codegen and the generated
/// bindings have to emit the same number, and a literal in each file is how they drift.
/// Q12's truncation status: the host's buffer is too small, nothing was written, and
/// `*ml_needed` says how much room is needed.
///
/// It lived as a literal `(-1)` in `header.rs` and nowhere else, which was fine while only
/// the generated bindings mentioned it. The array return made codegen emit it too, and the
/// note below already said what that costs -- so it moved here before the two could drift,
/// rather than after.
pub const ML_ST_INSUFFICIENT_BUFFER: i32 = -1;

pub const ML_ST_INDEX_OUT_OF_RANGE: i32 = -2;

/// Longest module name the compiler accepts, in ASCII characters.
///
/// The module name is not just a file stem: it becomes the crate name, the C header guard,
/// the Delphi unit name, and — since `SPEC-qualified-iface-hash` — **the suffix of a
/// reserved export**, `ml_iface_hash_<module>`. That last one is why a bound exists at all.
///
/// A DYNAMIC host has to build that symbol name from the module it is loading, so it needs a
/// buffer, and a buffer has an edge. Measured before this constant existed: the reference C
/// host could gate a 65-character name and refused at 66, while `mlc build` accepted 70 and
/// exited 0 — the compiler produced a module its own reference host could not gate
/// (`SPEC-module-name-length` §0.1).
///
/// **The direction matters more than the number.** The compiler sets the bound and the host
/// sizes its buffer from it; doing it the other way round would make one host's
/// implementation detail into the language's contract. `hosts/c-host/host.c` derives its
/// buffer from this value and `doc_claims.rs` fails if the two ever disagree.
///
/// The value itself is arbitrary and is written down as arbitrary: 64 is a round number that
/// fits comfortably inside what the reference host already handled, and it is far above
/// anything a real module is named — the longest example in this repository is
/// `count_bounded`, at 13.
pub const ML_MAX_MODULE_NAME: usize = 64;

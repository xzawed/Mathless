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

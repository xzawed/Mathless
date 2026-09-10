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
/// `-1` is `ML_ST_INSUFFICIENT_BUFFER`, spelled in `header.rs` where the two bindings are
/// written. This one is next, and it lives here because codegen has to emit the same number
/// the header promises — the two were literals in two files for `-1`, which is the drift this
/// constant exists to prevent for `-2`.
pub const ML_ST_INDEX_OUT_OF_RANGE: i32 = -2;

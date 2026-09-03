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

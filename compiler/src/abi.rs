//! D18 module ABI contract — the single source of truth for the ABI version.
//!
//! Codegen interpolates [`ML_MODULE_ABI_VERSION`] into every emitted module's
//! `ml_module_abi_version()`, and the oracle tests assert against the same constant, so
//! there is exactly one place to change. A host rejects a **major** mismatch (D18).

/// The ABI version every Mathless module exports as `ml_module_abi_version() -> u32`.
///
/// Bump this — and document the change — only when the module ABI changes incompatibly.
pub const ML_MODULE_ABI_VERSION: u32 = 1;

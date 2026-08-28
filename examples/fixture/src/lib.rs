//! W1 fixture module — a hand-written stand-in for `examples/discount.mls`'s
//! eventual compiler output. C-ABI exports per D18: `mlx_` user prefix + reserved
//! `ml_module_abi_version`.
//!
//! Mirrors `examples/discount.mls`: vip → 10% off, else full price.

/// vip → `price * 0.9`, otherwise `price`. Matches `examples/discount.mls`.
#[no_mangle]
pub extern "C" fn mlx_discount(price: f64, vip: bool) -> f64 {
    if vip {
        price * 0.9
    } else {
        price
    }
}

/// Reserved ABI-version symbol queried by the host (D18).
///
/// Hand-written literal, kept in sync with `mlc::ML_MODULE_ABI_VERSION` by convention (this
/// fixture crate deliberately does not depend on `mlc`). `loads_fixture` asserts the two match.
#[no_mangle]
pub extern "C" fn ml_module_abi_version() -> u32 {
    1
}

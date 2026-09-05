//! W7 acceptance (WBS W7): generate a C header + Delphi import unit that match the
//! module's D18 ABI. These tests check the generated *text*. The C header is separately
//! compiled and used for real by `hosts/rust-oracle/tests/c_host.rs` (acceptance D); the
//! Delphi unit still has no host — there is no `dcc64` here.

use mlc::compile_to_ir;
use mlc::header::{emit_c_header, emit_delphi_unit};
use mlc::ir::IrModule;

fn discount_ir() -> IrModule {
    compile_to_ir(include_str!("../../examples/discount.mls")).unwrap()
}

#[test]
fn c_header_matches_module_abi() {
    let h = emit_c_header(&discount_ir(), "discount");
    assert!(h.contains("#ifndef ML_DISCOUNT_H"), "{h}");
    assert!(h.contains("#include <stdbool.h>"), "{h}");
    assert!(h.contains(r#"extern "C""#), "{h}");
    assert!(h.contains("uint32_t ml_module_abi_version(void);"), "{h}");
    assert!(
        h.contains("double mlx_discount(double /* price */, bool /* vip */);"),
        "{h}"
    );
}

#[test]
fn delphi_unit_matches_module_abi() {
    let u = emit_delphi_unit(&discount_ir(), "Mlx_Discount", "discount");
    assert!(u.contains("unit Mlx_Discount;"), "{u}");
    assert!(u.contains("ML_MODULE = 'discount.dll';"), "{u}");
    assert!(
        u.contains("function ml_module_abi_version: LongWord; cdecl; external ML_MODULE;"),
        "{u}"
    );
    assert!(
        u.contains(
            "function mlx_discount(price: Double; vip: Boolean): Double; cdecl; external ML_MODULE;"
        ),
        "{u}"
    );
    assert!(u.trim_end().ends_with("end."), "{u}");
}

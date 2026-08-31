//! rounding slice — acceptance A/B/B2/B3/C (E2).
//!
//! Section 3-B3 is the one that matters. DP-R3 says the builtins match C exactly, and "exactly"
//! includes things `==` cannot see: NaN compares unequal to itself, and `-0.0 == 0.0` is true.
//! So the comparison here is on BITS, against `std`'s own implementations — which is also the
//! only honest way to claim a `no_std` reimplementation is correct.
#![cfg(windows)]

use ml_oracle::{pe, Module};
use mlc::emit::emit_artifacts;

fn build(tag: &str) -> (std::path::PathBuf, Module) {
    let src = include_str!("../../../examples/deduction.mls");
    let out = std::env::temp_dir().join(format!("mlc_rnd_{tag}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&out);
    std::fs::create_dir_all(&out).unwrap();
    let arts = emit_artifacts(src, "deduction", &out).expect("emit deduction");
    let m = Module::load(arts.dll.to_str().unwrap()).expect("load deduction.dll");
    (out, m)
}

/// Every input worth arguing about, including the two that a naive implementation gets wrong.
const CASES: &[f64] = &[
    2.4,
    2.5,
    2.6,
    -2.4,
    -2.5,
    -2.6,
    0.0,
    -0.0,
    // The classic trap: `floor(x + 0.5)` returns 1 here, and the correct answer is 0.
    0.499_999_999_999_999_94,
    -0.499_999_999_999_999_94,
    135_000.0,
    555_555.9,
    // Where the old `as i32` workaround died.
    2_250_000_000.0,
    50_000_000_000.7,
    // 2^53: at and above this every f64 is already an integer.
    9_007_199_254_740_992.0,
    f64::INFINITY,
    f64::NEG_INFINITY,
    f64::NAN,
];

#[test]
fn the_builtins_match_std_bit_for_bit() {
    let (out, m) = build("bits");
    let sym = |n: &[u8]| -> extern "C" fn(f64) -> f64 {
        unsafe { std::mem::transmute(m.symbol(n).unwrap()) }
    };
    let fl = sym(b"mlx_fl\0");
    let ce = sym(b"mlx_ce\0");
    let ro = sym(b"mlx_ro\0");
    let tr = sym(b"mlx_tr\0");

    for &x in CASES {
        // Bits, not values: `-0.0 == 0.0` is true and `NaN == NaN` is false, so `==` would
        // both hide a real difference and invent a fake one.
        assert_eq!(
            fl(x).to_bits(),
            x.floor().to_bits(),
            "floor({x}) — ours {:x}, std {:x}",
            fl(x).to_bits(),
            x.floor().to_bits()
        );
        assert_eq!(ce(x).to_bits(), x.ceil().to_bits(), "ceil({x})");
        assert_eq!(ro(x).to_bits(), x.round().to_bits(), "round({x})");
        assert_eq!(tr(x).to_bits(), x.trunc().to_bits(), "trunc({x})");
    }

    drop(m);
    let _ = std::fs::remove_dir_all(&out);
}

#[test]
fn the_saturation_that_motivated_the_slice_is_gone() {
    // Section 3-B2. These are the measured numbers from the SPEC: the `as i32` workaround
    // returned 2,147,483,647 for both of the last two.
    let (out, m) = build("sat");
    let deduction: extern "C" fn(f64) -> f64 =
        unsafe { std::mem::transmute(m.symbol(b"mlx_deduction\0").unwrap()) };

    assert_eq!(deduction(3_000_000.0), 135_000.0);
    assert_eq!(deduction(12_345_678.0), 555_555.0);
    assert_eq!(deduction(47_000_000_000.0), 2_115_000_000.0);

    let big = deduction(50_000_000_000.0);
    assert_eq!(big, 2_250_000_000.0, "the old workaround gave 2147483647");
    assert_ne!(big, 2_147_483_647.0, "explicitly not the saturated value");
    assert_eq!(deduction(1_000_000_000_000.0), 45_000_000_000.0);

    drop(m);
    let _ = std::fs::remove_dir_all(&out);
}

#[test]
fn the_export_surface_is_unchanged_in_shape() {
    let (out, m) = build("prot");
    drop(m);
    let dll = out.join("deduction.dll");
    let mut names = pe::read_exports(&dll).expect("read exports");
    names.sort();
    assert_eq!(
        names,
        vec![
            "ml_module_abi_version".to_string(),
            "mlx_ce".to_string(),
            "mlx_deduction".to_string(),
            "mlx_fl".to_string(),
            "mlx_ro".to_string(),
            "mlx_tr".to_string(),
        ],
        "the helpers must NOT be exported — they are internal `ml_` functions"
    );
    let size = std::fs::metadata(&dll).unwrap().len();
    println!("deduction.dll = {size} B");
    assert!(size < 60_000, "still a small stripped module: {size}");

    let _ = std::fs::remove_dir_all(&out);
}

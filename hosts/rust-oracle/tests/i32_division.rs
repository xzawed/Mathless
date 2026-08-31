//! i32-division slice — acceptance A/B/B2/C (E2).
//!
//! The point of this file is section 3-B2. `/` and `%` are TOTAL (SPEC-i32-division DP-D1),
//! and "total" is not a claim you can make from reading codegen: the two edge cases are
//! exactly the ones where Rust's own operators panic, and a panic in a generated module does
//! not crash — it spins in `ml_panic`'s `loop {}` and hangs the calling thread (STATUS 5-4).
//! So every call below both RETURNS and returns the specified value. A hang here would show
//! up as a test that never finishes, which is the failure this slice exists to prevent.
#![cfg(windows)]

use ml_oracle::{pe, Module};
use mlc::emit::emit_artifacts;

fn build(name: &str) -> (std::path::PathBuf, Module) {
    let src = include_str!("../../../examples/pack.mls");
    let out = std::env::temp_dir().join(format!("mlc_{name}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&out);
    std::fs::create_dir_all(&out).unwrap();
    let arts = emit_artifacts(src, "pack", &out).expect("emit pack");
    let m = Module::load(arts.dll.to_str().unwrap()).expect("load pack.dll");
    (out, m)
}

#[test]
fn division_and_remainder_match_integer_semantics() {
    let (out, m) = build("div");
    let boxes: extern "C" fn(i32, i32) -> i32 =
        unsafe { std::mem::transmute(m.symbol(b"mlx_boxes\0").unwrap()) };
    let loose: extern "C" fn(i32, i32) -> i32 =
        unsafe { std::mem::transmute(m.symbol(b"mlx_loose\0").unwrap()) };

    // Section 3-B. Truncation is toward zero and the remainder takes the dividend's sign
    // (DP-D3), which is what C, Rust and Delphi all do.
    assert_eq!(boxes(17, 5), 3);
    assert_eq!(loose(17, 5), 2);
    assert_eq!(boxes(-17, 5), -3);
    assert_eq!(loose(-17, 5), -2);
    assert_eq!(boxes(17, -5), -3);
    assert_eq!(loose(17, -5), 2);
    assert_eq!(boxes(100, 10), 10);
    assert_eq!(loose(100, 10), 0);
    assert_eq!(boxes(7, 8), 0);
    assert_eq!(loose(7, 8), 7);
    assert_eq!(boxes(2147483647, 3), 715827882);

    drop(m);
    let _ = std::fs::remove_dir_all(&out);
}

#[test]
fn the_two_edges_return_instead_of_hanging() {
    let (out, m) = build("edge");
    let boxes: extern "C" fn(i32, i32) -> i32 =
        unsafe { std::mem::transmute(m.symbol(b"mlx_boxes\0").unwrap()) };
    let loose: extern "C" fn(i32, i32) -> i32 =
        unsafe { std::mem::transmute(m.symbol(b"mlx_loose\0").unwrap()) };

    // Section 3-B2, edge 1: a zero divisor. Rust's `i32 /` panics here; ours is defined as 0.
    // Before this slice the f64 round-trip returned 2147483647 for boxes(17, 0) — a plausible
    // number, which is the worse kind of wrong answer.
    assert_eq!(boxes(17, 0), 0);
    assert_eq!(boxes(-17, 0), 0);
    assert_eq!(boxes(0, 0), 0);
    assert_eq!(loose(17, 0), 0);
    assert_eq!(loose(-17, 0), 0);

    // Edge 2: i32::MIN / -1. Rust's operator panics on this one even in a release build —
    // division overflow is not governed by `overflow-checks`. Ours wraps, matching the
    // measured `-i32::MIN == i32::MIN` from the unary slice, and the f64 round-trip's
    // saturation to i32::MAX is NOT what we do.
    assert_eq!(boxes(i32::MIN, -1), i32::MIN);
    assert_eq!(loose(i32::MIN, -1), 0);
    assert_ne!(boxes(i32::MIN, -1), i32::MAX, "not the f64 saturation");

    drop(m);
    let _ = std::fs::remove_dir_all(&out);
}

#[test]
fn the_boundary_pattern_reports_a_domain_error() {
    // Section 4: total operators do not take the fallible route away — they leave it to the
    // caller, at the export where D17's `fail` is legal.
    let (out, m) = build("checked");
    let checked: extern "C" fn(i32, i32, *mut i32) -> i32 =
        unsafe { std::mem::transmute(m.symbol(b"mlx_boxes_checked\0").unwrap()) };

    let mut v = -999i32;
    assert_eq!(checked(17, 5, &mut v), 0, "status 0 on success");
    assert_eq!(v, 3);

    let mut untouched = -999i32;
    assert_eq!(checked(17, 0, &mut untouched), 1, "E_EMPTY_BOX");
    assert_eq!(untouched, -999, "DP-E3: out-param untouched on failure");

    drop(m);
    let _ = std::fs::remove_dir_all(&out);
}

#[test]
fn the_export_surface_is_unchanged_in_shape() {
    // Section 3-C. The slice adds functions to this example, so the count is 3 + the ABI
    // symbol — what matters is that nothing else leaked and the module is still stripped.
    let (out, m) = build("prot");
    drop(m);
    let dll = out.join("pack.dll");
    let names = pe::read_exports(&dll).expect("read exports");
    assert!(
        names.contains(&"ml_module_abi_version".to_string()),
        "{names:?}"
    );
    assert!(names.contains(&"mlx_boxes".to_string()), "{names:?}");
    assert!(names.contains(&"mlx_loose".to_string()), "{names:?}");
    assert!(
        names.contains(&"mlx_boxes_checked".to_string()),
        "{names:?}"
    );
    assert_eq!(names.len(), 4, "nothing else may be exported: {names:?}");
    // The size proxy moves with the added guard; record it rather than pin it (STATUS 5-5).
    let size = std::fs::metadata(&dll).unwrap().len();
    println!("pack.dll = {size} B");
    assert!(size < 60_000, "still a small stripped module: {size}");

    let _ = std::fs::remove_dir_all(&out);
}

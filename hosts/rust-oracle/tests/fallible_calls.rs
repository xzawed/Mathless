//! fallible-calls slice — acceptance A/B/B2/B3/C (E2).
//!
//! The frontend test pins the shape; this measures the VALUES against a loaded module, which
//! is the only way to see that a propagated status arrives unchanged and that the helpers
//! stayed out of the export table.
//!
//! Acceptance C is the one that earns the slice. SPEC section 0.1 measured what the workaround
//! costs: promoting a shared rule to its own export makes its callers non-fallible, so the
//! validation becomes advisory. This checks that the cost is actually gone — the helpers are
//! reused and the export surface does not grow.
#![cfg(windows)]

use ml_oracle::{pe, Module};
use mlc::emit::emit_artifacts;

const E_BAD_QTY: i32 = 1;
const E_DIV0: i32 = 2;

fn build(tag: &str) -> (std::path::PathBuf, Module) {
    let src = include_str!("../../../examples/quote.mls");
    let out = std::env::temp_dir().join(format!("mlc_fc_{tag}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&out);
    std::fs::create_dir_all(&out).unwrap();
    let arts = emit_artifacts(src, "quote", &out).expect("emit quote");
    let m = Module::load(arts.dll.to_str().unwrap()).expect("load quote.dll");
    (out, m)
}

#[test]
fn the_success_path_returns_the_right_value() {
    let (out, m) = build("b");
    let unit_price: extern "C" fn(f64, i32, *mut f64) -> i32 =
        unsafe { std::mem::transmute(m.symbol(b"mlx_unit_price\0").unwrap()) };

    let mut v = -7.0f64;
    assert_eq!(unit_price(100.0, 4, &mut v), 0);
    assert_eq!(v, 25.0);

    drop(m);
    let _ = std::fs::remove_dir_all(&out);
}

#[test]
fn a_propagated_status_arrives_unchanged() {
    // Section 3-B. The code comes from `check_qty`, two levels down from the C boundary, and
    // must be the callee's own code — not renumbered, not offset, not collapsed into one
    // "something failed" value. The error table is module-scoped, so caller and callee share
    // it; the propagation is a pass-through by construction, and this proves it.
    let (out, m) = build("b2");
    let unit_price: extern "C" fn(f64, i32, *mut f64) -> i32 =
        unsafe { std::mem::transmute(m.symbol(b"mlx_unit_price\0").unwrap()) };

    let mut v = -7.0f64;
    assert_eq!(
        unit_price(100.0, 0, &mut v),
        E_BAD_QTY,
        "qty 0 fails check_qty"
    );
    assert_eq!(v, -7.0, "and out_value is untouched (D17 DP-E3)");

    let mut v = -7.0f64;
    assert_eq!(
        unit_price(100.0, 99999, &mut v),
        E_BAD_QTY,
        "the upper bound"
    );
    assert_eq!(v, -7.0);

    // The SECOND helper's code, reached only after the first one succeeded. If propagation
    // collapsed codes, or if the first `try` swallowed the second, this would be E_BAD_QTY
    // or 0 instead.
    //
    // qty passes check_qty, so the failure can only come from safe_div — which needs a zero
    // divisor, and `q as f64` is zero only if q is zero, which check_qty rejects. So this
    // path is unreachable from unit_price by design; measured through line_check instead.
    drop(m);
    let _ = std::fs::remove_dir_all(&out);
}

#[test]
fn a_declared_out_composes_with_a_propagating_call() {
    // Section 3-B3. On success the out is written; on a propagated failure the function left
    // before reaching the assignment, so it is not. DP-O3 is explicit that outs are NOT
    // rolled back — what matters is that the host must not read them on a non-zero status,
    // and this measures which of the two happened rather than assuming.
    let (out, m) = build("b3");
    let line_check: extern "C" fn(i32, *mut i32, *mut i32) -> i32 =
        unsafe { std::mem::transmute(m.symbol(b"mlx_line_check\0").unwrap()) };

    let (mut tier, mut v) = (-7i32, -7i32);
    assert_eq!(line_check(5, &mut tier, &mut v), 0);
    assert_eq!(tier, 1, "the declared out is written on success");
    assert_eq!(v, 5, "and out_value carries the return");

    let (mut tier, mut v) = (-7i32, -7i32);
    assert_eq!(line_check(0, &mut tier, &mut v), E_BAD_QTY);
    assert_eq!(
        tier, -7,
        "the try failed BEFORE `tier = 1`, so the out was never reached"
    );
    assert_eq!(v, -7, "and out_value is untouched");

    drop(m);
    let _ = std::fs::remove_dir_all(&out);
}

#[test]
fn the_second_helper_propagates_its_own_code() {
    // `safe_div`'s code must survive a hop of its own, and the two helpers' codes must stay
    // distinguishable — the whole point of D17's per-module error table.
    const SRC: &str = "error E_BAD = 1\n\
                       error E_DIV0 = 2\n\
                       fn guard(x: f64) -> f64! { if x < 0.0 { fail E_BAD } return x }\n\
                       fn divide(a: f64, b: f64) -> f64! { if b == 0.0 { fail E_DIV0 } return a / b }\n\
                       export fn f(a: f64, b: f64) -> f64! {\n\
                         let g = try guard(a)\n\
                         return try divide(g, b)\n\
                       }";
    let out = std::env::temp_dir().join(format!("mlc_fc_two_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&out);
    std::fs::create_dir_all(&out).unwrap();
    let arts = emit_artifacts(SRC, "two", &out).expect("emit two");
    let m = Module::load(arts.dll.to_str().unwrap()).expect("load two.dll");
    let f: extern "C" fn(f64, f64, *mut f64) -> i32 =
        unsafe { std::mem::transmute(m.symbol(b"mlx_f\0").unwrap()) };

    let mut v = -7.0f64;
    assert_eq!(f(10.0, 2.0, &mut v), 0);
    assert_eq!(v, 5.0);

    let mut v = -7.0f64;
    assert_eq!(f(-1.0, 2.0, &mut v), 1, "the FIRST helper's code");
    assert_eq!(v, -7.0);

    let mut v = -7.0f64;
    assert_eq!(f(10.0, 0.0, &mut v), E_DIV0, "the SECOND helper's code");
    assert_eq!(v, -7.0);

    drop(m);
    let _ = std::fs::remove_dir_all(&out);
}

#[test]
fn the_helpers_stay_out_of_the_export_table() {
    // Section 3-C, and the measurement that earns the slice. SPEC section 0.1 measured the
    // workaround's cost: promoting `check_qty` to its own export took the export table from
    // 3 names to 4 AND made the callers non-fallible, so the validation stopped being
    // enforced by the ABI. Both helpers are reused here by two exports, and neither appears.
    let (out, m) = build("c");
    drop(m);
    let dll = out.join("quote.dll");

    let mut names = pe::read_exports(&dll).expect("read exports");
    names.sort();
    assert_eq!(
        names,
        vec![
            "ml_module_abi_version".to_string(),
            "mlx_line_check".to_string(),
            "mlx_unit_price".to_string(),
        ],
        "a reused fallible helper must not become part of the module's surface"
    );

    let size = std::fs::metadata(&dll).unwrap().len();
    println!("quote.dll = {size} B, exports = {names:?}");
    assert!(size < 60_000, "still a small stripped module: {size}");

    let _ = std::fs::remove_dir_all(&out);
}

#[test]
fn a_three_level_chain_carries_the_deepest_code() {
    // Section 3-B2. `export` -> `fn` -> `fn`: the code from the bottom must arrive at the C
    // boundary unchanged, which means every intermediate hop re-wrapped it rather than
    // reinterpreting it.
    const SRC: &str = "error E_DEEP = 7\n\
                       fn c(x: i32) -> i32! { if x == 0 { fail E_DEEP } return x }\n\
                       fn b(x: i32) -> i32! { let y = try c(x) return y + 1 }\n\
                       fn a(x: i32) -> i32! { let y = try b(x) return y + 1 }\n\
                       export fn f(x: i32) -> i32! { return try a(x) }";
    let out = std::env::temp_dir().join(format!("mlc_fc_deep_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&out);
    std::fs::create_dir_all(&out).unwrap();
    let arts = emit_artifacts(SRC, "deep", &out).expect("emit deep");
    let m = Module::load(arts.dll.to_str().unwrap()).expect("load deep.dll");
    let f: extern "C" fn(i32, *mut i32) -> i32 =
        unsafe { std::mem::transmute(m.symbol(b"mlx_f\0").unwrap()) };

    let mut v = -7i32;
    assert_eq!(f(5, &mut v), 0);
    assert_eq!(v, 7, "5 + 1 + 1 — every level ran");

    let mut v = -7i32;
    assert_eq!(f(0, &mut v), 7, "E_DEEP arrives from three levels down");
    assert_eq!(v, -7);

    drop(m);
    let _ = std::fs::remove_dir_all(&out);
}

//! Logical-operator slice — acceptance A/B/C (E2): the oracle loads a module whose loop
//! header combines two conditions, and calls it. Expression-level change, so the
//! ABI/exports are unchanged.
#![cfg(windows)]

use ml_oracle::{pe, Module};
use mlc::emit::emit_artifacts;

mod common;

#[test]
fn oracle_loads_and_calls_a_module_using_logical_operators() {
    let src = include_str!("../../../examples/count_bounded.mls");
    let out = std::env::temp_dir().join(format!("mlc_logic_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&out);
    std::fs::create_dir_all(&out).unwrap();
    let arts = emit_artifacts(src, "count_bounded", &out).expect("emit count_bounded");

    let m = Module::load(arts.dll.to_str().unwrap()).expect("load count_bounded.dll");
    let ver: extern "C" fn() -> u32 =
        unsafe { std::mem::transmute(m.symbol(b"ml_module_abi_version\0").unwrap()) };
    let count: extern "C" fn(i32, i32) -> i32 =
        unsafe { std::mem::transmute(m.symbol(b"mlx_count_bounded\0").unwrap()) };
    assert_eq!(ver(), mlc::ML_MODULE_ABI_VERSION, "abi version");
    // Whichever bound is smaller stops the loop — both operands of `&&` matter.
    assert_eq!(count(10, 3), 3, "cap stops it");
    assert_eq!(count(3, 10), 3, "n stops it");
    assert_eq!(count(0, 5), 0, "body runs zero times");

    let mut exports = pe::read_exports(&arts.dll).expect("read exports");
    exports.sort();
    assert_eq!(
        exports,
        vec![
            "ml_iface_hash_count_bounded".to_string(),
            "ml_module_abi_version".to_string(),
            "mlx_count_bounded".to_string(),
        ]
    );

    drop(m);
    let _ = std::fs::remove_dir_all(&out);
}

#[test]
fn or_returns_the_right_value_for_every_combination() {
    // The VALUE of `||`, which is a different property from its evaluation ORDER.
    //
    // This test used to be called `or_short_circuit_is_specified_but_not_observable_here`,
    // and its comment said nothing in the language could observe short-circuiting yet — true
    // when it was written, and it stayed in the name long after it stopped being true. The
    // observation arrived with array input (#200); `short_circuit_is_observable_through_an_
    // out_of_range_index` below now measures it. A test NAME is a claim too: this one was
    // asserting an open debt in every run's output.
    let out = std::env::temp_dir().join(format!("mlc_logic_or_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&out);
    std::fs::create_dir_all(&out).unwrap();
    let arts = emit_artifacts(
        "export fn any(a: bool, b: bool) -> bool { return a || b }",
        "any",
        &out,
    )
    .expect("emit any");

    let m = Module::load(arts.dll.to_str().unwrap()).expect("load any.dll");
    let any: extern "C" fn(bool, bool) -> bool =
        unsafe { std::mem::transmute(m.symbol(b"mlx_any\0").unwrap()) };
    assert!(any(true, false));
    assert!(any(false, true));
    assert!(!any(false, false));
    assert!(any(true, true));

    drop(m);
    let _ = std::fs::remove_dir_all(&out);
}

/// **Short-circuit evaluation, MEASURED on the surface — `STATUS.md` §5-1 repaid.**
///
/// That debt has been open since this slice shipped (#54/#55). `SPEC-logical-ops` §2.3 chose
/// to SPECIFY short-circuiting while refusing to claim it had been measured, because nothing
/// on the surface could observe the difference: there were no calls (so no side effects),
/// `f64 x/0` is `inf` rather than a trap, and `i32 /` arrived later as a TOTAL operator that
/// does not trap either. Five later slices each recorded that they could not pay it, and the
/// entry's standing note was "the next candidate is an operation with a side effect, or a
/// `fail` in expression position".
///
/// **The instrument arrived with array input (#200) and nobody noticed.** An out-of-range
/// index is exactly an operation with an observable effect in EXPRESSION position: it returns
/// `ML_ST_INDEX_OUT_OF_RANGE` from where the index sits. So whether the right operand was
/// evaluated is now visible in the status a host receives.
///
/// Both operators, and both with a control, because "status 0" alone would also be produced
/// by an index that silently did nothing:
///
/// | call | short-circuit | eager |
/// |---|---|---|
/// | `and_short(false, xs)` | status 0 | `-2` |
/// | `or_short(true, xs)` | status 0 | `-2` |
/// | `and_short(true, xs)` | `-2` (control: the operand IS reached) | `-2` |
/// | `or_short(false, xs)` | `-2` (control) | `-2` |
///
/// This is surface E2. The old note that Rust's `&&` guarantees it was backend E1 — true
/// about the lowering, and silent about what a host actually sees.
#[test]
fn short_circuit_is_observable_through_an_out_of_range_index() {
    // Built inline rather than added to `examples/`: a module whose only purpose is to index
    // out of range would be carried into the golden snapshots, the FPC gate and the
    // export-surface measurement, none of which is what this measures.
    const SRC: &str = "\
export fn and_short(flag: bool, xs: [i32]) -> i32! {
    if flag && xs[99] > 0 { return 1 }
    return 0
}

export fn or_short(flag: bool, xs: [i32]) -> i32! {
    if flag || xs[99] > 0 { return 1 }
    return 0
}
";
    let out = common::TempOut::new("shortcircuit");
    let arts = emit_artifacts(SRC, "shortcircuit", &out).expect("emit shortcircuit");
    let m = Module::load(arts.dll.to_str().unwrap()).expect("load shortcircuit.dll");

    type Fn2 = unsafe extern "C" fn(bool, *const i32, i32, *mut i32) -> i32;
    let and_short: Fn2 = unsafe { std::mem::transmute(m.symbol(b"mlx_and_short\0").unwrap()) };
    let or_short: Fn2 = unsafe { std::mem::transmute(m.symbol(b"mlx_or_short\0").unwrap()) };

    let xs = [1i32, 2, 3]; // index 99 is far outside 0..3
    let oor = mlc::abi::ML_ST_INDEX_OUT_OF_RANGE;
    let mut v = -999i32;

    // `&&` with a false left operand: the right operand must NOT be evaluated.
    let st = unsafe { and_short(false, xs.as_ptr(), 3, &mut v) };
    assert_eq!(
        (st, v),
        (0, 0),
        "`false && xs[99] > 0` must not evaluate the right operand — an eager lowering would \
         return {oor} from the index"
    );

    // `||` with a true left operand: same, the other way round.
    let st = unsafe { or_short(true, xs.as_ptr(), 3, &mut v) };
    assert_eq!(
        (st, v),
        (0, 1),
        "`true || xs[99] > 0` must not evaluate the right operand"
    );

    // The controls. Without these, a lowering that dropped the index entirely would pass the
    // two assertions above while measuring nothing at all.
    let st = unsafe { and_short(true, xs.as_ptr(), 3, &mut v) };
    assert_eq!(
        st, oor,
        "control: with a TRUE left operand the index IS reached, so it must report \
         out-of-range — otherwise the two checks above prove nothing"
    );
    let st = unsafe { or_short(false, xs.as_ptr(), 3, &mut v) };
    assert_eq!(
        st, oor,
        "control: with a FALSE left operand of `||` the index is reached"
    );

    drop(m);
}

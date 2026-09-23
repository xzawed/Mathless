//! `SPEC-division-guard-operands` — acceptance B/C/D (E2).
//!
//! `#76` made `i32` `/` and `%` total by emitting a guard on the divisor, and the guard put
//! the LEFT operand inside its `else`. Nothing could observe that until `#200` gave
//! expression position something that early-returns: an out-of-range array index reports
//! `ML_ST_INDEX_OUT_OF_RANGE` from where the index sits.
//!
//! So a zero divisor skipped the bounds check, and the host got "success, and the answer is
//! 0". This measures that it does not any more — and the controls are what make the
//! measurement mean anything, because "status -2" alone would also be produced by a lowering
//! that had stopped dividing at all.
#![cfg(windows)]

use ml_oracle::Module;
use mlc::emit::emit_artifacts;

mod common;

/// Built inline rather than added to `examples/`: this module's whole purpose is to index out
/// of range, and the corpus is carried into golden snapshots, the FPC gate, the C++ header
/// gate and the export-surface measurement — none of which is what this measures. Same
/// reasoning as `short_circuit_is_observable_through_an_out_of_range_index`.
const SRC: &str = "\
export fn pick(xs: [i32], i: i32, d: i32) -> i32! {
    return xs[i] / d
}

export fn rest(xs: [i32], i: i32, d: i32) -> i32! {
    return xs[i] % d
}
";

type PickFn = unsafe extern "C" fn(*const i32, i32, i32, i32, *mut i32) -> i32;

#[test]
fn a_zero_divisor_does_not_skip_the_left_operand() {
    let out = common::TempOut::new("divguard");
    let arts = emit_artifacts(SRC, "divguard", &out).expect("emit divguard");
    let m = Module::load(arts.dll.to_str().unwrap()).expect("load divguard.dll");

    let pick: PickFn = unsafe { std::mem::transmute(m.symbol(b"mlx_pick\0").unwrap()) };
    let rest: PickFn = unsafe { std::mem::transmute(m.symbol(b"mlx_rest\0").unwrap()) };

    let xs = [10i32, 20, 30];
    let oor = mlc::abi::ML_ST_INDEX_OUT_OF_RANGE;
    // The sentinel is what makes a failure assertion say something about the out-param. D17
    // promises it is left UNMODIFIED when status is non-zero, and `array_input.rs` treats a
    // write there as a violation — checking only the status would pass a lowering that
    // reported `-2` and scribbled on the host's variable anyway. Grok raised it: the first
    // version of this test read `.0` and never looked at `v`.
    const SENTINEL: i32 = -999;
    let call = |f: PickFn, i: i32, d: i32| {
        let mut v = SENTINEL;
        let st = unsafe { f(xs.as_ptr(), xs.len() as i32, i, d, &mut v) };
        (st, v)
    };

    // The defect. Before the fix both of these were `(0, 0)` — the host was told the call
    // succeeded and the answer was zero, with the index never checked.
    for (label, f) in [("/", pick), ("%", rest)] {
        assert_eq!(
            call(f, 999, 0),
            (oor, SENTINEL),
            "`xs[999] {label} 0`: the divisor guard must not skip the bounds check, and D17 \
             leaves the out-param untouched on failure. A zero divisor still yields 0 \
             (DP-N4), but the left operand is evaluated first and an out-of-range index \
             reports {oor} from where it sits"
        );
        assert_eq!(
            call(f, -1, 0),
            (oor, SENTINEL),
            "`xs[-1] {label} 0`: negative indices are the other side of the same check"
        );
    }

    // Controls. Without these the assertions above would also pass for a lowering that had
    // stopped evaluating the divisor, or stopped dividing, or made every call fail.
    assert_eq!(
        call(pick, 1, 2),
        (0, 10),
        "control: 20 / 2 with a valid index"
    );
    assert_eq!(
        call(rest, 1, 3),
        (0, 2),
        "control: 20 % 3 with a valid index"
    );
    assert_eq!(
        call(pick, 999, 2),
        (oor, SENTINEL),
        "control: the bounds check already fired at a non-zero divisor before this slice — if \
         it stops, the test above is measuring nothing"
    );

    // DP-N4 is untouched: a zero divisor is still a defined value, not a failure.
    assert_eq!(
        call(pick, 1, 0),
        (0, 0),
        "DP-N4: `20 / 0` is `0`, not an error. This slice changes WHEN the left operand is \
         evaluated, not what a zero divisor produces"
    );
    assert_eq!(call(rest, 1, 0), (0, 0), "DP-N4 for `%` as well");

    drop(m);
    let _ = std::fs::remove_dir_all(&out);
}

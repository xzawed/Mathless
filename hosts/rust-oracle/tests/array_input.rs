//! Array-input slice — acceptance B/C/H (E2).
//!
//! The frontend and the emitted text are pinned in `compiler/tests/array_input.rs`. This file
//! asks the other question: does a real host, holding real memory, get the right answers
//! across the C ABI — and does an out-of-range index come back as the reserved negative with
//! the out-param untouched?
//!
//! Every call here passes a pointer into the host's OWN array. That is the whole contract:
//! the module borrows it for the call (D16 rule 1) and never writes to it.
#![cfg(windows)]

use ml_oracle::Module;
use mlc::abi::ML_ST_INDEX_OUT_OF_RANGE;
use mlc::emit::emit_artifacts;

/// The domain code `examples/basket.mls` declares.
const ML_BASKET_ERR_E_LENGTH_MISMATCH: i32 = 1;

fn build(tag: &str) -> (std::path::PathBuf, Module) {
    let src = include_str!("../../../examples/basket.mls");
    let out = std::env::temp_dir().join(format!("mlc_arr_{tag}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&out);
    std::fs::create_dir_all(&out).unwrap();
    let arts = emit_artifacts(src, "basket", &out).expect("emit basket");
    let m = Module::load(arts.dll.to_str().unwrap()).expect("load basket.dll");
    (out, m)
}

type BasketTotal = extern "C" fn(*const i32, i32, *const i32, i32, i32, *mut i32) -> i32;
type Pick = extern "C" fn(*const i32, i32, i32, *mut i32) -> i32;
type Largest = extern "C" fn(*const f64, i32, f64, *mut f64) -> i32;
type HowMany = extern "C" fn(*const bool, i32) -> i32;
type AnySet = extern "C" fn(*const bool, i32, *mut bool) -> i32;

/// Acceptance B: the values a host reads are the values the rule computes — over a whole
/// array, in ONE call.
#[test]
fn a_host_gets_the_right_answer_from_one_call() {
    let (out, m) = build("value");
    let total: BasketTotal =
        unsafe { std::mem::transmute(m.symbol(b"mlx_basket_total\0").unwrap()) };

    // The host's memory, start to finish.
    let qty = [2i32, 1, 4];
    let price = [30i32, 500, 25];
    let mut got = -1i32;
    // 60 (under cap) + 100 (500 capped) + 100 (under cap) = 260.
    let st = total(
        qty.as_ptr(),
        qty.len() as i32,
        price.as_ptr(),
        price.len() as i32,
        100,
        &mut got,
    );
    assert_eq!(st, 0, "status");
    assert_eq!(got, 260);

    // The lengths are two independent numbers, and only the module can compare them — a host
    // that gets them out of step is exactly what this domain error is for.
    let mut untouched = 12345i32;
    let st = total(
        qty.as_ptr(),
        qty.len() as i32,
        price.as_ptr(),
        2,
        100,
        &mut untouched,
    );
    assert_eq!(st, ML_BASKET_ERR_E_LENGTH_MISMATCH);
    assert_eq!(
        untouched, 12345,
        "D17: a failed call writes no out-param, domain error included"
    );
    drop(m);
    let _ = std::fs::remove_dir_all(out);
}

/// Acceptance C: out of range is the reserved NEGATIVE, and it writes nothing.
///
/// Both halves matter and they fail differently. A wrong status is a host reading an error as
/// a value; a written out-param is a host reading garbage it was told to ignore (DP-E3).
#[test]
fn an_out_of_range_index_is_the_reserved_negative_and_writes_nothing() {
    let (out, m) = build("bounds");
    let pick: Pick = unsafe { std::mem::transmute(m.symbol(b"mlx_pick\0").unwrap()) };
    let xs = [10i32, 20, 30];

    let mut got = -1i32;
    assert_eq!(pick(xs.as_ptr(), 3, 2, &mut got), 0);
    assert_eq!(got, 30, "the last element is in range");

    for bad in [3i32, 4, -1, i32::MIN, i32::MAX] {
        let mut canary = 0x5A5A_5A5Ai32;
        let st = pick(xs.as_ptr(), 3, bad, &mut canary);
        assert_eq!(
            st, ML_ST_INDEX_OUT_OF_RANGE,
            "index {bad} must be refused with the reserved status"
        );
        assert_eq!(
            canary, 0x5A5A_5A5A,
            "index {bad}: the out-param must be untouched on a failure"
        );
    }

    // A negative LENGTH is a host contract violation (DP-A6). The module does not launder it:
    // every index is out of range, so every read fails — nothing is dereferenced.
    let mut canary = 0x5A5A_5A5Ai32;
    assert_eq!(
        pick(xs.as_ptr(), -1, 0, &mut canary),
        ML_ST_INDEX_OUT_OF_RANGE
    );
    assert_eq!(canary, 0x5A5A_5A5A);
    drop(m);
    let _ = std::fs::remove_dir_all(out);
}

/// Acceptance H: empty is an ordinary answer. This is the case a host's own idiom breaks on
/// (`@arr[0]` in Delphi), not the module's arithmetic — so the module side is pinned here and
/// the host side in the Pascal host.
#[test]
fn an_empty_array_is_a_normal_answer() {
    let (out, m) = build("empty");
    let largest: Largest = unsafe { std::mem::transmute(m.symbol(b"mlx_largest\0").unwrap()) };

    let xs = [1.5f64, 9.25, -3.0];
    let mut got = 0.0f64;
    assert_eq!(largest(xs.as_ptr(), 3, 0.0, &mut got), 0);
    assert_eq!(got, 9.25);

    // Length 0, and the pointer is never read. Passing a valid pointer anyway, because
    // HOST_ABI's rule is "do not pass NULL" even where nothing would dereference it.
    let mut got = -1.0f64;
    assert_eq!(largest(xs.as_ptr(), 0, 42.0, &mut got), 0);
    assert_eq!(got, 42.0, "an empty array returns the seed, not an error");
    drop(m);
    let _ = std::fs::remove_dir_all(out);
}

/// `len` reads the companion the caller passed — not a NUL scan, not a stored count.
#[test]
fn len_is_the_number_the_host_passed() {
    let (out, m) = build("len");
    let how_many: HowMany = unsafe { std::mem::transmute(m.symbol(b"mlx_how_many\0").unwrap()) };
    let flags = [true, false, true, true];
    for n in 0..=4i32 {
        assert_eq!(how_many(flags.as_ptr(), n), n, "len({n})");
    }
    drop(m);
    let _ = std::fs::remove_dir_all(out);
}

/// A `[bool]` is one byte per element on both sides.
///
/// The out-param version of this hazard was measured on 2026-09-07: declaring `LongBool` made
/// the module's `false` read as `true` in Pascal, with no crash. An array multiplies it —
/// every element after the first would be read from the wrong address — so the element size
/// is pinned by reading elements the host wrote as single bytes.
#[test]
fn a_bool_array_is_one_byte_per_element() {
    let (out, m) = build("bools");
    let any: AnySet = unsafe { std::mem::transmute(m.symbol(b"mlx_any_set\0").unwrap()) };

    assert_eq!(
        std::mem::size_of::<bool>(),
        1,
        "the host's side of the deal"
    );

    let none = [false, false, false, false];
    let mut got = true;
    assert_eq!(any(none.as_ptr(), 4, &mut got), 0);
    assert!(!got, "no flag is set");

    // Only the LAST element is set, so a module reading four bytes per element would walk off
    // the end and answer from whatever follows — the failure this pins.
    let last = [false, false, false, true];
    let mut got = false;
    assert_eq!(any(last.as_ptr(), 4, &mut got), 0);
    assert!(got, "the fourth element is set, and it is one byte along");
    drop(m);
    let _ = std::fs::remove_dir_all(out);
}

/// **The borrowed pointer must not have pulled anything in.** Measured, not assumed.
///
/// Indexing lowers to a bounds check and a raw read. Either could in principle have become a
/// CRT call, and this slice had no import measurement at all — the string slices have one
/// (`string_input.rs`), the array one did not. Found by auditing rather than by a guard
/// (STATUS §7-3 (5)).
///
/// **A comparison, never an absolute** (§7): every cdylib already imports `memcpy`/`memset`
/// through the DllMain scaffolding, so "imports no memset" would be false while "adds no
/// import" is the true and useful claim. The baseline is a scalar module built the same way.
#[test]
fn an_array_parameter_adds_no_import_over_a_scalar_baseline() {
    use ml_oracle::pe;

    let out = std::env::temp_dir().join(format!("mlc_arrin_imports_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&out);
    std::fs::create_dir_all(&out).unwrap();

    let base = mlc::emit::emit_artifacts(
        "export fn f(x: f64) -> f64 { return x * 2.0 }",
        "baseline",
        &out,
    )
    .expect("baseline");
    let baseline = pe::read_imports(&base.dll).expect("baseline imports");

    let arts =
        mlc::emit::emit_artifacts(include_str!("../../../examples/basket.mls"), "basket", &out)
            .expect("emit basket");
    let imports = pe::read_imports(&arts.dll).expect("imports");
    println!("basket imports = {imports:?}");

    assert_eq!(
        imports, baseline,
        "an array parameter must add no import: the module borrows the host's memory for the \
         call and neither copies nor owns it (D16 rule 1)"
    );
    for banned in ["malloc", "free", "memmove", "HeapAlloc"] {
        assert!(
            !imports.iter().any(|i| i.ends_with(&format!("!{banned}"))),
            "{banned} must not be imported: {imports:?}"
        );
    }
    let _ = std::fs::remove_dir_all(&out);
}

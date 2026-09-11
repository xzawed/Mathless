//! Array-return slice — acceptance B and C (E2).
//!
//! The frontend is pinned in `compiler/tests/array_return.rs` and the emitted signature with
//! it. This file asks the only question that matters afterwards: does a real host, holding
//! real memory, get the right answers across the C ABI — and does Q12's hardest promise
//! survive?
//!
//! **That promise is the reason this slice was hard.** Q12 says a truncated call writes
//! nothing. String return kept it with a two-pass count-then-copy, which array elements
//! cannot have: they must be computed. The `result <n>` statement is what replaces the second
//! pass — it declares the length, and the capacity check happens there, before any element is
//! written. `truncation_writes_not_one_byte` is the test that says whether that worked, and it
//! compares the whole buffer byte by byte rather than trusting a status code.
#![cfg(windows)]

use ml_oracle::{pe, Module};
use mlc::abi::ML_ST_INSUFFICIENT_BUFFER;
use mlc::emit::emit_artifacts;

/// A byte that is neither NUL nor plausible data, so "written" and "not written" are both
/// visible. The string-return slice built this habit and it is reused verbatim here.
const CANARY: u8 = 0xAA;

fn build(name: &str, tag: &str) -> (std::path::PathBuf, Module) {
    let src = match name {
        "schedule" => include_str!("../../../examples/schedule.mls"),
        "allocate" => include_str!("../../../examples/allocate.mls"),
        other => panic!("no such example: {other}"),
    };
    let out = std::env::temp_dir().join(format!("mlc_arr_ret_{tag}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&out);
    std::fs::create_dir_all(&out).unwrap();
    let arts = emit_artifacts(src, name, &out).unwrap_or_else(|e| panic!("emit {name}: {e}"));
    let m = Module::load(arts.dll.to_str().unwrap()).unwrap_or_else(|e| panic!("load: {e}"));
    (out, m)
}

/// `int32_t mlx_schedule(int32_t, int32_t, int32_t*, int32_t, int32_t*)`
type Schedule = unsafe extern "C" fn(i32, i32, *mut i32, i32, *mut i32) -> i32;
/// `int32_t mlx_allocate(const int32_t*, int32_t, int32_t, int32_t*, int32_t, int32_t*)`
type Allocate = unsafe extern "C" fn(*const i32, i32, i32, *mut i32, i32, *mut i32) -> i32;
/// `int32_t mlx_in_stock(const int32_t*, int32_t, bool*, int32_t, int32_t*)`
type InStock = unsafe extern "C" fn(*const i32, i32, *mut bool, i32, *mut i32) -> i32;
/// `int32_t mlx_one_payment(int32_t, int32_t, int32_t, int32_t*)` — the D17 shape, unchanged.
type OnePayment = unsafe extern "C" fn(i32, i32, i32, *mut i32) -> i32;

/// B1 — the values, and `*ml_needed` in ELEMENTS.
#[test]
fn a_host_gets_the_whole_schedule_from_one_call() {
    let (_d, m) = build("schedule", "b1");
    let schedule: Schedule = unsafe { std::mem::transmute(m.symbol(b"mlx_schedule\0").unwrap()) };

    let mut buf = [0i32; 16];
    let mut needed = -999i32;
    let st = unsafe { schedule(100_000, 7, buf.as_mut_ptr(), 16, &mut needed) };

    assert_eq!(st, 0, "a fitting call succeeds");
    assert_eq!(
        needed, 7,
        "*ml_needed counts ELEMENTS, not bytes (SPEC §2.2)"
    );
    // 100000 / 7 is 14285 with 5 left over; the example folds nothing, so the parts are equal
    // and the host can see the remainder for itself. Asserting the sum rather than each
    // element is deliberate: it would catch an off-by-one in the loop that equal elements hide.
    assert_eq!(buf[..7], [14_285; 7], "every element written");
    assert_eq!(buf[..7].iter().sum::<i32>(), 99_995);
}

/// B5 — elements the author's loop never assigns come back as zero, not as whatever the host
/// left in its buffer (SPEC §2.5).
///
/// The buffer is filled with canary first, so "the module zeroed it" and "the host's bytes
/// survived" are distinguishable. Without the fill this test would pass on a zeroed stack.
#[test]
fn unwritten_elements_are_zero_and_not_the_hosts_bytes() {
    let (_d, m) = build("allocate", "b5");
    let allocate: Allocate = unsafe { std::mem::transmute(m.symbol(b"mlx_allocate\0").unwrap()) };

    let stock = [10i32, 20, 5];
    let mut buf = [0i32; 8];
    for b in buf.iter_mut() {
        *b = i32::from_ne_bytes([CANARY; 4]);
    }
    let mut needed = -999i32;
    // `want` of 10 is satisfied by the first warehouse, so the loop writes 10 then 0 then 0 —
    // the zeros here come from the fill, and the test cannot tell them apart from written
    // zeros. That is fine: what it CAN tell is that no canary survives.
    let st = unsafe { allocate(stock.as_ptr(), 3, 10, buf.as_mut_ptr(), 8, &mut needed) };

    assert_eq!(st, 0);
    assert_eq!(needed, 3);
    assert_eq!(buf[..3], [10, 0, 0]);
    assert!(
        buf[..3]
            .iter()
            .all(|&v| v != i32::from_ne_bytes([CANARY; 4])),
        "an element the module did not write must be ZERO, not the host's leftover bytes"
    );
}

/// **B2 — the promise this slice exists to keep.** Truncation is a failure, and not one byte
/// of the host's buffer is touched.
///
/// Compared byte by byte rather than element by element: a partial write of three elements
/// out of seven would leave the tail intact and an element-wise check on the tail would pass.
#[test]
fn truncation_writes_not_one_byte() {
    let (_d, m) = build("schedule", "b2");
    let schedule: Schedule = unsafe { std::mem::transmute(m.symbol(b"mlx_schedule\0").unwrap()) };

    let mut buf = [0u8; 3 * 4];
    buf.fill(CANARY);
    let mut needed = -999i32;
    let st = unsafe {
        schedule(
            100_000,
            7,
            buf.as_mut_ptr().cast::<i32>(),
            3, // room for three elements; the answer needs seven
            &mut needed,
        )
    };

    assert_eq!(
        st, ML_ST_INSUFFICIENT_BUFFER,
        "truncation is a FAILURE, not a short success"
    );
    assert_eq!(needed, 7, "and the host is told the exact size it needs");
    assert!(
        buf.iter().all(|&b| b == CANARY),
        "the module wrote into a buffer it had already decided was too small: {buf:?}"
    );
}

/// B3 — the probe idiom, and that it converges in exactly two calls.
///
/// `ml_buf` is NULL here, which SPEC §2.7 permits only when `ml_cap` is 0. The allocation
/// deliberately uses `n * size_of::<i32>()`, because the unit trap this slice adds is a host
/// that allocates `*ml_needed` BYTES and then promises that many ELEMENTS.
#[test]
fn the_probe_converges_in_two_calls() {
    let (_d, m) = build("schedule", "b3");
    let schedule: Schedule = unsafe { std::mem::transmute(m.symbol(b"mlx_schedule\0").unwrap()) };

    let mut needed = -999i32;
    let st = unsafe { schedule(100_000, 7, std::ptr::null_mut(), 0, &mut needed) };
    assert_eq!(st, ML_ST_INSUFFICIENT_BUFFER, "a probe reports truncation");
    assert_eq!(needed, 7);

    let mut heap = vec![0i32; needed as usize];
    let mut again = -999i32;
    let st = unsafe { schedule(100_000, 7, heap.as_mut_ptr(), needed, &mut again) };
    assert_eq!(st, 0, "the second call fits, always");
    assert_eq!(again, needed);
}

/// B4 and B8 — an exact fit succeeds, and a negative capacity is read as zero rather than as
/// a huge unsigned number (SPEC §2.7, the `_snprintf(count < 0)` hazard).
#[test]
fn an_exact_fit_succeeds_and_a_negative_capacity_is_zero() {
    let (_d, m) = build("schedule", "b4");
    let schedule: Schedule = unsafe { std::mem::transmute(m.symbol(b"mlx_schedule\0").unwrap()) };

    let mut buf = [0i32; 7];
    let mut needed = -999i32;
    let st = unsafe { schedule(100_000, 7, buf.as_mut_ptr(), 7, &mut needed) };
    assert_eq!(st, 0, "cap == needed is a SUCCESS");
    assert_eq!(needed, 7);

    let mut canary = [0u8; 7 * 4];
    canary.fill(CANARY);
    let mut needed = -999i32;
    let st = unsafe {
        schedule(
            100_000,
            7,
            canary.as_mut_ptr().cast::<i32>(),
            -5,
            &mut needed,
        )
    };
    assert_eq!(
        st, ML_ST_INSUFFICIENT_BUFFER,
        "a negative cap holds nothing"
    );
    assert_eq!(needed, 7);
    assert!(
        canary.iter().all(|&b| b == CANARY),
        "a negative capacity must not be read as an enormous one"
    );
}

/// **C — the slice's actual argument, measured.**
///
/// Warehouse allocation is sequentially dependent: what warehouse `w` gives depends on what
/// the earlier ones took. Before this slice the host called once per warehouse and the module
/// re-walked the prefix each time, so the rule was cut into n pieces the host reassembled.
/// One call now answers all of it.
#[test]
fn the_sequential_rule_answers_in_one_call() {
    let (_d, m) = build("allocate", "c");
    let allocate: Allocate = unsafe { std::mem::transmute(m.symbol(b"mlx_allocate\0").unwrap()) };

    let stock = [10i32, 20, 5];
    let mut got = [0i32; 3];
    let mut needed = -999i32;
    let st = unsafe { allocate(stock.as_ptr(), 3, 25, got.as_mut_ptr(), 3, &mut needed) };

    assert_eq!(st, 0);
    assert_eq!(needed, 3);
    assert_eq!(got, [10, 15, 0], "fill in order until the order is met");
    assert_eq!(
        got.iter().sum::<i32>(),
        25,
        "and the parts sum to the order"
    );
}

/// An empty result is an ordinary success, not a truncation — even through a NULL buffer.
///
/// It is the one case where `ml_cap == 0` and the call still succeeds, so it is also the case
/// that proves the zero fill never dereferences a NULL it was told holds nothing.
#[test]
fn an_empty_result_with_a_null_buffer_is_a_success() {
    let (_d, m) = build("allocate", "empty");
    let allocate: Allocate = unsafe { std::mem::transmute(m.symbol(b"mlx_allocate\0").unwrap()) };

    let mut needed = -999i32;
    let st = unsafe {
        allocate(
            std::ptr::null(),
            0,
            25,
            std::ptr::null_mut(),
            0,
            &mut needed,
        )
    };
    assert_eq!(st, 0, "nothing to write is not a failure to write");
    assert_eq!(needed, 0);
}

/// §2.6 — a `[bool]` result is ONE BYTE per element, which is the width the generated Delphi
/// unit's `PBoolean` promises.
///
/// Read back through a `[u8]` rather than a `[bool]`, so a module writing four-byte booleans
/// would show up as a stride error here instead of as undefined behaviour in a host.
#[test]
fn a_bool_result_is_one_byte_per_element() {
    let (_d, m) = build("allocate", "bool");
    let in_stock: InStock = unsafe { std::mem::transmute(m.symbol(b"mlx_in_stock\0").unwrap()) };

    let stock = [3i32, 0, 7, 0];
    let mut raw = [CANARY; 8];
    let mut needed = -999i32;
    let st = unsafe {
        in_stock(
            stock.as_ptr(),
            4,
            raw.as_mut_ptr().cast::<bool>(),
            8,
            &mut needed,
        )
    };

    assert_eq!(st, 0);
    assert_eq!(needed, 4);
    assert_eq!(
        raw[..4],
        [1, 0, 1, 0],
        "one byte per element, in order — a four-byte bool would spread these out"
    );
    assert_eq!(
        raw[4], CANARY,
        "and it must not write past the elements it declared"
    );
}

/// A NON-array export in the same module still has the plain D17 shape.
///
/// This is the test that says the buffer triple is added per FUNCTION, not per module. A
/// backend that decided "this module returns an array" rather than "this function does" would
/// pass every other test in this file and break every scalar export beside it -- and the
/// generated header would agree with the mistake, because both come from the same place.
///
/// `one_payment` is also the workaround the slice replaced: one call per month, with the
/// remainder folded into the first. Keeping it callable is deliberate; a host that wants a
/// single month should not have to take the whole schedule.
#[test]
fn a_scalar_export_beside_an_array_one_keeps_the_plain_d17_shape() {
    let (_d, m) = build("schedule", "scalar");
    let one: OnePayment = unsafe { std::mem::transmute(m.symbol(b"mlx_one_payment\0").unwrap()) };

    let mut got = -999i32;
    let st = unsafe { one(100_000, 7, 0, &mut got) };
    assert_eq!(st, 0);
    assert_eq!(got, 14_290, "the first payment carries the remainder");

    let mut rest = -999i32;
    assert_eq!(unsafe { one(100_000, 7, 3, &mut rest) }, 0);
    assert_eq!(rest, 14_285);

    // And the whole schedule still sums to the principal, which the array return alone cannot
    // show: its elements are equal, so an off-by-one in the fold would be invisible there.
    let mut total = 0i32;
    for month in 0..7 {
        let mut v = 0i32;
        assert_eq!(unsafe { one(100_000, 7, month, &mut v) }, 0);
        total += v;
    }
    assert_eq!(total, 100_000, "the parts sum to the principal exactly");

    // The domain error is untouched by this slice, and a failed call writes no out-param.
    let mut canary = 0x5A5A_5A5Ai32;
    assert!(
        unsafe { one(100_000, 7, 7, &mut canary) } > 0,
        "month 7 of 7"
    );
    assert_eq!(canary, 0x5A5A_5A5A);
}

/// **D16 and Q12's core promise: the module does not allocate.** Measured, not assumed.
///
/// This slice writes into the host's buffer — a zero fill over `n` elements and then the
/// author's own writes. Either could have lowered to a CRT call, and a `malloc` or a heap
/// entry appearing here would mean the module had started owning memory it hands back, which
/// is the exact thing D16 forbids and Q12 was designed around.
///
/// It was NOT measured when the slice landed. The string slice has the equivalent test
/// (`string_input.rs`), and the array one simply did not — found by auditing the session's
/// own output rather than by a guard (STATUS §7-3 (5): a guard's scope is a claim too).
///
/// **Stated as a COMPARISON, never as an absolute.** §7 records why: every cdylib carries the
/// same DllMain scaffolding, so the baseline already imports `memcpy` and `memset` whether or
/// not anything uses them. "This module imports no memset" would be false and "this feature
/// adds no import" is what is true — so the baseline is a scalar module built the same way,
/// and the two sets must be identical.
#[test]
fn an_array_return_adds_no_import_over_a_scalar_baseline() {
    let out = std::env::temp_dir().join(format!("mlc_arr_imports_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&out);
    std::fs::create_dir_all(&out).unwrap();

    let base = emit_artifacts(
        "export fn f(x: f64) -> f64 { return x * 2.0 }",
        "baseline",
        &out,
    )
    .expect("baseline");
    let baseline = pe::read_imports(&base.dll).expect("baseline imports");

    // Both examples, because they exercise different element widths: `schedule` writes i32,
    // `allocate` writes i32 AND the one-byte bool path.
    for (src, name) in [
        (include_str!("../../../examples/schedule.mls"), "schedule"),
        (include_str!("../../../examples/allocate.mls"), "allocate"),
    ] {
        let arts = emit_artifacts(src, name, &out).unwrap_or_else(|e| panic!("emit {name}: {e}"));
        let imports = pe::read_imports(&arts.dll).expect("imports");
        println!("{name} imports = {imports:?}");
        assert_eq!(
            imports, baseline,
            "{name} adds an import a scalar module does not have. The module must not \
             allocate (D16), and the buffer protocol exists precisely so it never has to"
        );
        // Belt and braces, the way string_input.rs does it: name what is forbidden, so a
        // change that moved the BASELINE too would still be caught.
        for banned in ["malloc", "free", "calloc", "realloc", "HeapAlloc"] {
            assert!(
                !imports.iter().any(|i| i.ends_with(&format!("!{banned}"))),
                "{name} imports {banned}: {imports:?}"
            );
        }
    }
    let _ = std::fs::remove_dir_all(&out);
}

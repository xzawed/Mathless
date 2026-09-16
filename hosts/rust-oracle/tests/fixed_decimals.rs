//! `fixed(x, places)` — the VALUES, measured through a loaded module (`SPEC-fixed-decimals`).
//!
//! This file is where the slice's reason lives. `STATUS.md` §9-53 ran the documented in-module
//! workaround and measured it answering `"1234.5"` for 1234.05, `"0.-7"` for -0.07 and
//! `"21474836.47"` for fifty million — **every one of them status 0, with no warning**. A
//! frontend test cannot see any of that; only a call can.
//!
//! Buffers are canaries (0xAA) for the same reason `string_return.rs` uses them: "wrote fewer
//! bytes than it claimed" and "wrote past the end" are both invisible to an equality check on
//! the prefix, and D17 says a FAILED call leaves the buffer alone.
#![cfg(windows)]

use ml_oracle::{pe, Module};
use mlc::emit::emit_artifacts;

mod common;

/// Mirrors `ML_ST_INDEX_OUT_OF_RANGE` in the generated header. `fixed` reuses it rather than
/// reserving a new status (DP-M4) — the cost of a new one is what #238 measured.
const ML_ST_INDEX_OUT_OF_RANGE: i32 = -2;
const ML_ST_INSUFFICIENT_BUFFER: i32 = -1;

type FixedFn = extern "C" fn(f64, *mut u8, i32, *mut i32) -> i32;

/// A buffer the module must not scribble on outside `cap` bytes.
struct Canary {
    bytes: [u8; 64],
}

impl Canary {
    fn new() -> Canary {
        Canary { bytes: [0xAA; 64] }
    }
    fn ptr(&mut self) -> *mut u8 {
        self.bytes.as_mut_ptr()
    }
    fn intact_from(&self, from: usize) -> bool {
        self.bytes[from..].iter().all(|&b| b == 0xAA)
    }
}

fn build(tag: &str, src: &str) -> (common::TempOut, Module) {
    let out = common::TempOut::new(&format!("fixed_{tag}"));
    let arts = emit_artifacts(src, "money", &out).expect("emit");
    let m = Module::load(arts.dll.to_str().unwrap()).expect("load money.dll");
    (out, m)
}

fn amount_fn(m: &Module) -> FixedFn {
    unsafe { std::mem::transmute(m.symbol(b"mlx_amount\0").unwrap()) }
}

/// Call with a 64-byte canary and return `(status, needed, text, buffer intact past needed)`.
fn call(f: FixedFn, x: f64) -> (i32, i32, String, bool) {
    let mut buf = Canary::new();
    let mut needed = -7i32;
    let status = f(x, buf.ptr(), 64, &mut needed);
    if status != 0 {
        // Nothing may have been written, so there is no text to read.
        return (status, needed, String::new(), buf.intact_from(0));
    }
    let n = (needed - 1).max(0) as usize;
    let text = String::from_utf8_lossy(&buf.bytes[..n]).into_owned();
    (
        status,
        needed,
        text,
        buf.bytes[n] == 0 && buf.intact_from(n + 1),
    )
}

/// **Acceptance A** — the table `SPEC-fixed-decimals` §0.1 measured wrong, now measured right.
///
/// The "was" column is not decoration: it is what a loaded module actually returned on
/// 2026-09-14 for the workaround this slice replaces, and it is kept here so that a future
/// change which reintroduces any of those answers fails against the recorded defect rather
/// than against an abstract expectation.
#[test]
fn the_table_that_motivated_the_slice_is_right_now() {
    let (_out, m) = build(
        "a",
        "export fn amount(won: f64) -> string! { return fixed(won, 2) }",
    );
    let f = amount_fn(&m);

    for (x, want, was) in [
        (1234.50_f64, "1234.50", "1234.50"),
        (1234.05, "1234.05", "1234.5"),
        (-1234.05, "-1234.05", "-1234.-5"),
        (-0.07, "-0.07", "0.-7"),
        (50_000_000.0, "50000000.00", "21474836.47"),
    ] {
        let (status, needed, text, intact) = call(f, x);
        assert_eq!(status, 0, "{x} must succeed");
        assert_eq!(text, want, "{x}: the workaround used to answer {was:?}");
        assert_eq!(
            needed as usize,
            want.len() + 1,
            "{x}: needed counts the NUL (DP-T4)"
        );
        assert!(intact, "{x}: wrote past the result");
    }

    drop(m);
}

/// **Acceptance D** — no `-0`, and the sign is decided by the ROUNDED value.
///
/// `-0.004` at two places rounds to zero. Printing the input's sign there would give `"-0.00"`,
/// which is the same class of defect as §0.1's `"0.-7"`: a sign that does not belong to the
/// number being shown.
#[test]
fn a_value_that_rounds_to_zero_has_no_sign() {
    let (_out, m) = build(
        "d",
        "export fn amount(won: f64) -> string! { return fixed(won, 2) }",
    );
    let f = amount_fn(&m);

    for x in [-0.004_f64, -0.0, 0.0, 0.004] {
        let (status, _, text, _) = call(f, x);
        assert_eq!(status, 0);
        assert_eq!(text, "0.00", "{x} must not carry a sign");
    }

    drop(m);
}

/// **Acceptance C** — the same direction as `round`, measured side by side in ONE module.
///
/// The three values are the ones `SPEC-fixed-decimals` §3-C names, and the expected answers
/// are written out **as measured**, including the two that contradict decimal intuition:
///
///   - `1.005` → `"1.00"`. The nearest f64 to 1.005 is BELOW it, so ×100 is 100.49999999999999
///     and half-away-from-zero keeps 100. `round(1.005 * 100.0)` answers 100 too.
///   - `2.675` → `"2.68"`. The nearest f64 to 2.675 is below it, but ×100 rounds back up to
///     exactly 267.5, and half-away-from-zero takes 268. `round` agrees.
///
/// Hiding either of those behind a "close enough" comparison would hide the property the test
/// exists for: whatever `fixed` does, `round` does the same, because there is one rule.
#[test]
fn the_rounding_direction_is_the_one_round_takes() {
    let (_out, m) = build(
        "c",
        "export fn amount(won: f64) -> string! { return fixed(won, 2) }\n\
         export fn scaled(won: f64) -> i32 { return round(won * 100.0) as i32 }",
    );
    let f = amount_fn(&m);
    let scaled: extern "C" fn(f64) -> i32 =
        unsafe { std::mem::transmute(m.symbol(b"mlx_scaled\0").unwrap()) };

    for (x, want) in [
        (1.005_f64, "1.00"),
        (2.675, "2.68"),
        (-1.005, "-1.00"),
        (1.5, "1.50"),
        (-1.5, "-1.50"),
        (2.5, "2.50"),
    ] {
        let (status, _, text, _) = call(f, x);
        assert_eq!(status, 0);
        assert_eq!(text, want, "{x}");
        // The same digits the other way round: `fixed` without its point must be what
        // `round(x * 100)` produced. One rule, two spellings.
        let from_round = scaled(x).to_string();
        let from_fixed = text.replace('.', "");
        let from_fixed = from_fixed.trim_start_matches('-');
        let from_round_mag = from_round.trim_start_matches('-');
        assert_eq!(
            from_fixed.trim_start_matches('0'),
            from_round_mag.trim_start_matches('0'),
            "{x}: fixed said {text:?}, round said {from_round:?}"
        );
    }

    drop(m);
}

/// **Acceptance B** — the two ends of the place range, through a call.
#[test]
fn zero_places_has_no_point_and_nine_places_has_nine_digits() {
    let (_out, m) = build(
        "b",
        "export fn amount(won: f64) -> string! { return fixed(won, 0) }\n\
         export fn nine(x: f64) -> string! { return fixed(x, 9) }",
    );
    let f = amount_fn(&m);
    let nine: FixedFn = unsafe { std::mem::transmute(m.symbol(b"mlx_nine\0").unwrap()) };

    for (x, want) in [(-1234.6_f64, "-1235"), (0.4, "0"), (-0.4, "0")] {
        let (status, _, text, _) = call(f, x);
        assert_eq!(status, 0);
        assert_eq!(text, want, "{x}: places 0 means no point at all");
    }

    let (status, _, text, _) = call(nine, 1.5);
    assert_eq!(status, 0);
    assert_eq!(text, "1.500000000");
    assert_eq!(
        text.split('.').nth(1).map(str::len),
        Some(9),
        "exactly nine, zero-filled"
    );

    drop(m);
}

/// **Acceptance E** — the i64 edge, measured on BOTH sides.
///
/// `SPEC-fixed-decimals` §2.3 says the lowering works in i64 precisely so the `as i32`
/// saturation that produced `"21474836.47"` cannot happen. i64 is not unbounded either, and
/// this pins where it ends: one representable step apart, one succeeds and one is `-2`.
#[test]
fn past_the_i64_range_is_a_status_not_a_wrong_number() {
    let (_out, m) = build(
        "e",
        "export fn amount(won: f64) -> string! { return fixed(won, 2) }",
    );
    let f = amount_fn(&m);

    // The largest f64 whose ×100 still fits in i64, and its immediate neighbour above.
    let ok = 92_233_720_368_547_750.0_f64;
    let over = 92_233_720_368_547_760.0_f64;
    assert!(ok < over, "the two probes must be different f64 values");

    let (status, _, text, _) = call(f, ok);
    assert_eq!(status, 0, "just inside the range must answer");
    assert_eq!(text, "92233720368547747.84");

    let (status, _, _, intact) = call(f, over);
    assert_eq!(
        status, ML_ST_INDEX_OUT_OF_RANGE,
        "just outside must be a status, never a wrapped number"
    );
    assert!(intact, "a failed call leaves the buffer alone (D17)");

    // The NEGATIVE side of the same check, which had never been called. It is not symmetry
    // for its own sake: `ml_fixscale`'s floor is written `<=` rather than `<` ON PURPOSE, so
    // that `r` can never be exactly `i64::MIN`, because BOTH helpers below it do `-v` and
    // that negation is what would overflow. The comment there says so; nothing measured it.
    //
    // A Grok verify round asked for this one — it noticed the passage above tests only the
    // positive overflow while the guard's stated purpose is the negation on the other side.
    let (status, needed, text, _) = call(f, -ok);
    assert_eq!(status, 0, "just inside the range must answer on both signs");
    assert_eq!(text, "-92233720368547747.84");
    assert_eq!(
        needed as usize,
        text.len() + 1,
        "the sign is counted once, by pass 1 as well as pass 2"
    );

    let (status, _, _, intact) = call(f, -over);
    assert_eq!(
        status, ML_ST_INDEX_OUT_OF_RANGE,
        "just outside must be a status on the negative side too, never a wrapped number"
    );
    assert!(intact, "a failed call leaves the buffer alone (D17)");

    drop(m);
}

/// **Acceptance F** — NaN and both infinities, with the buffer untouched.
#[test]
fn a_value_with_no_decimal_form_is_refused_and_writes_nothing() {
    let (_out, m) = build(
        "f",
        "export fn amount(won: f64) -> string! { return fixed(won, 2) }",
    );
    let f = amount_fn(&m);

    for x in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        let (status, _, _, intact) = call(f, x);
        assert_eq!(status, ML_ST_INDEX_OUT_OF_RANGE, "{x} has no decimal form");
        assert!(intact, "{x}: not one byte may be written on failure");
    }

    drop(m);
}

/// **Acceptance B, the run-time half** — a non-literal `places` is checked in the module.
///
/// The typechecker can only refuse a LITERAL out of range. A parameter arrives at run time,
/// and the question this answers is what happens then: a status, not a buffer overrun and not
/// a silently clamped result.
#[test]
fn a_place_count_that_is_not_a_literal_is_checked_at_run_time() {
    let (_out, m) = build(
        "rt",
        "export fn amount(won: f64, places: i32) -> string! { return fixed(won, places) }",
    );
    let g: extern "C" fn(f64, i32, *mut u8, i32, *mut i32) -> i32 =
        unsafe { std::mem::transmute(m.symbol(b"mlx_amount\0").unwrap()) };

    for (places, want) in [(0, Some("1235")), (2, Some("1234.50")), (9, None)] {
        let mut buf = Canary::new();
        let mut needed = -7i32;
        let status = g(1234.5, places, buf.ptr(), 64, &mut needed);
        assert_eq!(status, 0, "places={places} is inside the range");
        if let Some(want) = want {
            let n = (needed - 1) as usize;
            assert_eq!(&String::from_utf8_lossy(&buf.bytes[..n]), want);
        }
    }

    for places in [-1, 10, 1000, i32::MIN, i32::MAX] {
        let mut buf = Canary::new();
        let mut needed = -7i32;
        let status = g(1234.5, places, buf.ptr(), 64, &mut needed);
        assert_eq!(
            status, ML_ST_INDEX_OUT_OF_RANGE,
            "places={places} is outside 0..=9"
        );
        assert!(
            buf.bytes.iter().all(|&b| b == 0xAA),
            "places={places}: nothing may be written"
        );
    }

    drop(m);
}

/// **Acceptance G** — Q12 unchanged: the probe converges in two calls.
#[test]
fn the_probe_converges_in_exactly_two_calls() {
    let (_out, m) = build(
        "g",
        "export fn amount(won: f64) -> string! { return fixed(won, 2) }",
    );
    let f = amount_fn(&m);

    // `ml_buf` may be NULL iff `ml_cap == 0` (DP-T7).
    let mut needed = -7i32;
    let status = f(50_000_000.0, core::ptr::null_mut(), 0, &mut needed);
    assert_eq!(status, ML_ST_INSUFFICIENT_BUFFER);
    assert_eq!(needed, 12, "\"50000000.00\" is 11 bytes, 12 with the NUL");

    let mut buf = vec![0xAAu8; needed as usize];
    let mut needed2 = -7i32;
    let status = f(50_000_000.0, buf.as_mut_ptr(), needed, &mut needed2);
    assert_eq!(status, 0, "the size the probe reported must be enough");
    assert_eq!(needed2, needed);
    assert_eq!(&buf[..11], b"50000000.00");
    assert_eq!(buf[11], 0);

    // One byte short is a failure that writes nothing — not a truncation.
    let mut buf = Canary::new();
    let mut n3 = -7i32;
    assert_eq!(
        f(50_000_000.0, buf.ptr(), needed - 1, &mut n3),
        ML_ST_INSUFFICIENT_BUFFER
    );
    assert_eq!(n3, needed);
    assert!(buf.intact_from(0), "nothing may be written on truncation");

    drop(m);
}

/// The shipped example, loaded and called — all three of its exports.
///
/// Every other test here compiles its own snippet, which is the right shape for pinning one
/// property at a time but leaves `examples/money.mls` itself only compiled, never run. The C
/// host gate calls it; this is the oracle half of acceptance J, and it is also what keeps
/// `golden.rs::every_exported_example_function_is_called_by_an_oracle_test` true — that guard
/// caught `labelled` having no named call site anywhere.
#[test]
fn the_shipped_example_answers_through_a_loaded_module() {
    let out = common::TempOut::new("fixed_example");
    let arts = emit_artifacts(include_str!("../../../examples/money.mls"), "money", &out)
        .expect("emit money");
    let m = Module::load(arts.dll.to_str().unwrap()).expect("load money.dll");

    let amount: FixedFn = unsafe { std::mem::transmute(m.symbol(b"mlx_amount\0").unwrap()) };
    let labelled: FixedFn = unsafe { std::mem::transmute(m.symbol(b"mlx_labelled\0").unwrap()) };
    let rate: FixedFn = unsafe { std::mem::transmute(m.symbol(b"mlx_rate\0").unwrap()) };

    let (status, needed, text, intact) = call(amount, 1234.05);
    assert_eq!((status, text.as_str(), needed), (0, "1234.05", 8));
    assert!(intact);

    let (status, _, text, intact) = call(labelled, -0.07);
    assert_eq!((status, text.as_str()), (0, "KRW -0.07"));
    assert!(intact);

    // 8.25 at one place, rounded away from zero. `0.0825` is a tax rate, not money — the
    // reason the builtin is not called `money` (DP-M2).
    let (status, _, text, intact) = call(rate, 0.0825);
    assert_eq!((status, text.as_str()), (0, "8.3%"));
    assert!(intact);

    drop(m);
}

/// **Acceptance I** — formatting a number adds no import over a scalar baseline.
///
/// This is the measurement that says "no CRT". A `snprintf` or a `_ftoa` would appear here
/// long before anyone noticed it in the generated source, and the protection proxies
/// `SECURITY.md` records are export count and import set — both must be unmoved.
#[test]
fn formatting_adds_no_import_and_no_export() {
    let out = common::TempOut::new("fixed_imports");

    let base = emit_artifacts(
        "export fn f(x: f64) -> f64 { return x * 2.0 }",
        "baseline",
        &out,
    )
    .expect("baseline");
    let baseline = pe::read_imports(&base.dll).expect("baseline imports");

    let arts = emit_artifacts(
        "export fn amount(won: f64) -> string! { return fixed(won, 2) }",
        "money",
        &out,
    )
    .expect("emit money");
    let imports = pe::read_imports(&arts.dll).expect("imports");
    println!("money imports = {imports:?}");

    assert_eq!(
        imports, baseline,
        "decimal formatting is integer arithmetic the module carries itself"
    );
    for banned in ["snprintf", "sprintf", "printf", "ftoa", "gcvt", "malloc"] {
        assert!(
            !imports
                .iter()
                .any(|i| i.to_lowercase().contains(&banned.to_lowercase())),
            "{banned} must not be imported: {imports:?}"
        );
    }

    let exports = pe::read_exports(&arts.dll).expect("exports");
    assert_eq!(
        exports.len(),
        3,
        "one function plus the two reserved exports (D18): {exports:?}"
    );
}

/// `fixed` composes with concatenation, and the two-pass count still agrees.
///
/// A concatenation sizes the host's buffer in pass 1 and fills it in pass 2. `fixed` is the
/// first piece kind whose width depends on a COMPUTED value rather than on bytes already in
/// memory, so "the two passes agree" is worth a call rather than an inspection: the failure
/// mode is a write past the end of the caller's buffer, which the canary sees.
#[test]
fn a_formatted_number_is_a_piece_like_any_other() {
    let (_out, m) = build(
        "cat",
        "export fn amount(won: f64) -> string! { return \"KRW \" + fixed(won, 2) + \" only\" }",
    );
    let f = amount_fn(&m);

    for (x, want) in [
        (1234.05_f64, "KRW 1234.05 only"),
        (-0.07, "KRW -0.07 only"),
        (0.0, "KRW 0.00 only"),
    ] {
        let (status, needed, text, intact) = call(f, x);
        assert_eq!(status, 0);
        assert_eq!(text, want, "{x}");
        assert_eq!(needed as usize, want.len() + 1);
        assert!(intact, "{x}: the two passes disagreed");
    }

    drop(m);
}

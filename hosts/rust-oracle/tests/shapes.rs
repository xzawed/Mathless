//! WW1b of SPEC-export-wrappers — value coverage for the export shapes that had none.
//!
//! This file exists *before* the wrapper refactor, on purpose. The refactor rewrites how every
//! export reaches the C ABI, and an adversarial pass measured that none of the planned
//! snapshots observe a value: a swapped-argument adapter reproduced the header, the Delphi
//! unit, the export set, the import set and the file size exactly, differing only in `.text`
//! bytes. So the real safety net is this kind of test, and seven shapes had no such test at
//! all. A harness written after the change cannot tell anyone the change was safe.
//!
//! Every assertion below is chosen to fail for a *specific* wrong adapter, and the comment
//! says which. That is the difference between coverage and a checklist.
#![cfg(windows)]

use core::ffi::c_char;

use ml_oracle::Module;
use mlc::emit::emit_artifacts;

const E_NEG: i32 = 1;
const E_ODD: i32 = 2;

fn build(tag: &str) -> (std::path::PathBuf, Module) {
    let src = include_str!("../../../examples/shapes.mls");
    let out = std::env::temp_dir().join(format!("mlc_shapes_{tag}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&out);
    std::fs::create_dir_all(&out).unwrap();
    let arts = emit_artifacts(src, "shapes", &out).expect("emit shapes");
    let m = Module::load(arts.dll.to_str().unwrap()).expect("load shapes.dll");
    (out, m)
}

#[test]
fn a_fallible_i32_return_actually_uses_the_out_param() {
    // THE one. For `-> i32!` the value and the status are the same Rust type, so an adapter
    // written `match body(n) { Ok(v) => v, Err(e) => e }` compiles, never writes `*out_value`,
    // and reports every non-zero success as an error code. That is #67's defect re-entering
    // at the adapter layer, and for `f64!`/`bool!` it would be a hard type error instead —
    // so only THIS shape can catch it.
    let (out, m) = build("i32bang");
    let bump: extern "C" fn(i32, *mut i32) -> i32 =
        unsafe { std::mem::transmute(m.symbol(b"mlx_bump\0").unwrap()) };

    let mut v = -99i32;
    assert_eq!(bump(10, &mut v), 0, "status must be 0, not the value");
    assert_eq!(v, 3, "the value must arrive through out_value");

    // The two values that expose the wrong adapter. Under it the status IS the value, so a
    // success returning 0 would read as OK by accident...
    let mut v = -99i32;
    assert_eq!(bump(7, &mut v), 0);
    assert_eq!(v, 0, "a success value of 0 is still a success");
    // ...and a success returning 1 would read as the failure E_NEG, which is also 1.
    let mut v = -99i32;
    assert_eq!(
        bump(8, &mut v),
        0,
        "a success whose VALUE equals an error code"
    );
    assert_eq!(v, E_NEG, "the value is 1 — the status must still be 0");

    let mut v = -99i32;
    assert_eq!(bump(-1, &mut v), E_NEG);
    assert_eq!(v, -99, "a failure leaves out_value untouched");

    drop(m);
    let _ = std::fs::remove_dir_all(&out);
}

#[test]
fn a_fallible_bool_return_round_trips() {
    // `-> bool!` appeared nowhere in the repo. `bool` is 1 byte at this ABI (D18), so an
    // adapter that widened it would be visible here and nowhere else.
    let (out, m) = build("boolbang");
    let is_big: extern "C" fn(i32, *mut bool) -> i32 =
        unsafe { std::mem::transmute(m.symbol(b"mlx_is_big\0").unwrap()) };

    let mut v = false;
    assert_eq!(is_big(500, &mut v), 0);
    assert!(v);
    let mut v = true;
    assert_eq!(is_big(5, &mut v), 0);
    assert!(!v, "false must actually be written, not left alone");

    let mut v = true;
    assert_eq!(is_big(-1, &mut v), E_NEG);
    assert!(v, "untouched on failure");

    drop(m);
    let _ = std::fs::remove_dir_all(&out);
}

#[test]
fn zero_parameter_exports_work_in_both_shapes() {
    // An adapter template that emits `body()` versus `body` is a build error, and one that
    // mishandles the empty parameter list would show up only here. Note the C declarations
    // differ: the infallible one is `(void)`, the fallible one is `(int32_t* out_value)`.
    let (out, m) = build("noargs");
    let answer: extern "C" fn(*mut i32) -> i32 =
        unsafe { std::mem::transmute(m.symbol(b"mlx_answer\0").unwrap()) };
    let pi_ish: extern "C" fn() -> f64 =
        unsafe { std::mem::transmute(m.symbol(b"mlx_pi_ish\0").unwrap()) };

    let mut v = -1i32;
    assert_eq!(answer(&mut v), 0);
    assert_eq!(v, 42);
    assert_eq!(pi_ish(), 3.5);

    drop(m);
    let _ = std::fs::remove_dir_all(&out);
}

#[test]
fn two_same_typed_out_params_are_not_transposed() {
    // The sharpest hole. Both are `int32_t*`, so rustc cannot catch a swap in the adapter's
    // forwarding call — differently-typed outs are saved by the type system, these are not.
    // The values are deliberately asymmetric (lo < hi) so a swap is visible.
    let (out, m) = build("twoouts");
    let span: extern "C" fn(i32, *mut i32, *mut i32) -> i32 =
        unsafe { std::mem::transmute(m.symbol(b"mlx_span\0").unwrap()) };

    let (mut lo, mut hi) = (0i32, 0i32);
    assert_eq!(span(10, &mut lo, &mut hi), 10);
    assert_eq!(lo, 9, "lo is the FIRST declared out");
    assert_eq!(hi, 11, "hi is the second — a swap makes lo=11, hi=9");

    drop(m);
    let _ = std::fs::remove_dir_all(&out);
}

#[test]
fn out_params_that_are_not_i32_are_written_through() {
    // Every declared out loaded by a test before this was an `i32`. An adapter that forwarded
    // a `*mut f64` as something else would not have been caught anywhere.
    let (out, m) = build("outtypes");
    let split: extern "C" fn(f64, *mut f64, *mut bool) -> f64 =
        unsafe { std::mem::transmute(m.symbol(b"mlx_split\0").unwrap()) };

    let (mut whole, mut neg) = (-1.0f64, true);
    let frac = split(3.25, &mut whole, &mut neg);
    assert_eq!(whole, 3.0);
    assert!(!neg);
    assert_eq!(frac, 0.25);

    let (mut whole, mut neg) = (-1.0f64, false);
    let frac = split(-2.5, &mut whole, &mut neg);
    assert_eq!(whole, -3.0, "floor(-2.5) is -3, not -2");
    assert!(neg);
    assert_eq!(frac, 0.5);

    drop(m);
    let _ = std::fs::remove_dir_all(&out);
}

#[test]
fn a_declared_out_and_out_value_of_the_same_type_are_not_transposed() {
    // Two trailing `int32_t*` slots. If the adapter passed them in the wrong order, the
    // return value would land in the host's declared out and vice versa — and both are the
    // same type, so nothing at compile time objects. `examples/quote.mls::line_check` has
    // this shape too, but its two values were not chosen to make a swap visible; these are.
    let (out, m) = build("samety");
    let grade: extern "C" fn(i32, *mut i32, *mut i32) -> i32 =
        unsafe { std::mem::transmute(m.symbol(b"mlx_grade\0").unwrap()) };

    let (mut tier, mut v) = (-99i32, -99i32);
    assert_eq!(grade(5, &mut tier, &mut v), 0);
    assert_eq!(tier, 1, "the declared out");
    assert_eq!(v, 50, "and the return value — a swap gives tier=50, v=1");

    let (mut tier, mut v) = (-99i32, -99i32);
    assert_eq!(grade(-1, &mut tier, &mut v), E_NEG);
    assert_eq!(v, -99, "out_value untouched on failure");

    drop(m);
    let _ = std::fs::remove_dir_all(&out);
}

#[test]
fn a_fallible_function_can_take_a_string() {
    // Status + `*const u8` existed at no level before. The parameter is borrowed for the call
    // and the adapter must forward the pointer unchanged.
    let (out, m) = build("strarg");
    let code_of: extern "C" fn(*const c_char, *mut i32) -> i32 =
        unsafe { std::mem::transmute(m.symbol(b"mlx_code_of\0").unwrap()) };

    let mut v = -99i32;
    assert_eq!(code_of(c"KR".as_ptr(), &mut v), 0);
    assert_eq!(v, 82);

    let mut v = -99i32;
    assert_eq!(code_of(c"XX".as_ptr(), &mut v), 0);
    assert_eq!(v, 0);

    let mut v = -99i32;
    assert_eq!(code_of(c"ODD".as_ptr(), &mut v), E_ODD);
    assert_eq!(v, -99);

    drop(m);
    let _ = std::fs::remove_dir_all(&out);
}

#[test]
fn a_string_return_with_no_string_parameter_works() {
    // The Q12 triple sitting behind a non-string parameter list — a different arrangement of
    // the three trailing slots from every string test in the repo.
    let (out, m) = build("q12int");
    let tag: extern "C" fn(i32, *mut u8, i32, *mut i32) -> i32 =
        unsafe { std::mem::transmute(m.symbol(b"mlx_tag\0").unwrap()) };

    let mut buf = [0xAAu8; 32];
    let mut needed = -99i32;
    assert_eq!(tag(500, buf.as_mut_ptr(), 32, &mut needed), 0);
    assert_eq!(needed, 4, "\"BIG\" plus the NUL");
    assert_eq!(&buf[..4], b"BIG\0");

    let mut buf = [0xAAu8; 32];
    let mut needed = -99i32;
    assert_eq!(tag(-1, buf.as_mut_ptr(), 32, &mut needed), E_NEG);
    assert!(
        buf.iter().all(|&b| b == 0xAA),
        "a domain error writes nothing"
    );
    assert_eq!(needed, -99);

    drop(m);
    let _ = std::fs::remove_dir_all(&out);
}

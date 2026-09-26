//! Checked i32 arithmetic in fallible bodies — the values a loaded module returns
//! (`SPEC-checked-arithmetic` acceptance A, B, C, D, G and DP-O7, DP-O9).
//!
//! Every operation is measured on both sides of its boundary: the last input that fits returns
//! the exact answer, the first that does not returns `ML_ST_OVERFLOW` and leaves the out value
//! alone. The infallible twin beside it shows the rule is about the BODY, not the operator.
#![cfg(windows)]

use ml_oracle::Module;
use mlc::emit::emit_artifacts;

mod common;

const OVERFLOW: i32 = mlc::abi::ML_ST_OVERFLOW;

const SRC: &str = "
const BIG = 2147483647
export fn add(a: i32, b: i32) -> i32! { return a + b }
export fn sub(a: i32, b: i32) -> i32! { return a - b }
export fn mul(a: i32, b: i32) -> i32! { return a * b }
export fn neg(a: i32) -> i32! { return -a }
export fn div(a: i32, b: i32) -> i32! { return a / b }
export fn rem(a: i32, b: i32) -> i32! { return a % b }
export fn conv(x: f64) -> i32! { return x as i32 }
export fn text(a: i32) -> string! { return (a * 2) as string }
export fn arr(a: i32) -> [i32]! {
  result 2
  result[0] = 1
  result[1] = a * 2
}
export fn add_wrap(a: i32, b: i32) -> i32 { return a + b }
export fn lit() -> i32! { return 2147483647 + 1 }
export fn via_const(a: i32) -> i32! { return BIG + a }
fn twice(a: i32) -> i32 { return a * 2 }
export fn via_helper(a: i32) -> i32! { return twice(a) }
";

type Bin = extern "C" fn(i32, i32, *mut i32) -> i32;
type Un = extern "C" fn(i32, *mut i32) -> i32;
type Conv = extern "C" fn(f64, *mut i32) -> i32;
type Text = extern "C" fn(i32, *mut u8, i32, *mut i32) -> i32;
type Arr = extern "C" fn(i32, *mut i32, i32, *mut i32) -> i32;
type Wrap = extern "C" fn(i32, i32) -> i32;
type Lit = extern "C" fn(*mut i32) -> i32;

fn load(tag: &str) -> (common::TempOut, Module) {
    let out = common::TempOut::new(&format!("checked_{tag}"));
    let arts = emit_artifacts(SRC, "checked", &out).expect("emit checked");
    let m = Module::load(arts.dll.to_str().unwrap()).expect("load checked.dll");
    (out, m)
}

fn sym<T>(m: &Module, name: &[u8]) -> T {
    let p = m.symbol(name).unwrap();
    unsafe { std::mem::transmute_copy(&p) }
}

/// Status and out value; the out value stays at the sentinel when the call fails (D17).
fn bin(f: Bin, a: i32, b: i32) -> (i32, i32) {
    let mut v = -7;
    (f(a, b, &mut v), v)
}

fn un(f: Un, a: i32) -> (i32, i32) {
    let mut v = -7;
    (f(a, &mut v), v)
}

/// **Acceptance A: each operation, both sides of its boundary.**
#[test]
fn each_operation_fails_at_its_boundary_and_not_before() {
    let (_out, m) = load("ops");
    let (add, sub, mul): (Bin, Bin, Bin) = (
        sym(&m, b"mlx_add\0"),
        sym(&m, b"mlx_sub\0"),
        sym(&m, b"mlx_mul\0"),
    );
    let neg: Un = sym(&m, b"mlx_neg\0");

    assert_eq!(bin(add, i32::MAX - 1, 1), (0, i32::MAX));
    assert_eq!(bin(add, i32::MAX, 1), (OVERFLOW, -7));
    assert_eq!(bin(sub, i32::MIN + 1, 1), (0, i32::MIN));
    assert_eq!(bin(sub, i32::MIN, 1), (OVERFLOW, -7));
    // 46340² fits, 46341² does not.
    assert_eq!(bin(mul, 46340, 46340), (0, 2147395600));
    assert_eq!(bin(mul, 46341, 46341), (OVERFLOW, -7));
    assert_eq!(bin(mul, -46341, 46341), (OVERFLOW, -7));
    assert_eq!(un(neg, i32::MIN + 1), (0, i32::MAX));
    assert_eq!(un(neg, i32::MIN), (OVERFLOW, -7));
    drop(m);
}

/// **Acceptance B: division and remainder — only the quotient that does not exist fails.**
#[test]
fn only_the_quotient_min_by_minus_one_fails() {
    let (_out, m) = load("div");
    let div: Bin = sym(&m, b"mlx_div\0");
    let rem: Bin = sym(&m, b"mlx_rem\0");
    assert_eq!(
        bin(div, i32::MIN, -1),
        (OVERFLOW, -7),
        "2^31 has no i32 value"
    );
    assert_eq!(bin(div, i32::MIN, 1), (0, i32::MIN));
    assert_eq!(bin(div, 7, 0), (0, 0), "DP-N4 is untouched: x / 0 is 0");
    // DP-O5: the true remainder of MIN by -1 is 0, and 0 fits.
    assert_eq!(bin(rem, i32::MIN, -1), (0, 0));
    assert_eq!(bin(rem, 7, 0), (0, 0));
    drop(m);
}

/// **Acceptance D: `f64 as i32` fails for NaN and for what truncates outside i32, only there.**
#[test]
fn a_conversion_with_no_i32_value_fails() {
    let (_out, m) = load("conv");
    let conv: Conv = sym(&m, b"mlx_conv\0");
    let c = |x: f64| {
        let mut v = -7;
        (conv(x, &mut v), v)
    };
    assert_eq!(c(3.9), (0, 3));
    assert_eq!(c(-3.9), (0, -3));
    assert_eq!(c(2147483647.9), (0, i32::MAX), "truncates into range");
    assert_eq!(c(-2147483648.9), (0, i32::MIN), "truncates into range");
    for x in [
        2147483648.0,
        -2147483649.0,
        f64::NAN,
        f64::INFINITY,
        f64::NEG_INFINITY,
        3e9,
    ] {
        assert_eq!(c(x), (OVERFLOW, -7), "{x} has no i32 value");
    }
    drop(m);
}

/// **Acceptance A and G: the string and array shapes, and a repeat call gives the same answer.**
#[test]
fn string_and_array_bodies_are_checked_too() {
    let (_out, m) = load("shapes");
    let text: Text = sym(&m, b"mlx_text\0");
    let mut buf = [0u8; 32];
    let mut needed = -7;
    assert_eq!(text(1073741823, buf.as_mut_ptr(), 32, &mut needed), 0);
    assert_eq!(&buf[..needed as usize - 1], b"2147483646");
    assert_eq!(
        text(1073741824, buf.as_mut_ptr(), 32, &mut needed),
        OVERFLOW
    );

    let arr: Arr = sym(&m, b"mlx_arr\0");
    let mut out = [0i32; 2];
    let mut n = -7;
    assert_eq!(arr(3, out.as_mut_ptr(), 2, &mut n), 0);
    assert_eq!(out, [1, 6]);
    // After `result 2`: the buffer is undefined (DP-O6), and asking again changes nothing —
    // -3 is not a truncation, so a retry loop keyed on `status < 0` would never converge.
    for _ in 0..2 {
        let mut n = -7;
        assert_eq!(arr(1073741824, out.as_mut_ptr(), 2, &mut n), OVERFLOW);
    }
    drop(m);
}

/// **Acceptance C, DP-O7, DP-O9: the rule belongs to the body.**
#[test]
fn infallible_bodies_still_wrap_and_literals_are_checked() {
    let (_out, m) = load("scope");
    let add_wrap: Wrap = sym(&m, b"mlx_add_wrap\0");
    assert_eq!(
        add_wrap(i32::MAX, 1),
        i32::MIN,
        "an infallible body keeps wrapping"
    );

    // DP-O9: a literal-only overflow is not folded — it fails when it runs.
    let lit: Lit = sym(&m, b"mlx_lit\0");
    let mut v = -7;
    assert_eq!((lit(&mut v), v), (OVERFLOW, -7));
    let via_const: Un = sym(&m, b"mlx_via_const\0");
    assert_eq!(un(via_const, 0), (0, i32::MAX));
    assert_eq!(un(via_const, 1), (OVERFLOW, -7));

    // DP-O7: an infallible helper is the escape hatch — it wraps even under a fallible caller.
    let via_helper: Un = sym(&m, b"mlx_via_helper\0");
    assert_eq!(un(via_helper, 1073741824), (0, i32::MIN));
    drop(m);
}

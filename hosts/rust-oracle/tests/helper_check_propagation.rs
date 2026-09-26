//! Helper check propagation — the values a loaded module returns
//! (`SPEC-helper-check-propagation` acceptance A, B, C, D and G(1)).
//!
//! The rule is the CALL PATH's: arithmetic a fallible body executes is checked wherever it is
//! written, so a helper without `!` fails with `ML_ST_OVERFLOW` when a `!` function calls it and
//! still wraps when an infallible function — or the host — calls it. Every case is measured on
//! both sides of its boundary, and every failing case beside the caller's own-arithmetic twin.
#![cfg(windows)]

use ml_oracle::Module;
use mlc::emit::emit_artifacts;

mod common;

const OVERFLOW: i32 = mlc::abi::ML_ST_OVERFLOW;
const OUT_OF_RANGE: i32 = mlc::abi::ML_ST_INDEX_OUT_OF_RANGE;

const SRC: &str = "
fn sq(x: i32) -> i32 { return x * x }
fn h3(x: i32) -> i32 { return x * x }
fn h2(x: i32) -> i32 { return h3(x) + 1 }
fn h1(x: i32) -> i32 { return h2(x) - 1 }
fn big(a: i32, b: i32) -> bool { return a * b > 100 }
fn toi(x: f64) -> f64 { return (x as i32) as f64 }
export fn wmul(a: i32, b: i32) -> i32 { return a * b }
fn viaint(a: i32, b: i32) -> i32 { return wmul(a, b) }
export fn scalar(x: i32) -> i32! { return sq(x) }
export fn plain_sq(x: i32) -> i32 { return sq(x) }
export fn own(x: i32) -> i32! { return x * x }
export fn text(x: i32) -> string! { return \"v=\" + sq(x) as string }
export fn arr(x: i32) -> [i32]! {
  result 3
  result[0] = 7
  result[1] = sq(x)
  result[2] = 9
}
export fn chain(x: i32) -> i32! { return h1(x) }
export fn chain_plain(x: i32) -> i32 { return h1(x) }
export fn pred(a: i32, b: i32) -> bool! { return big(a, b) }
export fn conv(x: f64) -> f64! { return toi(x) }
export fn call_export(a: i32, b: i32) -> i32! { return wmul(a, b) }
export fn call_via(a: i32, b: i32) -> i32! { return viaint(a, b) }
export fn and_short(p: bool, x: i32) -> bool! { return p && sq(x) > 0 }
export fn left_index(xs: [i32], x: i32) -> i32! { return xs[5] + sq(x) }
export fn left_helper(xs: [i32], x: i32) -> i32! { return sq(x) + xs[5] }
";

type Un = extern "C" fn(i32, *mut i32) -> i32;
type Bin = extern "C" fn(i32, i32, *mut i32) -> i32;
type Plain = extern "C" fn(i32) -> i32;
type PlainBin = extern "C" fn(i32, i32) -> i32;
type Text = extern "C" fn(i32, *mut u8, i32, *mut i32) -> i32;
type Arr = extern "C" fn(i32, *mut i32, i32, *mut i32) -> i32;
type Pred = extern "C" fn(i32, i32, *mut bool) -> i32;
type Conv = extern "C" fn(f64, *mut f64) -> i32;
type And = extern "C" fn(bool, i32, *mut bool) -> i32;
type Idx = extern "C" fn(*const i32, i32, i32, *mut i32) -> i32;

fn load(tag: &str) -> (common::TempOut, Module) {
    load_src(tag, SRC)
}

fn load_src(tag: &str, src: &str) -> (common::TempOut, Module) {
    let out = common::TempOut::new(&format!("hcp_{tag}"));
    let arts = emit_artifacts(src, "hcp", &out).expect("emit hcp");
    let m = Module::load(arts.dll.to_str().unwrap()).expect("load hcp.dll");
    (out, m)
}

fn sym<T>(m: &Module, name: &[u8]) -> T {
    let p = m.symbol(name).unwrap();
    unsafe { std::mem::transmute_copy(&p) }
}

/// Status and out value; the out value stays at the sentinel when the call fails (D17).
fn un(f: Un, x: i32) -> (i32, i32) {
    let mut v = -7;
    (f(x, &mut v), v)
}

fn bin(f: Bin, a: i32, b: i32) -> (i32, i32) {
    let mut v = -7;
    (f(a, b, &mut v), v)
}

/// **Acceptance A: all three return shapes, beside the caller's own arithmetic.**
///
/// 46340² fits in i32 and 46341² does not. The own-arithmetic function `own` is the control:
/// a helper call has to answer exactly what the same expression written inline answers.
#[test]
fn a_helper_under_a_fallible_caller_fails_like_its_own_arithmetic() {
    let (_out, m) = load("shapes");
    let scalar: Un = sym(&m, b"mlx_scalar\0");
    let own: Un = sym(&m, b"mlx_own\0");
    assert_eq!(un(scalar, 46340), (0, 2147395600));
    assert_eq!(un(scalar, 46341), (OVERFLOW, -7));
    assert_eq!(un(own, 46341), (OVERFLOW, -7), "the control");

    let text: Text = sym(&m, b"mlx_text\0");
    let mut buf = [0u8; 32];
    let mut needed = -7;
    assert_eq!(text(46340, buf.as_mut_ptr(), 32, &mut needed), 0);
    assert_eq!(&buf[..needed as usize - 1], b"v=2147395600");
    let mut needed = -7;
    assert_eq!(text(46341, buf.as_mut_ptr(), 32, &mut needed), OVERFLOW);
    assert_eq!(
        needed, -7,
        "the helper fails before either Q12 pass writes anything"
    );

    // After `result 3`: DP-O6 — the status is -3 and the buffer is undefined, exactly as for
    // the caller's own arithmetic (SPEC §2.4). No new rejection: this program compiles.
    let arr: Arr = sym(&m, b"mlx_arr\0");
    let mut out = [0i32; 3];
    let mut n = -7;
    assert_eq!(arr(3, out.as_mut_ptr(), 3, &mut n), 0);
    assert_eq!(out, [7, 9, 9]);
    assert_eq!(arr(46341, out.as_mut_ptr(), 3, &mut n), OVERFLOW);
    drop(m);
}

/// **Acceptance A and B: a chain three deep, and the same chain under an infallible caller.**
#[test]
fn a_chain_is_checked_under_a_fallible_caller_and_wraps_under_an_infallible_one() {
    let (_out, m) = load("chain");
    let chain: Un = sym(&m, b"mlx_chain\0");
    assert_eq!(un(chain, 46340), (0, 2147395600));
    assert_eq!(
        un(chain, 46341),
        (OVERFLOW, -7),
        "h1 -> h2 -> h3 overflows in h3"
    );

    // B: the same helpers called from a body with no status channel keep today's wrap.
    // 46341² wraps to -2147479015; +1 then -1 leave it there.
    let chain_plain: Plain = sym(&m, b"mlx_chain_plain\0");
    assert_eq!(chain_plain(46341), -2147479015);
    // The one-hop case LANGUAGE.md quotes: `sq(46341)` is -3 under `scalar` (above) and this
    // under an infallible caller — one helper, two answers, by caller.
    let plain_sq: Plain = sym(&m, b"mlx_plain_sq\0");
    assert_eq!(plain_sq(46341), -2147479015);
    drop(m);
}

/// **Acceptance A (DP-P3): the return type does not matter — `bool` and `f64` helpers too.**
#[test]
fn bool_and_f64_helpers_are_checked() {
    let (_out, m) = load("types");
    let pred: Pred = sym(&m, b"mlx_pred\0");
    let p = |a, b| {
        let mut v = false;
        (pred(a, b, &mut v), v)
    };
    assert_eq!(p(10, 11), (0, true));
    assert_eq!(p(10, 10), (0, false));
    assert_eq!(p(65536, 65536).0, OVERFLOW, "a * b in a bool helper");

    let conv: Conv = sym(&m, b"mlx_conv\0");
    let c = |x: f64| {
        let mut v = -7.0;
        (conv(x, &mut v), v)
    };
    assert_eq!(c(3.9), (0, 3.0));
    assert_eq!(c(f64::NAN).0, OVERFLOW, "f64 as i32 in an f64 helper");
    assert_eq!(c(3e9).0, OVERFLOW);
    drop(m);
}

/// **Acceptance C (DP-P2): an exported infallible function is checked when a `!` body calls
/// it — directly or through an internal helper — and wraps when the host calls it.**
#[test]
fn an_exported_callee_is_checked_internally_and_wraps_for_the_host() {
    let (_out, m) = load("export");
    let call_export: Bin = sym(&m, b"mlx_call_export\0");
    let call_via: Bin = sym(&m, b"mlx_call_via\0");
    let wmul: PlainBin = sym(&m, b"mlx_wmul\0");
    assert_eq!(bin(call_export, 46340, 46340), (0, 2147395600));
    assert_eq!(bin(call_export, 65536, 65536), (OVERFLOW, -7));
    assert_eq!(
        bin(call_via, 65536, 65536),
        (OVERFLOW, -7),
        "the check does not stop at an export beneath an internal helper"
    );
    assert_eq!(wmul(65536, 65536), 0, "the host's own call keeps wrapping");
    drop(m);
}

/// **Acceptance D (DP-P4): a helper call is an operand like any other.**
///
/// `&&` does not evaluate its right side when the left is false, and of two failing operands
/// the left one answers — an index `-2` or a helper `-3`, whichever comes first.
#[test]
fn a_helper_call_short_circuits_and_fails_in_source_order() {
    let (_out, m) = load("order");
    let and_short: And = sym(&m, b"mlx_and_short\0");
    let a = |p, x| {
        let mut v = true;
        (and_short(p, x, &mut v), v)
    };
    assert_eq!(a(false, 46341), (0, false), "the helper never ran");
    assert_eq!(a(true, 46341).0, OVERFLOW);

    let xs = [1i32, 2];
    let left_index: Idx = sym(&m, b"mlx_left_index\0");
    let left_helper: Idx = sym(&m, b"mlx_left_helper\0");
    let mut v = -7;
    assert_eq!(left_index(xs.as_ptr(), 2, 46341, &mut v), OUT_OF_RANGE);
    assert_eq!(left_helper(xs.as_ptr(), 2, 46341, &mut v), OVERFLOW);
    drop(m);
}

/// **Acceptance G(1) (DP-P7): the checked copies' names collide with no legal program.**
///
/// A helper `mul`, a user function literally named `ck_mul` and one named `__twin_mul` (the
/// prototype's scheme, which failed with E0428 here) live in one module, and every one of them
/// is checked under the `!` caller.
#[test]
fn checked_copy_names_do_not_collide_with_user_names() {
    let src = "
fn mul(a: i32) -> i32 { return a * 2 }
fn ck_mul(a: i32) -> i32 { return a + 1 }
fn __twin_mul(a: i32) -> i32 { return a * 3 }
export fn f(a: i32) -> i32! { return mul(a) - ck_mul(a) + __twin_mul(a) }
";
    let (_out, m) = load_src("names", src);
    let f: Un = sym(&m, b"mlx_f\0");
    // 20 - 11 + 30: each name reached its own function. Had `ck_mul` resolved to mul's
    // copy the answer would be 30, had `__twin_mul` done so, 29.
    assert_eq!(un(f, 10), (0, 39));
    assert_eq!(un(f, 1073741824), (OVERFLOW, -7), "mul overflows");
    // 2a and a + 1 fit; 3a does not, and `__twin_mul(a)` runs before the outer `+`.
    assert_eq!(
        un(f, 715827883),
        (OVERFLOW, -7),
        "__twin_mul overflows, mul does not"
    );
    drop(m);
}

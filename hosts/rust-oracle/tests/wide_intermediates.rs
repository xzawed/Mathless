//! Wide intermediates — the values a loaded module returns
//! (`SPEC-wide-intermediates` acceptance A, B, C, D and E).
//!
//! In a fallible body a PURE arithmetic tree — leaves that are variables, i32 constants, `len`,
//! or calls to loop-free infallible functions without string arguments — is computed exactly
//! (i64) and checked once, where its value has to become an i32. Everything else keeps the
//! checked-arithmetic rule: the check stands at the operator.
#![cfg(windows)]

use std::sync::mpsc;
use std::time::Duration;

use ml_oracle::Module;
use mlc::emit::emit_artifacts;

mod common;

const OVERFLOW: i32 = mlc::abi::ML_ST_OVERFLOW;
const OUT_OF_RANGE: i32 = mlc::abi::ML_ST_INDEX_OUT_OF_RANGE;

const SRC: &str = "
error E_BIG = 1
fn rate(g: i32) -> i32 {
  if g == 1 { return 5 }
  return 0
}
fn cnt(n: i32) -> i32 {
  let mut i = 0
  while i < n { i = i + 1 }
  return i
}
fn spin(n: i32) -> i32 {
  let mut i = 0
  while i < n { i = i + 0 }
  return i
}
fn blen(s: string) -> i32 { return byte_len(s) }
fn h(a: i32, b: i32) -> i32 { return a * b / b }
export fn quot(a: i32, b: i32) -> i32! { return a * b / b }
export fn avg(a: i32, b: i32) -> i32! { return (a + b) / 2 }
export fn half(a: i32) -> i32! { return a * 2 / 2 }
export fn prod(a: i32, b: i32) -> i32! { return a * b }
export fn deep(a: i32, b: i32, c: i32) -> i32! { return a * b * c / (b * c) }
export fn quot_div(a: i32, b: i32, d: i32) -> i32! { return a * b / d }
export fn split(a: i32, b: i32) -> i32! {
  let t = a * b
  return t / b
}
export fn idx_after(xs: [i32], a: i32, b: i32) -> i32! { return a * b + xs[5] }
export fn idx_exact(xs: [i32], a: i32, b: i32) -> i32! { return a * b / b + xs[5] }
export fn idx_before(xs: [i32], a: i32, b: i32) -> i32! { return xs[5] + a * b }
export fn call_leaf(p: i32, g: i32) -> i32! { return p * rate(g) / 100 }
export fn call_while(p: i32, n: i32) -> i32! { return p * cnt(n) / 100 }
export fn call_str(s: string, p: i32) -> i32! { return p * blen(s) / 100 }
export fn direct_blen(s: string, p: i32) -> i32! { return p * byte_len(s) / 100 }
export fn hang(a: i32, b: i32, n: i32) -> i32! { return a * b + spin(n) }
export fn cmp(a: i32, b: i32) -> bool! { return a + b > 0 }
export fn tof_exact(a: i32, b: i32) -> f64! { return (a * b / b) as f64 }
export fn tof_over(a: i32, b: i32) -> f64! { return (a * b) as f64 }
export fn lit_fits() -> i32! { return 2147483647 + 1 - 1 }
export fn lit_over() -> i32! { return 2147483647 + 1 }
export fn plain(a: i32, b: i32) -> i32 { return a * b / b }
export fn via_copy(a: i32, b: i32) -> i32! { return h(a, b) }
export fn dom(a: i32, b: i32) -> i32! {
  if a * b / b > 1000 { fail E_BIG }
  return 0
}
fn id(x: i32) -> i32 { return x }
export fn len_leaf(xs: [i32], a: i32) -> i32! { return a * len(xs) / len(xs) }
export fn at_assign(a: i32, b: i32) -> i32! {
  let mut t = 0
  t = a * b / b
  return t
}
export fn at_arg(a: i32, b: i32) -> i32! { return id(a * b / b) }
export fn at_arg_over(a: i32, b: i32) -> i32! { return id(a * b) }
export fn at_index(xs: [i32], a: i32, b: i32) -> i32! { return xs[a * b / b - 99999] }
fn sq(x: i32) -> i32 { return x * x }
export fn inl(x: i32) -> i32! { return x * x / x }
export fn ext(x: i32) -> i32! { return sq(x) / x }
export fn ext_let(x: i32) -> i32! {
  let t = x * x
  return t / x
}
";

type Bin = extern "C" fn(i32, i32, *mut i32) -> i32;
type Tri = extern "C" fn(i32, i32, i32, *mut i32) -> i32;
type Un = extern "C" fn(i32, *mut i32) -> i32;
type Idx = extern "C" fn(*const i32, i32, i32, i32, *mut i32) -> i32;
type Arr1 = extern "C" fn(*const i32, i32, i32, *mut i32) -> i32;
type Str = extern "C" fn(*const core::ffi::c_char, i32, *mut i32) -> i32;
type Cmp = extern "C" fn(i32, i32, *mut bool) -> i32;
type Tof = extern "C" fn(i32, i32, *mut f64) -> i32;
type Nil = extern "C" fn(*mut i32) -> i32;
type Plain = extern "C" fn(i32, i32) -> i32;

/// Each test builds into its own directory: the tag is part of the path, and tests in one binary
/// share a process id, so a shared tag would have them racing on one build tree.
fn load(tag: &str) -> (common::TempOut, Module) {
    let out = common::TempOut::new(&format!("wide_{tag}"));
    let arts = emit_artifacts(SRC, "wide", &out).expect("emit wide");
    let m = Module::load(arts.dll.to_str().unwrap()).expect("load wide.dll");
    (out, m)
}

fn sym<T>(m: &Module, name: &[u8]) -> T {
    let p = m.symbol(name).unwrap();
    unsafe { std::mem::transmute_copy(&p) }
}

fn bin(f: Bin, a: i32, b: i32) -> (i32, i32) {
    let mut v = -7;
    (f(a, b, &mut v), v)
}

fn tri(f: Tri, a: i32, b: i32, c: i32) -> (i32, i32) {
    let mut v = -7;
    (f(a, b, c, &mut v), v)
}

/// **Acceptance A: a pure tree is exact, and checked once.**
#[test]
fn a_pure_tree_answers_exactly_and_fails_only_when_the_value_does_not_fit() {
    let (_out, m) = load("exact");
    let quot: Bin = sym(&m, b"mlx_quot\0");
    let avg: Bin = sym(&m, b"mlx_avg\0");
    let half: Un = sym(&m, b"mlx_half\0");
    let prod: Bin = sym(&m, b"mlx_prod\0");
    let deep: Tri = sym(&m, b"mlx_deep\0");
    let quot_div: Tri = sym(&m, b"mlx_quot_div\0");
    let split: Bin = sym(&m, b"mlx_split\0");

    // Only an intermediate overflows; the answer fits.
    assert_eq!(bin(quot, 100000, 100000), (0, 100000));
    assert_eq!(bin(avg, 2000000000, 2000000000), (0, 2000000000));
    let mut v = -7;
    assert_eq!((half(1073741824, &mut v), v), (0, 1073741824));
    // The exact value itself does not fit: still -3.
    assert_eq!(bin(prod, 100000, 100000), (OVERFLOW, -7));
    // "Exact" is bounded by i64 (DP-W5).
    assert_eq!(tri(deep, 5, 1000000, 1000000), (0, 5));
    assert_eq!(tri(deep, 5, i32::MAX, i32::MAX), (OVERFLOW, -7));
    // A zero divisor under an exact product is DP-N4's 0.
    assert_eq!(tri(quot_div, 100000, 100000, 0), (0, 0));
    // The price (§2.6): a `let` is a narrowing point.
    assert_eq!(bin(split, 100000, 100000), (OVERFLOW, -7));

    // `len` is a leaf: 2^30 * 2 overflows only in the middle.
    let len_leaf: Arr1 = sym(&m, b"mlx_len_leaf\0");
    let xs = [1i32, 2];
    let mut v = -7;
    assert_eq!(
        (len_leaf(xs.as_ptr(), 2, 1073741824, &mut v), v),
        (0, 1073741824)
    );
    // Every place that takes the value narrows it once, and only there (DP-W3): an assignment,
    // an argument, an index.
    let at_assign: Bin = sym(&m, b"mlx_at_assign\0");
    let at_arg: Bin = sym(&m, b"mlx_at_arg\0");
    let at_arg_over: Bin = sym(&m, b"mlx_at_arg_over\0");
    assert_eq!(bin(at_assign, 100000, 100000), (0, 100000));
    assert_eq!(bin(at_arg, 100000, 100000), (0, 100000));
    assert_eq!(bin(at_arg_over, 100000, 100000), (OVERFLOW, -7));
    let at_index: Idx = sym(&m, b"mlx_at_index\0");
    let mut v = -7;
    assert_eq!(
        (at_index(xs.as_ptr(), 2, 100000, 100000, &mut v), v),
        (0, 2)
    );
    drop(m);
}

/// **Acceptance B: an impure operator keeps its check where it stands.**
///
/// `a*b + xs[5]`: the overflow is real and comes first, so it answers (-3) — the index behind it
/// never overtakes it. `a*b/b + xs[5]`: the pure part has an exact value, so the next operation
/// that really fails answers (-2). `xs[5] + a*b`: the index is evaluated first, today and now.
#[test]
fn an_impure_operator_keeps_its_check_in_place() {
    let (_out, m) = load("impure");
    let xs = [1i32, 2];
    let run = |name: &[u8]| {
        let f: Idx = sym(&m, name);
        let mut v = -7;
        f(xs.as_ptr(), 2, 100000, 100000, &mut v)
    };
    assert_eq!(run(b"mlx_idx_after\0"), OVERFLOW);
    assert_eq!(run(b"mlx_idx_exact\0"), OUT_OF_RANGE);
    assert_eq!(run(b"mlx_idx_before\0"), OUT_OF_RANGE);

    // Call leaves: a loop-free helper is a leaf; a helper with `while`, one that takes a
    // string, and `byte_len` itself are not — the product is checked at the operator, as today.
    let call_leaf: Bin = sym(&m, b"mlx_call_leaf\0");
    let call_while: Bin = sym(&m, b"mlx_call_while\0");
    assert_eq!(bin(call_leaf, 500000000, 1), (0, 25000000));
    assert_eq!(bin(call_while, 500000000, 5), (OVERFLOW, -7));
    for name in [&b"mlx_call_str\0"[..], b"mlx_direct_blen\0"] {
        let f: Str = sym(&m, name);
        let mut v = -7;
        assert_eq!(
            (f(c"abcde".as_ptr(), 500000000, &mut v), v),
            (OVERFLOW, -7),
            "{}",
            String::from_utf8_lossy(&name[..name.len() - 1])
        );
    }
    drop(m);
}

/// **Acceptance C: no hang.** `spin` never returns for n > 0; it has a `while`, so it is not a
/// leaf and the real overflow before it fails first — exactly as today. Run on a thread with a
/// deadline, because a regression would never return; on timeout the module is leaked rather
/// than unloaded under a running thread.
#[test]
fn a_failure_that_came_first_still_comes_first() {
    let (out, m) = load("hang");
    let hang: Tri = sym(&m, b"mlx_hang\0");
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let mut v = -7;
        let st = hang(100000, 100000, 5, &mut v);
        let _ = tx.send((st, v));
    });
    match rx.recv_timeout(Duration::from_secs(10)) {
        Ok(r) => assert_eq!(r, (OVERFLOW, -7)),
        Err(_) => {
            std::mem::forget(m);
            std::mem::forget(out);
            panic!("mlx_hang did not return within 10 s: the overflow check was deferred past a `while`");
        }
    }
    drop(m);
}

/// **Acceptance D: comparisons, `as f64`, literals, and a domain `fail`.**
#[test]
fn comparisons_conversions_literals_and_domain_errors_see_the_exact_value() {
    let (_out, m) = load("cmp");
    let cmp: Cmp = sym(&m, b"mlx_cmp\0");
    let mut b = false;
    assert_eq!(
        (cmp(i32::MAX, 1, &mut b), b),
        (0, true),
        "exact, not wrapped"
    );

    let tof_exact: Tof = sym(&m, b"mlx_tof_exact\0");
    let tof_over: Tof = sym(&m, b"mlx_tof_over\0");
    let mut x = -7.0;
    assert_eq!((tof_exact(100000, 100000, &mut x), x), (0, 100000.0));
    // DP-W6: narrowed to i32 before the conversion, so a value past i32 is -3, not a rounded f64.
    let mut x = -7.0;
    assert_eq!((tof_over(100000, 100000, &mut x), x), (OVERFLOW, -7.0));

    // DP-W7: DP-O9 restated — a literal expression fails only if its exact value does not fit.
    let lit_fits: Nil = sym(&m, b"mlx_lit_fits\0");
    let lit_over: Nil = sym(&m, b"mlx_lit_over\0");
    let mut v = -7;
    assert_eq!((lit_fits(&mut v), v), (0, i32::MAX));
    let mut v = -7;
    assert_eq!((lit_over(&mut v), v), (OVERFLOW, -7));

    // The exact value exists, so the domain rule answers instead of -3.
    let dom: Bin = sym(&m, b"mlx_dom\0");
    assert_eq!(bin(dom, 100000, 100000), (1, -7));
    assert_eq!(bin(dom, 10, 10), (0, 0));
    drop(m);
}

/// **Acceptance E: infallible bodies keep wrapping; a checked copy widens too.**
#[test]
fn infallible_bodies_are_unchanged_and_checked_copies_widen() {
    let (_out, m) = load("plain");
    // 100000 * 100000 wraps to 1410065408; / 100000 = 14100 — today's answer, unchanged (DP-W10).
    let plain: Plain = sym(&m, b"mlx_plain\0");
    assert_eq!(plain(100000, 100000), 14100);
    // The same expression in a helper under a `!` caller runs as its checked copy, which is a
    // fallible body: exact.
    let via_copy: Bin = sym(&m, b"mlx_via_copy\0");
    assert_eq!(bin(via_copy, 100000, 100000), (0, 100000));
    drop(m);
}

/// **The price (§2.6): a narrowing point in the middle of a computation narrows there.**
///
/// The same arithmetic three ways, at 46341 (46341² does not fit i32): inline it is one pure
/// tree and exact; through a `let`, or through a helper whose `return` narrows, the product is
/// checked on its own and fails. `LANGUAGE.md` quotes these three values.
#[test]
fn extracting_the_intermediate_moves_the_check_to_where_it_is_stored() {
    let (_out, m) = load("price");
    let one = |name: &[u8], x: i32| {
        let f: Un = sym(&m, name);
        let mut v = -7;
        (f(x, &mut v), v)
    };
    assert_eq!(one(b"mlx_inl\0", 46341), (0, 46341));
    assert_eq!(one(b"mlx_ext_let\0", 46341), (OVERFLOW, -7));
    assert_eq!(one(b"mlx_ext\0", 46341), (OVERFLOW, -7));
    drop(m);
}

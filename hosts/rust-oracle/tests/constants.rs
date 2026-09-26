//! Constant declarations — the values a loaded module returns (`SPEC-constants` acceptance A).
//!
//! A constant is its literal (DP-K5), so every value here is compared with the same function
//! written with the literal inline. The negative cases are the reason this file exists: before
//! this slice no negative integer literal ever reached codegen as one node — `-1` in source is
//! a negation of `1` — and the i32 operators are emitted as methods, where `-1i32.wrapping_add(x)`
//! would parse as `-(1i32.wrapping_add(x))`.
#![cfg(windows)]

use ml_oracle::Module;
use mlc::emit::emit_artifacts;

mod common;

type I32Fn = extern "C" fn(i32) -> i32;
type F64Fn = extern "C" fn(f64) -> f64;
type BoolFn = extern "C" fn(bool) -> bool;

const SRC: &str = "
const RATE = 0.1
const HALF_DOWN = -0.5
const STRICT = true
export const NONE = -1
export const LOW = -2147483648
export const HIGH = 2147483647

export fn neg_left(x: i32) -> i32 { return NONE + x }
export fn neg_times(x: i32) -> i32 { return NONE * x }
export fn low_plus(x: i32) -> i32 { return LOW + x }
export fn low_minus(x: i32) -> i32 { return LOW - x }
export fn high_plus(x: i32) -> i32 { return HIGH + x }
export fn rate(x: f64) -> f64 { return RATE * x }
export fn half_down(x: f64) -> f64 { return HALF_DOWN * x }
export fn strict(b: bool) -> bool { return STRICT && b }
";

fn load(tag: &str) -> (common::TempOut, Module) {
    let out = common::TempOut::new(&format!("consts_{tag}"));
    let arts = emit_artifacts(SRC, "consts", &out).expect("emit consts");
    let m = Module::load(arts.dll.to_str().unwrap()).expect("load consts.dll");
    (out, m)
}

fn sym<T>(m: &Module, name: &[u8]) -> T {
    let p = m.symbol(name).unwrap();
    unsafe { std::mem::transmute_copy(&p) }
}

/// **A negative constant on the left of a method-emitted operator keeps its sign.**
#[test]
fn a_negative_constant_is_one_value_not_a_negated_call() {
    let (_out, m) = load("neg");
    let neg_left: I32Fn = sym(&m, b"mlx_neg_left\0");
    let neg_times: I32Fn = sym(&m, b"mlx_neg_times\0");
    // -1 + 5 = 4. Parsed as -(1 + 5) it would be -6.
    assert_eq!(neg_left(5), 4);
    assert_eq!(neg_times(7), -7);

    // i32::MIN, which source cannot spell as an expression (`2147483648` does not fit before
    // it is negated). Wrapping is the language's rule, so these are exact.
    let low_plus: I32Fn = sym(&m, b"mlx_low_plus\0");
    let low_minus: I32Fn = sym(&m, b"mlx_low_minus\0");
    let high_plus: I32Fn = sym(&m, b"mlx_high_plus\0");
    assert_eq!(low_plus(1), i32::MIN + 1);
    assert_eq!(low_minus(1), i32::MAX, "MIN - 1 wraps to MAX");
    assert_eq!(high_plus(1), i32::MIN, "MAX + 1 wraps to MIN");
    drop(m);
}

/// **Internal f64 and bool constants are their literals.**
#[test]
fn internal_constants_of_every_allowed_type() {
    let (_out, m) = load("types");
    let rate: F64Fn = sym(&m, b"mlx_rate\0");
    let half_down: F64Fn = sym(&m, b"mlx_half_down\0");
    let strict: BoolFn = sym(&m, b"mlx_strict\0");
    assert_eq!(rate(200.0).to_bits(), (0.1f64 * 200.0).to_bits());
    assert_eq!(half_down(3.0).to_bits(), (-0.5f64 * 3.0).to_bits());
    assert!(strict(true));
    assert!(!strict(false));
    drop(m);
}

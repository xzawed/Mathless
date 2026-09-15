//! Integer-type (`i32`) slice — acceptance A/B/C (E2): the oracle loads an `i32` module and
//! calls it. i32 maps to a plain C `int32_t` across the ABI.
#![cfg(windows)]

use ml_oracle::{pe, Module};
use mlc::emit::emit_artifacts;

mod common;

#[test]
fn oracle_loads_and_calls_an_i32_function() {
    let src = include_str!("../../../examples/add.mls");
    let out = common::TempOut::new("i32");
    let arts = emit_artifacts(src, "add", &out).expect("emit add");

    let m = Module::load(arts.dll.to_str().unwrap()).expect("load add.dll");
    let ver: extern "C" fn() -> u32 =
        unsafe { std::mem::transmute(m.symbol(b"ml_module_abi_version\0").unwrap()) };
    let add: extern "C" fn(i32, i32) -> i32 =
        unsafe { std::mem::transmute(m.symbol(b"mlx_add\0").unwrap()) };
    assert_eq!(ver(), mlc::ML_MODULE_ABI_VERSION, "abi version");
    assert_eq!(add(2, 3), 5, "i32 add");
    assert_eq!(add(10, -4), 6, "i32 add with a negative");

    let mut exports = pe::read_exports(&arts.dll).expect("read exports");
    exports.sort();
    assert_eq!(
        exports,
        vec![
            "ml_iface_hash_add".to_string(),
            "ml_module_abi_version".to_string(),
            "mlx_add".to_string()
        ]
    );

    drop(m);
}

/// What i32 `+`, `-` and `*` ANSWER when they overflow — read back through the C ABI.
///
/// **Why this exists.** codegen lowers all three to `wrapping_add`/`wrapping_sub`/
/// `wrapping_mul`, explicitly and unconditionally, so that an overflow cannot panic: in a
/// `no_std` cdylib the `#[panic_handler]` is `loop {}`, and a panic would HANG the host rather
/// than crash it (STATUS §5-4). That decision is sound and deliberate.
///
/// What was missing is a measurement of it. Every existing pin reads the GENERATED SOURCE —
/// `i32_division.rs` asserts `rust.contains("}).wrapping_mul(c)")` — which is the §7-3 shape
/// applied to this repo's own tests: it proves the emitter wrote a word, not that a host gets
/// a number. Nothing anywhere loaded a module and asked what `2000000000 + 2000000000` is.
///
/// Measured here, and the values are written out rather than computed, because a computed
/// expectation (`a.wrapping_add(b)`) would pass against any lowering that happens to match
/// Rust's — including one that changed for a reason nobody intended.
///
/// **All three operators, not one.** The first draft of this test said "`+`, `-` and `*`" and
/// called only `mlx_add` — verify caught it. That is #211's rule turned on a test rather than
/// a guard: the SCOPE of a claim is part of the claim, and `-` and `*` are separate arms of
/// the same `match` in codegen, so "add wraps" says nothing about either of them.
#[test]
fn i32_arithmetic_wraps_and_these_are_the_values() {
    // Written here rather than taken from `examples/add.mls`, which exports only `add`: this
    // test needs one module carrying all three arms.
    let src = "export fn add(a: i32, b: i32) -> i32 { return a + b }\n\
               export fn sub(a: i32, b: i32) -> i32 { return a - b }\n\
               export fn mul(a: i32, b: i32) -> i32 { return a * b }";
    let out = common::TempOut::new("i32_wrap");
    let arts = emit_artifacts(src, "wrap", &out).expect("emit wrap");

    let m = Module::load(arts.dll.to_str().unwrap()).expect("load wrap.dll");
    let add: extern "C" fn(i32, i32) -> i32 =
        unsafe { std::mem::transmute(m.symbol(b"mlx_add\0").unwrap()) };
    let sub: extern "C" fn(i32, i32) -> i32 =
        unsafe { std::mem::transmute(m.symbol(b"mlx_sub\0").unwrap()) };
    let mul: extern "C" fn(i32, i32) -> i32 =
        unsafe { std::mem::transmute(m.symbol(b"mlx_mul\0").unwrap()) };

    // Two positive operands, a negative answer, and status is not even a concept here: `add`
    // is infallible, so there is no channel through which a host could be told (D17 only
    // gives a function a status when its signature says `!`).
    assert_eq!(add(2_000_000_000, 2_000_000_000), -294_967_296);
    assert_eq!(add(i32::MAX, 1), i32::MIN);
    assert_eq!(add(i32::MIN, -1), i32::MAX);

    // `-` past the bottom, and `*` well past the top: `100000 * 100000` is 10^10, which is
    // not an exotic quantity — it is a count of cents, or milliseconds in four months.
    assert_eq!(sub(i32::MIN, 1), i32::MAX);
    assert_eq!(sub(-2_000_000_000, 2_000_000_000), 294_967_296);
    assert_eq!(mul(100_000, 100_000), 1_410_065_408);
    assert_eq!(mul(i32::MAX, 2), -2);

    // …and none of them saturates. The distinction matters because `f64 as i32` in the SAME
    // language DOES saturate (`numeric_conversion.rs`, a documented language rule), so an
    // author who learned "i32 clamps" from the conversion rule would be wrong about
    // arithmetic. Asserting the inequality pins the difference itself, not just the value.
    assert_ne!(
        add(i32::MAX, 1),
        i32::MAX,
        "arithmetic wraps; only the f64 conversion saturates"
    );
    assert_ne!(mul(i32::MAX, 2), i32::MAX, "same for `*`");
    assert_ne!(sub(i32::MIN, 1), i32::MIN, "same for `-`");

    drop(m);
}

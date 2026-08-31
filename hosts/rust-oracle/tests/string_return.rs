//! string-return slice — acceptance A/B/B2..B7/C (E2).
//!
//! This is where the slice's value is. `SPEC-string-return.md` §0.2 says the point of going
//! first is to find flaws in the Q12 protocol cheaply, so the centre of gravity here is not
//! "is the string right" but the PROTOCOL EDGES: truncation, the probe, exact fit, the empty
//! string, a domain error, and a negative capacity.
//!
//! Every one of those is measured with a **canary buffer** (§3-B7): filled with 0xAA, with
//! more canary past `ml_cap`. `STATUS.md` §7 names the cheap-vs-expensive measurement
//! asymmetry as the cause of four escaped defects — the canary is the multiplier, because it
//! turns "read the emitted helper carefully" into a byte comparison.
#![cfg(windows)]

use core::ffi::c_char;

use ml_oracle::{pe, Module};
use mlc::emit::emit_artifacts;

/// The status the module returns when the buffer cannot hold the result (DP-T6). Mirrors
/// `ML_ST_INSUFFICIENT_BUFFER` in the generated header.
const ML_ST_INSUFFICIENT_BUFFER: i32 = -1;
const ML_ERR_E_UNKNOWN_SCAC: i32 = 1;

/// "UPS Ground" is 10 bytes, so 11 with the NUL — and 11 is the unit the ABI speaks (DP-T4).
const UPS: &[u8] = b"UPS Ground\0";

type CarrierFn = extern "C" fn(*const c_char, *mut u8, i32, *mut i32) -> i32;

fn build(tag: &str) -> (std::path::PathBuf, Module) {
    let src = include_str!("../../../examples/carrier.mls");
    let out = std::env::temp_dir().join(format!("mlc_sret_{tag}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&out);
    std::fs::create_dir_all(&out).unwrap();
    let arts = emit_artifacts(src, "carrier", &out).expect("emit carrier");
    let m = Module::load(arts.dll.to_str().unwrap()).expect("load carrier.dll");
    (out, m)
}

fn carrier_name(m: &Module) -> CarrierFn {
    unsafe { std::mem::transmute(m.symbol(b"mlx_carrier_name\0").unwrap()) }
}

/// A buffer the module must not scribble on outside `cap` bytes. 0xAA is chosen because it is
/// neither NUL nor ASCII: a stray terminator and a stray character are both visible.
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
    /// Every byte from `from` to the end must still be 0xAA.
    fn intact_from(&self, from: usize) -> bool {
        self.bytes[from..].iter().all(|&b| b == 0xAA)
    }
    fn is_pristine(&self) -> bool {
        self.intact_from(0)
    }
}

#[test]
fn the_measured_rule_returns_the_right_bytes() {
    // Section 3-B.
    let (out, m) = build("b");
    let f = carrier_name(&m);

    let mut buf = Canary::new();
    let mut needed = -7i32;
    let status = f(c"UPSN".as_ptr(), buf.ptr(), 64, &mut needed);
    assert_eq!(status, 0);
    assert_eq!(needed, 11, "total bytes INCLUDING the NUL (DP-T4)");
    assert_eq!(&buf.bytes[..11], UPS);
    assert!(
        buf.intact_from(11),
        "the module must write exactly `needed` bytes and not one more: {:?}",
        &buf.bytes[..20]
    );

    // Echoing an input parameter is legal and must copy, not alias.
    let mut buf = Canary::new();
    let mut needed = -7i32;
    assert_eq!(f(c"SELF".as_ptr(), buf.ptr(), 64, &mut needed), 0);
    assert_eq!(needed, 5);
    assert_eq!(&buf.bytes[..5], b"SELF\0");

    drop(m);
    let _ = std::fs::remove_dir_all(&out);
}

#[test]
fn truncation_is_a_failure_and_writes_nothing() {
    // Section 3-B2, and the single most important measurement in the slice. Q12 chose
    // "truncation is a failure, not a short success", and DP-T2 confirmed "buf untouched" —
    // so a wrong implementation that filled 4 bytes and reported success is exactly what the
    // canary catches. `strlcpy` reflexes produce precisely that.
    let (out, m) = build("b2");
    let f = carrier_name(&m);

    let mut buf = Canary::new();
    let mut needed = -7i32;
    let status = f(c"UPSN".as_ptr(), buf.ptr(), 4, &mut needed);
    assert_eq!(status, ML_ST_INSUFFICIENT_BUFFER);
    assert_eq!(needed, 11, "the exact size to allocate, same unit as cap");
    assert!(
        buf.is_pristine(),
        "not one byte may be written on truncation: {:?}",
        &buf.bytes[..16]
    );

    drop(m);
    let _ = std::fs::remove_dir_all(&out);
}

#[test]
fn the_probe_converges_in_exactly_two_calls() {
    // Section 3-B3. DP-T7: `ml_buf` may be NULL iff `ml_cap == 0`. The probe is the documented
    // way to learn the length, so this EXECUTES it rather than trusting the contract — the
    // difference between a resolved question and a remembered one.
    let (out, m) = build("b3");
    let f = carrier_name(&m);

    let mut needed = -7i32;
    let status = f(c"UPSN".as_ptr(), std::ptr::null_mut(), 0, &mut needed);
    assert_eq!(
        status, ML_ST_INSUFFICIENT_BUFFER,
        "a NULL probe must not crash"
    );
    assert_eq!(needed, 11);

    // Second call with exactly what the module asked for. Same unit, so this always fits —
    // no `+1`, no doubling loop.
    let mut exact = vec![0xAAu8; needed as usize];
    let mut needed2 = -7i32;
    let status = f(c"UPSN".as_ptr(), exact.as_mut_ptr(), needed, &mut needed2);
    assert_eq!(status, 0, "the size the module asked for must be enough");
    assert_eq!(needed2, 11);
    assert_eq!(&exact[..], UPS);

    drop(m);
    let _ = std::fs::remove_dir_all(&out);
}

#[test]
fn an_exact_fit_is_success_and_one_less_is_truncation() {
    // Section 3-B4. The classic off-by-one sits exactly here, and it is invisible to any test
    // that only uses a generous buffer.
    let (out, m) = build("b4");
    let f = carrier_name(&m);

    let mut buf = Canary::new();
    let mut needed = 0i32;
    assert_eq!(f(c"UPSN".as_ptr(), buf.ptr(), 11, &mut needed), 0);
    assert_eq!(&buf.bytes[..11], UPS);
    assert!(buf.intact_from(11));

    let mut buf = Canary::new();
    let mut needed = 0i32;
    assert_eq!(
        f(c"UPSN".as_ptr(), buf.ptr(), 10, &mut needed),
        ML_ST_INSUFFICIENT_BUFFER,
        "10 bytes holds the characters but not the NUL — that is truncation"
    );
    assert!(buf.is_pristine());
    assert_eq!(needed, 11);

    drop(m);
    let _ = std::fs::remove_dir_all(&out);
}

#[test]
fn an_empty_result_is_a_value_not_an_absence() {
    // Section 3-B5. `*needed == 0` must be impossible: an empty string still needs its NUL,
    // so it is 1. That is what makes 0 an unambiguous "no string was produced".
    let (out, m) = build("b5");
    let f = carrier_name(&m);

    let mut buf = Canary::new();
    let mut needed = -7i32;
    assert_eq!(f(c"NONE".as_ptr(), buf.ptr(), 1, &mut needed), 0);
    assert_eq!(needed, 1, "the NUL alone");
    assert_eq!(buf.bytes[0], 0);
    assert!(buf.intact_from(1));

    // And it does not fit in zero.
    let mut buf = Canary::new();
    let mut needed = -7i32;
    assert_eq!(
        f(c"NONE".as_ptr(), buf.ptr(), 0, &mut needed),
        ML_ST_INSUFFICIENT_BUFFER
    );
    assert_eq!(needed, 1);
    assert!(buf.is_pristine());

    drop(m);
    let _ = std::fs::remove_dir_all(&out);
}

#[test]
fn a_domain_error_touches_neither_the_buffer_nor_needed() {
    // Section 3-B6 / hazard H13. `*needed` is deliberately left alone (DP-T8) rather than
    // zeroed, because a zero would be indistinguishable from "an empty string" to a host that
    // zero-initialises it. The canary is what turns that from a promise into a measurement.
    let (out, m) = build("b6");
    let f = carrier_name(&m);

    let mut buf = Canary::new();
    let mut needed = -7i32;
    let status = f(c"ZZ99".as_ptr(), buf.ptr(), 64, &mut needed);
    assert_eq!(status, ML_ERR_E_UNKNOWN_SCAC, "a positive D17 domain code");
    assert!(buf.is_pristine(), "a failed call writes no bytes");
    assert_eq!(needed, -7, "and does not touch *needed either");

    drop(m);
    let _ = std::fs::remove_dir_all(&out);
}

#[test]
fn a_negative_capacity_is_truncation_not_an_overrun() {
    // Section 3-G. A signed capacity reinterpreted as unsigned is a documented real-world
    // overrun (MSVC says so for `_snprintf` with a negative count). And because a panic in a
    // generated module spins in `ml_panic` rather than crashing, getting this wrong would
    // hang the calling thread — so the test also proves the call RETURNS.
    let (out, m) = build("g");
    let f = carrier_name(&m);

    let mut buf = Canary::new();
    let mut needed = -7i32;
    let status = f(c"UPSN".as_ptr(), buf.ptr(), -1, &mut needed);
    assert_eq!(status, ML_ST_INSUFFICIENT_BUFFER);
    assert_eq!(needed, 11);
    assert!(buf.is_pristine(), "a negative cap must never be a huge one");

    let mut buf = Canary::new();
    let mut needed = -7i32;
    assert_eq!(
        f(c"UPSN".as_ptr(), buf.ptr(), i32::MIN, &mut needed),
        ML_ST_INSUFFICIENT_BUFFER
    );
    assert!(buf.is_pristine());

    drop(m);
    let _ = std::fs::remove_dir_all(&out);
}

#[test]
fn a_declared_out_composes_with_the_buffer_triple() {
    // DP-O1: declared outs in source order, then the return value last — the return value is
    // now three slots wide. If the generator ever emitted them in the other order, `tier`
    // would take the buffer pointer and this would write 8 bytes over an i32.
    let (out, m) = build("o1");
    let label: extern "C" fn(*const c_char, *mut i32, *mut u8, i32, *mut i32) -> i32 =
        unsafe { std::mem::transmute(m.symbol(b"mlx_carrier_label\0").unwrap()) };

    let mut buf = Canary::new();
    let mut tier = -7i32;
    let mut needed = -7i32;
    assert_eq!(
        label(c"UPSN".as_ptr(), &mut tier, buf.ptr(), 64, &mut needed),
        0
    );
    assert_eq!(tier, 1);
    assert_eq!(needed, 11);
    assert_eq!(&buf.bytes[..11], UPS);

    // Truncation still writes the declared out — it was assigned before the return, and
    // DP-O3 is explicit that outs are not rolled back. The contract is "do not read on
    // status != 0", which is exactly why this is measured rather than assumed.
    let mut buf = Canary::new();
    let mut tier = -7i32;
    let mut needed = -7i32;
    assert_eq!(
        label(c"UPSN".as_ptr(), &mut tier, buf.ptr(), 2, &mut needed),
        ML_ST_INSUFFICIENT_BUFFER
    );
    assert!(buf.is_pristine(), "the string buffer is still untouched");
    assert_eq!(needed, 11);

    drop(m);
    let _ = std::fs::remove_dir_all(&out);
}

#[test]
fn the_module_gains_no_export_and_its_imports_are_compared_not_claimed() {
    // Section 3-C. The import set is measured against a control module built the same way —
    // never as an absolute claim. That matters more here than in the string-INPUT slice: a
    // byte copy is exactly what rustc idiom-recognises into `memcpy`, and `vcruntime140!memcpy`
    // is already in every cdylib's baseline. The honest question is only "did it grow".
    let (out, m) = build("c");
    drop(m);
    let dll = out.join("carrier.dll");

    let mut names = pe::read_exports(&dll).expect("read exports");
    names.sort();
    assert_eq!(
        names,
        vec![
            "ml_module_abi_version".to_string(),
            "mlx_carrier_label".to_string(),
            "mlx_carrier_name".to_string(),
        ],
        "ml_strout is internal and must not be exported"
    );

    let base_out = std::env::temp_dir().join(format!("mlc_sret_cbase_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base_out);
    std::fs::create_dir_all(&base_out).unwrap();
    let base = emit_artifacts(
        "export fn f(x: f64) -> f64 { return x * 2.0 }",
        "baseline",
        &base_out,
    )
    .expect("emit baseline");
    let baseline = pe::read_imports(&base.dll).expect("baseline imports");

    let imports = pe::read_imports(&dll).expect("read imports");
    let size = std::fs::metadata(&dll).unwrap().len();
    println!("carrier.dll = {size} B, imports = {imports:?}");
    assert_eq!(
        imports, baseline,
        "the two-pass byte copy must not grow the import set"
    );
    assert!(size < 60_000, "still a small stripped module: {size}");

    let _ = std::fs::remove_dir_all(&out);
    let _ = std::fs::remove_dir_all(&base_out);
}

/// The combination no test covered, and the one a doc sentence got wrong twice.
///
/// `HOST_ABI.md` says "the module writes nothing on the failure path". That is true of the
/// STRING BUFFER — the only write site is `return`, which exits — and it is NOT true of a
/// declared scalar `out`, which DP-O3 explicitly declines to roll back. `examples/carrier.mls`
/// could not show the difference: `carrier_label` has no `fail`. This module does.
#[test]
fn a_failing_string_return_leaves_the_buffer_alone_but_not_a_declared_out() {
    const SRC: &str = "error E_BAD = 3\n\
                       export fn both(code: string, out tier: i32) -> string! {\n\
                         tier = 7\n\
                         if code == \"BAD\" { fail E_BAD }\n\
                         return \"ok\"\n\
                       }";
    let out = std::env::temp_dir().join(format!("mlc_sret_mix_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&out);
    std::fs::create_dir_all(&out).unwrap();
    let arts = emit_artifacts(SRC, "both", &out).expect("emit both");
    let m = Module::load(arts.dll.to_str().unwrap()).expect("load both.dll");
    let f: extern "C" fn(*const c_char, *mut i32, *mut u8, i32, *mut i32) -> i32 =
        unsafe { std::mem::transmute(m.symbol(b"mlx_both\0").unwrap()) };

    let mut buf = Canary::new();
    let mut tier = -7i32;
    let mut needed = -7i32;
    let status = f(c"BAD".as_ptr(), &mut tier, buf.ptr(), 64, &mut needed);

    assert_eq!(status, 3);
    assert!(
        buf.is_pristine(),
        "the string buffer IS untouched on failure — the only write site is `return`"
    );
    assert_eq!(needed, -7, "and so is *ml_needed");
    assert_eq!(
        tier, 7,
        "but a declared scalar out assigned before `fail` STAYS written (DP-O3). This is why \
         the host contract is 'do not read on status != 0', not 'the module does not write'"
    );

    drop(m);
    let _ = std::fs::remove_dir_all(&out);
}

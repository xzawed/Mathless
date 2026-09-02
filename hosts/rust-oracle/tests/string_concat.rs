//! WK4 (SPEC-string-concat §3-A..F) — the built string, measured on a real DLL.
//!
//! The tool that carries this file is the **canary buffer** from #92: fill it with 0xAA and a
//! single byte comparison answers "was anything written". Truncation and a domain failure both
//! have to leave it untouched, and with a BUILT string that is a stronger claim than it was for
//! a borrowed one — the module now has bytes of its own it could have started copying.
#![cfg(windows)]

use std::ffi::c_char;

use ml_oracle::{pe, Module};
use mlc::emit::emit_artifacts;

mod common;

/// A loaded module and the tree it came from.
///
/// **Field order is load-bearing.** Rust drops struct fields in declaration order, so the
/// `Module` must come FIRST: `FreeLibrary` has to run before anything tries to delete the
/// `.dll` underneath it. Written the other way round this leaked six trees per run — measured,
/// and worth the comment because the compiler will never mention it.
struct M {
    m: Module,
    _dir: common::TempOut,
}

fn build(tag: &str) -> M {
    let dir = common::TempOut::new(&format!("concat_{tag}"));
    let arts = emit_artifacts(
        include_str!("../../../examples/receipt.mls"),
        "receipt",
        &dir,
    )
    .expect("emit receipt");
    let m = Module::load(arts.dll.to_str().unwrap()).expect("load receipt.dll");
    M { m, _dir: dir }
}

type NameFn = extern "C" fn(*const c_char, *const c_char, *mut u8, i32, *mut i32) -> i32;
type LineFn = extern "C" fn(*const c_char, i32, i32, *mut u8, i32, *mut i32) -> i32;
type IntFn = extern "C" fn(i32, *mut u8, i32, *mut i32) -> i32;

fn sym<T>(m: &Module, name: &[u8]) -> T {
    unsafe { std::mem::transmute_copy(&m.symbol(name).expect("symbol")) }
}

/// Read the NUL-terminated result out of a buffer.
fn text(buf: &[u8]) -> String {
    let end = buf.iter().position(|b| *b == 0).unwrap_or(buf.len());
    String::from_utf8_lossy(&buf[..end]).into_owned()
}

#[test]
fn the_measured_business_rules_produce_the_right_bytes() {
    // §3-A. These are the rules SPEC §0.1 measured as blocked before the slice.
    let h = build("values");
    let full_name: NameFn = sym(&h.m, b"mlx_full_name\0");
    let line: LineFn = sym(&h.m, b"mlx_receipt_line\0");

    let mut buf = [0u8; 64];
    let mut needed = -1i32;

    let st = full_name(
        c"Gildong".as_ptr(),
        c"Hong".as_ptr(),
        buf.as_mut_ptr(),
        buf.len() as i32,
        &mut needed,
    );
    assert_eq!(st, 0);
    assert_eq!(text(&buf), "Hong Gildong");

    let st = line(
        c"WIDGET".as_ptr(),
        3,
        15000,
        buf.as_mut_ptr(),
        buf.len() as i32,
        &mut needed,
    );
    assert_eq!(st, 0);
    assert_eq!(
        text(&buf),
        "WIDGET x 3 = 45000",
        "digits the module produced"
    );
}

#[test]
fn the_integer_boundaries_render_exactly() {
    // §3-D. `i32::MIN` is the one that bites: negating it overflows, so the magnitude is taken
    // in u32. A wrong answer here is a wrong invoice, not a crash.
    let h = build("bounds");
    let label: IntFn = sym(&h.m, b"mlx_label\0");

    for (v, want) in [
        (0i32, "0"),
        (-1, "-1"),
        (7, "7"),
        (i32::MAX, "2147483647"),
        (i32::MIN, "-2147483648"),
    ] {
        let mut buf = [0u8; 32];
        let mut needed = -1i32;
        let st = label(v, buf.as_mut_ptr(), buf.len() as i32, &mut needed);
        assert_eq!(st, 0, "label({v})");
        assert_eq!(text(&buf), want, "label({v})");
        // §3-E: the count from pass 1 must equal what pass 2 actually wrote, or the next
        // longer string walks off the end of the host's buffer.
        assert_eq!(
            needed,
            want.len() as i32 + 1,
            "label({v}): needed must be the bytes written plus the NUL"
        );
    }
}

#[test]
fn truncation_by_one_byte_writes_nothing_at_all() {
    // §3-B with the canary. "WIDGET x 3 = 45000" needs 19 bytes including the NUL; 18 is one
    // short. Q12 says that is a FAILURE, and DP-T2 was rejected, so the buffer stays pristine.
    let h = build("trunc");
    let line: LineFn = sym(&h.m, b"mlx_receipt_line\0");

    let mut buf = [0xAAu8; 64];
    let mut needed = -1i32;
    let st = line(
        c"WIDGET".as_ptr(),
        3,
        15000,
        buf.as_mut_ptr(),
        18,
        &mut needed,
    );
    assert!(st < 0, "one byte short must fail, not truncate");
    assert_eq!(needed, 19, "needed is exact on the failure path too");
    assert!(
        buf.iter().all(|b| *b == 0xAA),
        "not one byte may be written when the call fails: {buf:?}"
    );
}

#[test]
fn a_domain_failure_leaves_the_buffer_alone_too() {
    // D17: a failed call writes no out-param, and the buffer is one.
    let h = build("fail");
    let summary: IntFn = sym(&h.m, b"mlx_summary\0");

    let mut buf = [0xAAu8; 64];
    let mut needed = -1i32;
    let st = summary(0, buf.as_mut_ptr(), buf.len() as i32, &mut needed);
    assert_eq!(st, 1, "the declared positive domain code");
    assert!(buf.iter().all(|b| *b == 0xAA), "{buf:?}");
}

#[test]
fn the_probe_still_converges_in_two_calls() {
    // §3-C. The probe is the documented way to learn the length, so it must be safe with a
    // NULL buffer even when the result is BUILT rather than borrowed.
    let h = build("probe");
    let line: LineFn = sym(&h.m, b"mlx_receipt_line\0");

    let mut needed = -1i32;
    let st = line(
        c"WIDGET".as_ptr(),
        3,
        15000,
        std::ptr::null_mut(),
        0,
        &mut needed,
    );
    assert!(st < 0, "a zero capacity is truncation");
    assert_eq!(needed, 19);

    let mut exact = vec![0u8; needed as usize];
    let st = line(
        c"WIDGET".as_ptr(),
        3,
        15000,
        exact.as_mut_ptr(),
        needed,
        &mut needed,
    );
    assert_eq!(st, 0, "the retry at exactly `needed` succeeds");
    assert_eq!(text(&exact), "WIDGET x 3 = 45000");
}

#[test]
fn a_borrowed_return_still_takes_the_old_path() {
    // The #92 shape must not regress just because a built form now exists next to it.
    let h = build("borrowed");
    let summary: IntFn = sym(&h.m, b"mlx_summary\0");

    let mut buf = [0u8; 32];
    let mut needed = -1i32;
    assert_eq!(summary(1, buf.as_mut_ptr(), 32, &mut needed), 0);
    assert_eq!(text(&buf), "1 item");

    let mut buf = [0u8; 32];
    assert_eq!(summary(5, buf.as_mut_ptr(), 32, &mut needed), 0);
    assert_eq!(text(&buf), "5 items", "built, in the same function");
}

#[test]
fn building_a_string_adds_no_import_and_no_export() {
    // §3-F and §3-H. Measured as a SET COMPARISON against a module built the same way — an
    // absolute claim would be false, because every cdylib already imports CRT startup and
    // `vcruntime140!memcpy` through the DllMain stub (#89 measured 25).
    let with = common::TempOut::new("concat_imp_with");
    let without = common::TempOut::new("concat_imp_without");
    let a = emit_artifacts(
        include_str!("../../../examples/receipt.mls"),
        "receipt",
        &with,
    )
    .expect("emit");
    let b = emit_artifacts(
        "export fn f(s: string) -> string! { return s }",
        "plainstr",
        &without,
    )
    .expect("emit");

    let (mut ia, mut ib) = (
        pe::read_imports(&a.dll).expect("imports"),
        pe::read_imports(&b.dll).expect("imports"),
    );
    ia.sort();
    ib.sort();
    assert_eq!(ia, ib, "producing digits must not pull in a formatter");

    let mut ex = pe::read_exports(&a.dll).expect("exports");
    ex.sort();
    assert_eq!(
        ex,
        vec![
            "ml_iface_hash".to_string(),
            "ml_module_abi_version".to_string(),
            "mlx_full_name".to_string(),
            "mlx_label".to_string(),
            "mlx_receipt_line".to_string(),
            "mlx_summary".to_string(),
        ],
        "four exports plus the two reserved symbols"
    );
}

//! non-ascii-literals slice — `SPEC-non-ascii-literals` acceptance A/B/F/G/I (E2).
//!
//! The rule is asymmetric on purpose and the values are what show it: the module HANDS OUT
//! UTF-8 bytes, and the host receives exactly those. It never compares them, because a C or
//! Delphi host sending the same characters in its ANSI code page sends different bytes — that
//! comparison would answer `false` with status 0 and no warning, which is the trap
//! `HOST_ABI.md` section 5 already measured for Delphi.
#![cfg(windows)]

use core::ffi::c_char;

use ml_oracle::{pe, Module};
use mlc::emit::emit_artifacts;

mod common;

type LabelFn = extern "C" fn(*const c_char, *mut u8, i32, *mut i32) -> i32;

fn build(tag: &str) -> (common::TempOut, Module) {
    let out = common::TempOut::new(&format!("na_{tag}"));
    let src = include_str!("../../../examples/claim.mls");
    let arts = emit_artifacts(src, "claim", &out).expect("emit claim");
    let m = Module::load(arts.dll.to_str().unwrap()).expect("load claim.dll");
    (out, m)
}

fn label(m: &Module, sym: &[u8]) -> LabelFn {
    unsafe { std::mem::transmute(m.symbol(sym).unwrap()) }
}

fn call(f: LabelFn, arg: &core::ffi::CStr) -> (i32, i32, String) {
    let mut buf = [0u8; 64];
    let mut needed = -7i32;
    let st = f(arg.as_ptr(), buf.as_mut_ptr(), 64, &mut needed);
    let n = (needed.max(1) as usize) - 1;
    (st, needed, String::from_utf8_lossy(&buf[..n]).into_owned())
}

/// **Acceptance A and F** — the bytes arrive, and `ml_needed` counts BYTES.
#[test]
fn a_returned_korean_label_arrives_byte_for_byte() {
    let (_out, m) = build("value");
    let status_label = label(&m, b"mlx_status_label\0");

    // A. Two Korean characters are SIX bytes, plus the NUL. A host sizing by characters
    // allocates 3 and truncates — which is why F is measured in the same call as A.
    assert_eq!(call(status_label, c"AP"), (0, 7, "승인".to_string()));
    assert_eq!(call(status_label, c"RJ"), (0, 7, "거절".to_string()));
    assert_eq!(call(status_label, c"RV"), (0, 10, "심사중".to_string()));

    // An unknown code is the module's own domain error, not an encoding question.
    let mut buf = [0u8; 64];
    let mut needed = -7i32;
    assert!(
        status_label(c"ZZ".as_ptr(), buf.as_mut_ptr(), 64, &mut needed) > 0,
        "a positive D17 status, and the vocabulary stayed in the module"
    );

    // The concat path carries non-ASCII too: a borrowed span plus a Korean suffix.
    let account_label = label(&m, b"mlx_account_label\0");
    assert_eq!(
        call(account_label, c"0881234567"),
        (0, 14, "088번 계좌".to_string()),
        "3 ASCII + 4 Korean characters is 13 bytes, plus the NUL"
    );

    drop(m);
}

/// **Acceptance G** — the Q12 probe is unchanged, and it is the only correct way to size this.
#[test]
fn the_probe_is_how_a_host_learns_the_byte_count() {
    let (_out, m) = build("probe");
    let status_label = label(&m, b"mlx_status_label\0");

    let mut needed = -7i32;
    let st = status_label(c"RV".as_ptr(), core::ptr::null_mut(), 0, &mut needed);
    assert_eq!(st, mlc::abi::ML_ST_INSUFFICIENT_BUFFER);
    assert_eq!(needed, 10, "3 Korean characters are 9 bytes plus the NUL");

    // A host that allocated by CHARACTER count would pass 4 here and be refused, which is the
    // slice's cost showing up as a loud failure rather than a silent truncation.
    let mut small = [0u8; 4];
    let mut n2 = -7i32;
    assert_eq!(
        status_label(c"RV".as_ptr(), small.as_mut_ptr(), 4, &mut n2),
        mlc::abi::ML_ST_INSUFFICIENT_BUFFER,
        "sizing by characters is a refusal, never a short answer"
    );
    assert!(small.iter().all(|&b| b == 0), "and nothing was written");

    drop(m);
}

/// **Acceptance B and I** — the artifacts stay ASCII, and the protection proxy does not move.
#[test]
fn the_artifacts_stay_ascii_and_the_export_surface_is_unchanged() {
    let out = common::TempOut::new("na_art");
    let arts =
        emit_artifacts(include_str!("../../../examples/claim.mls"), "claim", &out).expect("emit");

    for path in [&arts.header, &arts.delphi_unit] {
        let text = std::fs::read_to_string(path).expect("read an artifact");
        assert!(
            text.is_ascii(),
            "{} carries non-ASCII bytes — the escape in codegen is what keeps it out",
            path.display()
        );
        assert!(
            text.contains("UTF-8"),
            "{} must tell a host the module returns UTF-8",
            path.display()
        );
    }

    let mut exports = pe::read_exports(&arts.dll).expect("exports");
    exports.sort();
    assert_eq!(
        exports,
        vec![
            "ml_iface_hash_claim".to_string(),
            "ml_module_abi_version".to_string(),
            "mlx_account_label".to_string(),
            "mlx_status_label".to_string(),
        ],
        "a Korean literal is data, not an export"
    );

    // The import set is a comparison, never an absolute (STATUS section 7): the baseline is a
    // module with an ASCII literal, built the same way.
    let base_dir = common::TempOut::new("na_base");
    let base = emit_artifacts(
        "export fn f() -> string! { return \"ok\" }",
        "nabase",
        &base_dir,
    )
    .expect("baseline");
    let baseline = pe::read_imports(&base.dll).expect("baseline imports");
    assert!(!baseline.is_empty(), "a cdylib always imports CRT startup");
    assert_eq!(
        pe::read_imports(&arts.dll).expect("imports"),
        baseline,
        "UTF-8 bytes are still just bytes: nothing new is linked to carry them"
    );
}

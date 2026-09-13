//! string-input slice — acceptance A/B/B2/C (E2).
//!
//! Section 3-B2 is the one that earns the slice. `country == "KR"` lowers to a byte loop, and
//! the four cases that a wrong implementation passes anyway (`"kr"`, `""`, `"KRW"`, a buffer
//! with trailing garbage) are exactly the ones that separate byte equality from "close enough".
//!
//! Section 3-C measures the claim in SPEC section 2.3 — that the comparison does not call the
//! CRT — against the module's actual import table, not against the generated source.
#![cfg(windows)]

use core::ffi::c_char;

use ml_oracle::{pe, Module};
use mlc::emit::emit_artifacts;

fn build_named(tag: &str, name: &str, src: &str) -> (std::path::PathBuf, Module) {
    let out = std::env::temp_dir().join(format!("mlc_str_{tag}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&out);
    std::fs::create_dir_all(&out).unwrap();
    let arts = emit_artifacts(src, name, &out).unwrap_or_else(|e| panic!("emit {name}: {e}"));
    let m = Module::load(arts.dll.to_str().unwrap()).expect("load the dll");
    (out, m)
}

fn build(tag: &str) -> (std::path::PathBuf, Module) {
    build_named(tag, "vat", include_str!("../../../examples/vat.mls"))
}

#[test]
fn the_measured_rule_returns_the_right_rate() {
    // Section 3-B. A `c"KR"` literal is exactly the `const char*` a C host passes, and the
    // pointer type here is spelled `c_char` for the same reason: it is what the generated
    // header declares (DP-S1), so a change of shape shows up as a type error.
    let (out, m) = build("b");
    let vat: extern "C" fn(*const c_char) -> f64 =
        unsafe { std::mem::transmute(m.symbol(b"mlx_vat_rate\0").unwrap()) };

    assert_eq!(vat(c"KR".as_ptr()), 0.1);
    assert_eq!(vat(c"JP".as_ptr()), 0.08);
    assert_eq!(vat(c"DE".as_ptr()), 0.19);
    assert_eq!(vat(c"US".as_ptr()), 0.0, "an unknown code falls through");

    // The other two rules from SPEC section 1.1 — same shape, non-f64 returns.
    let issuer: extern "C" fn(*const c_char) -> i32 =
        unsafe { std::mem::transmute(m.symbol(b"mlx_issuer_of\0").unwrap()) };
    assert_eq!(issuer(c"4".as_ptr()), 1);
    assert_eq!(issuer(c"51".as_ptr()), 2);
    assert_eq!(issuer(c"5".as_ptr()), 0, "\"5\" is not \"51\"");

    // `!=` is the negated byte loop, so it needs its own value, not just its own compile.
    let is_export: extern "C" fn(*const c_char) -> bool =
        unsafe { std::mem::transmute(m.symbol(b"mlx_is_export_item\0").unwrap()) };
    assert!(!is_export(c"DOM".as_ptr()));
    assert!(is_export(c"EXP".as_ptr()));

    drop(m);
    let _ = std::fs::remove_dir_all(&out);
}

#[test]
fn equality_is_bytes_to_the_nul_and_nothing_looser() {
    // Section 3-B2. Each of these passes under some plausible-but-wrong implementation:
    // case-insensitive compare, a length-0 shortcut, a prefix compare, or a fixed-length one.
    let (out, m) = build("b2");
    let vat: extern "C" fn(*const c_char) -> f64 =
        unsafe { std::mem::transmute(m.symbol(b"mlx_vat_rate\0").unwrap()) };

    assert_eq!(vat(c"kr".as_ptr()), 0.0, "case matters");
    assert_eq!(vat(c"".as_ptr()), 0.0, "the empty string matches nothing");
    assert_eq!(vat(c"KRW".as_ptr()), 0.0, "a longer string is not a match");
    assert_eq!(vat(c"K".as_ptr()), 0.0, "a shorter prefix is not a match");

    // The NUL ends the string: a buffer whose first three bytes are `K`, `R`, 0 matches "KR"
    // no matter what follows. Reading past the NUL would make this 0.0.
    let padded = b"KR\0XXXXXXXXXXXXXXXX";
    assert_eq!(
        vat(padded.as_ptr().cast()),
        0.1,
        "trailing garbage is not read"
    );

    // ...and the mirror: a literal is NUL-terminated too, so a host buffer that merely STARTS
    // with the literal's bytes must not match. (`"KRW"` above covers the same edge from the
    // module's side; this one pins the host's.)
    assert_eq!(vat(c"KRX".as_ptr()), 0.0);

    drop(m);
    let _ = std::fs::remove_dir_all(&out);
}

#[test]
fn the_module_gains_no_export_and_no_import() {
    // Section 3-C. The export surface is the same shape as every other module's, and — the
    // claim that needed measuring — the byte loop adds NOTHING to the import table. If the
    // comparison had been lowered to `strcmp`, an `api-ms-win-crt-string` entry would appear
    // here, exactly as the CRT heap entry did when Q12 was measured.
    let (out, m) = build("c");
    drop(m);
    let dll = out.join("vat.dll");

    let mut names = pe::read_exports(&dll).expect("read exports");
    names.sort();
    assert_eq!(
        names,
        vec![
            "ml_iface_hash_vat".to_string(),
            "ml_module_abi_version".to_string(),
            "mlx_is_export_item".to_string(),
            "mlx_issuer_of".to_string(),
            "mlx_vat_rate".to_string(),
        ],
        "ml_streq is internal and must not be exported"
    );

    // "The import set is unchanged" is a comparison, not an absolute: every cdylib carries the
    // same DllMain scaffolding (CRT startup, `vcruntime140!memcpy`, a few kernel32 entries)
    // whether or not it touches a string. Measured here rather than assumed: the baseline is a
    // module with NO string in it, built the same way, and the two sets must be identical.
    let (base_out, base_m) = build_named(
        "c_base",
        "baseline",
        "export fn f(x: f64) -> f64 { return x * 2.0 }",
    );
    drop(base_m);
    let baseline = pe::read_imports(&base_out.join("baseline.dll")).expect("baseline imports");

    let imports = pe::read_imports(&dll).expect("read imports");
    println!("vat.dll imports = {imports:?}");
    assert_eq!(
        imports, baseline,
        "the byte loop must add no import; strcmp/memcmp would show up as a difference"
    );
    // Belt and braces: name the functions the SPEC section 2.3 actually forbids, so a future
    // change that shifted the BASELINE too would still be caught here.
    for banned in ["strcmp", "memcmp", "strncmp", "malloc", "free"] {
        assert!(
            !imports.iter().any(|i| i.ends_with(&format!("!{banned}"))),
            "{banned} must not be imported: {imports:?}"
        );
    }

    let size = std::fs::metadata(&dll).unwrap().len();
    println!("vat.dll = {size} B, {} imports", imports.len());
    assert!(size < 60_000, "still a small stripped module: {size}");

    let _ = std::fs::remove_dir_all(&out);
    let _ = std::fs::remove_dir_all(&base_out);
}

/// **`byte_len(s)` measured on a real module** — acceptance A and G of `SPEC-string-length`.
///
/// A is the value: NUL EXCLUDED, so `""` is 0 and `"UPSN"` is 4. That is deliberately a
/// different number from Q12's `*ml_needed`, which is NUL-INCLUSIVE and answers 1 for the
/// empty string — one is a length, the other an allocation size (SPEC §2.1).
///
/// G is the cost, measured rather than asserted. Bytes are not characters (DP-L2): the module
/// never interprets an encoding (DP-S2), so a UTF-8 host sending two Korean characters gets
/// **6**, not 2. The SPEC says this is the slice's only new risk; a test is what stops that
/// sentence from being a guess.
#[test]
fn byte_len_counts_bytes_up_to_the_nul_and_not_characters() {
    let (out, m) = build_named(
        "blen",
        "blen",
        "export fn n(s: string) -> i32 { return byte_len(s) }",
    );
    let n: extern "C" fn(*const c_char) -> i32 =
        unsafe { std::mem::transmute(m.symbol(b"mlx_n\0").unwrap()) };

    // A — the terminator is not counted. `c"…"` literals, the same idiom the rest of this
    // file uses, because that is exactly the `const char*` a C host passes.
    assert_eq!(n(c"".as_ptr()), 0, "the empty string is 0, not 1");
    assert_eq!(n(c"UPSN".as_ptr()), 4, "\"UPSN\" is 4");
    assert_eq!(n(c"x".as_ptr()), 1);

    // …and it stops AT the NUL rather than running to the end of the buffer. A raw byte
    // slice here on purpose: a `c"…"` literal cannot hold an interior NUL, which is the
    // whole point of this case.
    let embedded: &[u8] = b"ab\0cd\0";
    assert_eq!(
        n(embedded.as_ptr().cast::<c_char>()),
        2,
        "the walk must stop at the first NUL, not scan the whole allocation"
    );

    // G — the cost, in the same run so it cannot drift from the value above. Two Korean
    // characters in UTF-8 are six bytes; an author who reads `byte_len` as "characters"
    // gets 6 where they meant 2, and the module cannot tell the difference because DP-S2
    // leaves it holding opaque bytes.
    let utf8 = c"한국";
    assert_eq!(utf8.to_bytes().len(), 6, "control: 2 chars = 6 bytes");
    assert_eq!(
        n(utf8.as_ptr()),
        6,
        "byte_len answers BYTES — this is DP-L2's cost, measured rather than claimed"
    );

    drop(m);
    let _ = std::fs::remove_dir_all(out);
}

/// **Acceptance D of `SPEC-string-length`: `byte_len` adds no import and no export.**
///
/// The SPEC's reason is that the count is a byte loop rather than a `strlen` call — the same
/// argument the `==` slice made, and the same one the protection proxy (D04/D05) turns into a
/// number. A CRT entry appearing here would mean the emitted helper had been replaced by
/// something that links against the runtime.
///
/// **This was written AFTER the SPEC had already claimed acceptance A–H were all closed.**
/// D had in fact only been reasoned about; the number came from a one-off `dumpbin` run, and
/// review pointed out that a one-off measurement is not a guard — a later helper, or an LLVM
/// idiom that recognises the loop and calls `strlen`, would grow the import set with nothing
/// in CI to notice.
#[test]
fn byte_len_adds_no_import_over_a_scalar_baseline() {
    let (base_dir, base_m) = build_named(
        "blbase",
        "blbase",
        "export fn f(x: f64) -> f64 { return x * 2.0 }",
    );
    let base_dll = base_dir.join("blbase.dll");
    let baseline = pe::read_imports(&base_dll).expect("baseline imports");
    assert!(
        !baseline.is_empty(),
        "a cdylib always imports CRT startup — an empty read means the reader failed, and \
         two empty sets would compare equal while measuring nothing"
    );

    let (out, m) = build_named(
        "blimp",
        "blimp",
        "export fn n(s: string) -> i32 { return byte_len(s) }",
    );
    let dll = out.join("blimp.dll");
    let imports = pe::read_imports(&dll).expect("imports");
    println!("byte_len imports = {imports:?}");
    assert_eq!(
        imports, baseline,
        "byte_len must not add an import — it is a byte loop, not a `strlen` call"
    );

    // …and the protection proxy is untouched: still exactly the three D18 exports.
    let mut exports = pe::read_exports(&dll).expect("exports");
    exports.sort();
    assert_eq!(
        exports,
        vec![
            "ml_iface_hash_blimp".to_string(),
            "ml_module_abi_version".to_string(),
            "mlx_n".to_string(),
        ],
        "byte_len is a builtin, not an export"
    );

    drop(m);
    drop(base_m);
    let _ = std::fs::remove_dir_all(out);
    let _ = std::fs::remove_dir_all(base_dir);
}

//! string-slice slice — `SPEC-string-slice` acceptance A/B/C/D/F/G/H/I/J (E2).
//!
//! The test that earns the slice is [`a_span_is_not_the_suffix`]. Every writer this compiler
//! emitted before `byte_slice` stops at the SOURCE NUL — `ml_slen` counts to it, `ml_wstr`
//! copies to it — so a span lowered to a borrowed pointer `s + from` would ignore `to` and
//! hand back everything to the end of the string, with status 0 and no warning. That is the
//! shape of the `[bool]` width defect (`SPEC-array-return` acceptance E), and the only thing
//! separating it from a correct implementation is a source LONGER than the span.
#![cfg(windows)]

use core::ffi::c_char;

use ml_oracle::{pe, Module};
use mlc::emit::emit_artifacts;

mod common;

/// `int32_t mlx_f(const char* s, char* ml_buf, int32_t ml_cap, int32_t* ml_needed)`
type SliceFn = extern "C" fn(*const c_char, *mut u8, i32, *mut i32) -> i32;

const SRC: &str = "\
export fn head(s: string) -> string! { return byte_slice(s, 0, 3) }
export fn tail(s: string) -> string! { return byte_slice(s, 2, byte_len(s)) }
export fn empty(s: string) -> string! { return byte_slice(s, 2, 2) }
export fn joined(s: string) -> string! { return byte_slice(s, 0, 3) + \"-\" }
";

/// `common::TempOut` rather than a hand-rolled temp path, and that is not style.
///
/// The first version of this file copied the older `string_input.rs` helper — create, then
/// `let _ = remove_dir_all(&out)` at the END of the test. It leaked **eight trees** in this
/// session, all of them from the runs where the writer was deliberately broken: a panicking
/// test never reaches its last line. That is the recurrence `TempOut` was written to end
/// (STATUS §5-5.7, §9-36), reproduced by copying the pattern it replaced.
fn build(tag: &str, name: &str, src: &str) -> (common::TempOut, Module) {
    let out = common::TempOut::new(&format!("slice_{tag}"));
    let arts = emit_artifacts(src, name, &out).unwrap_or_else(|e| panic!("emit {name}: {e}"));
    let m = Module::load(arts.dll.to_str().unwrap()).expect("load the dll");
    (out, m)
}

fn spanner(m: &Module, sym: &[u8]) -> SliceFn {
    unsafe { std::mem::transmute(m.symbol(sym).unwrap()) }
}

/// A buffer the module must not scribble on outside `cap` bytes. 0xAA because it is neither
/// NUL nor ASCII: a stray terminator and a stray character are both visible.
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
    fn intact_from(&self, from: usize) -> bool {
        self.bytes[from..].iter().all(|&b| b == 0xAA)
    }
    fn is_pristine(&self) -> bool {
        self.intact_from(0)
    }
}

/// **Acceptance A, B and F** — the span is the bytes between the offsets, half-open.
#[test]
fn a_span_is_the_bytes_between_the_offsets() {
    // `_out` and not `_`: the underscore PREFIX binds the value and keeps the directory alive
    // to the end of the scope, while a bare `_` would drop it here and delete the tree out
    // from under the loaded module. Named this way because nothing reads it — cleanup is the
    // Drop, which is the whole point of `TempOut`.
    let (_out, m) = build("val", "span", SRC);

    // A — a literal span of a borrowed parameter.
    let head = spanner(&m, b"mlx_head\0");
    let mut buf = Canary::new();
    let mut needed = -7i32;
    assert_eq!(head(c"1234567890".as_ptr(), buf.ptr(), 64, &mut needed), 0);
    assert_eq!(needed, 4, "three bytes plus the NUL (Q12's unit, DP-T4)");
    assert_eq!(&buf.bytes[..4], b"123\0");
    assert!(buf.intact_from(4), "nothing past `needed`: {:?}", buf.bytes);

    // B — `byte_len(s)` as the end offset, which is the composition DP-B2 was chosen for.
    let tail = spanner(&m, b"mlx_tail\0");
    let mut buf = Canary::new();
    let mut needed = -7i32;
    assert_eq!(tail(c"1234567890".as_ptr(), buf.ptr(), 64, &mut needed), 0);
    assert_eq!(needed, 9, "bytes 2..10 is eight bytes, plus the NUL");
    assert_eq!(&buf.bytes[..9], b"34567890\0");

    // F — an empty span is a value, not an error. `*needed` is 1: the NUL alone, which is
    // deliberately never 0 (#92), so a host cannot confuse "empty" with "not written".
    let empty = spanner(&m, b"mlx_empty\0");
    let mut buf = Canary::new();
    let mut needed = -7i32;
    assert_eq!(empty(c"1234567890".as_ptr(), buf.ptr(), 64, &mut needed), 0);
    assert_eq!(needed, 1);
    assert_eq!(buf.bytes[0], 0);
    assert!(buf.intact_from(1));

    drop(m);
}

/// **Acceptance C and J — the span is not the suffix, on BOTH paths.**
///
/// The source is longer than the span on purpose, and that is the acceptance criterion, not
/// an incidental choice. With `byte_slice(c"123", 0, 3)` a NUL-bounded writer returns exactly
/// the same three bytes, so the test would pass over a wrong implementation.
///
/// Two calls because there are two ways a span reaches the buffer — alone, and as a piece of
/// a concatenation. They share one emitter here, but the review that found this hole was
/// right that nothing in the SPEC forced them to, so both are measured.
#[test]
fn a_span_is_not_the_suffix() {
    let (_out, m) = build("suffix", "span", SRC);
    let long = c"1234567890";

    // Standalone.
    let head = spanner(&m, b"mlx_head\0");
    let mut buf = Canary::new();
    let mut needed = -7i32;
    assert_eq!(head(long.as_ptr(), buf.ptr(), 64, &mut needed), 0);
    assert_eq!(
        needed, 4,
        "a NUL-bounded writer would size this at 11 — the whole string plus its NUL"
    );
    assert_eq!(
        &buf.bytes[..4],
        b"123\0",
        "the span stops at `to`, not at the source's NUL: {:?}",
        &buf.bytes[..12]
    );

    // As a piece of a concatenation.
    let joined = spanner(&m, b"mlx_joined\0");
    let mut buf = Canary::new();
    let mut needed = -7i32;
    assert_eq!(joined(long.as_ptr(), buf.ptr(), 64, &mut needed), 0);
    assert_eq!(needed, 5, "\"123-\" plus the NUL");
    assert_eq!(
        &buf.bytes[..5],
        b"123-\0",
        "the concat path must bound the span by LENGTH too: {:?}",
        &buf.bytes[..12]
    );

    drop(m);
}

/// **Acceptance D and the runtime half of E** — out of range fails, and writes nothing.
///
/// `SPEC-string-slice` §2.4 refuses rather than clamping, because a clamp is the silent wrong
/// answer this repository has closed four times. Q12 then requires that a failed call leave
/// the caller's buffer alone, and D17 that it leave `*ml_needed` alone — so the range check
/// has to run before either is touched, which is what the canary measures.
#[test]
fn an_out_of_range_span_writes_nothing() {
    let (_out, m) = build("range", "span", SRC);
    let head = spanner(&m, b"mlx_head\0");

    // `to` is 3 and the string has two bytes: out of range, not a short answer.
    let mut buf = Canary::new();
    let mut needed = -7i32;
    let status = head(c"12".as_ptr(), buf.ptr(), 64, &mut needed);
    assert_eq!(
        status,
        mlc::abi::ML_ST_INDEX_OUT_OF_RANGE,
        "a span past the end is a failure, not a clamp to what fits"
    );
    assert!(
        buf.is_pristine(),
        "a failed call must not write one byte: {:?}",
        &buf.bytes[..8]
    );
    assert_eq!(
        needed, -7,
        "D17: a failed call leaves `*ml_needed` untouched, so the check precedes it"
    );

    // The empty string is the sharpest version of the same case.
    let mut buf = Canary::new();
    assert_eq!(
        head(c"".as_ptr(), buf.ptr(), 64, &mut needed),
        mlc::abi::ML_ST_INDEX_OUT_OF_RANGE
    );
    assert!(buf.is_pristine());

    // …and the runtime half of E: `tail` computes `2..byte_len(s)`, which runs backwards
    // when the string is shorter than two bytes. The literal form of this is a compile
    // error; this one cannot be, because the end is a value.
    let tail = spanner(&m, b"mlx_tail\0");
    let mut buf = Canary::new();
    assert_eq!(
        tail(c"1".as_ptr(), buf.ptr(), 64, &mut needed),
        mlc::abi::ML_ST_INDEX_OUT_OF_RANGE,
        "`byte_slice(s, 2, 1)` is backwards and must fail rather than wrap"
    );
    assert!(buf.is_pristine());

    drop(m);
}

/// **Acceptance G** — the Q12 probe works for a span exactly as it does for any other return.
#[test]
fn the_probe_protocol_works_for_a_span() {
    let (_out, m) = build("probe", "span", SRC);
    let tail = spanner(&m, b"mlx_tail\0");
    let s = c"1234567890";

    // DP-T7: `ml_buf` may be NULL iff `ml_cap == 0`. The answer is the exact size to allocate.
    let mut needed = -7i32;
    let status = tail(s.as_ptr(), core::ptr::null_mut(), 0, &mut needed);
    assert_eq!(status, mlc::abi::ML_ST_INSUFFICIENT_BUFFER);
    assert_eq!(needed, 9);

    // The retry with exactly that size must succeed and fill the buffer to the last byte.
    let mut buf = Canary::new();
    let mut needed2 = -7i32;
    assert_eq!(
        tail(s.as_ptr(), buf.ptr(), needed, &mut needed2),
        0,
        "`cap = *needed` must be enough, or the probe idiom does not converge"
    );
    assert_eq!(needed2, 9);
    assert_eq!(&buf.bytes[..9], b"34567890\0");
    assert!(buf.intact_from(9), "an exact fit writes exactly `needed`");

    // One byte short is a failure, and still writes nothing.
    let mut buf = Canary::new();
    assert_eq!(
        tail(s.as_ptr(), buf.ptr(), needed - 1, &mut needed2),
        mlc::abi::ML_ST_INSUFFICIENT_BUFFER
    );
    assert!(
        buf.is_pristine(),
        "truncation is a failure, not a short write"
    );

    drop(m);
}

/// **Acceptance H — bytes are not characters, and a span can cut one in half.**
///
/// `byte_len` gives a UTF-8 host a number it did not expect (6 for two characters). A span
/// does something sharper with the same opacity: it hands back a fragment that is not a
/// character at all. DP-S2 leaves the module holding bytes, so it cannot know — which is why
/// the builtin is spelled `byte_` and why this cost is measured rather than asserted.
#[test]
fn a_span_cuts_bytes_not_characters() {
    let (_out, m) = build("utf8", "span", SRC);
    let head = spanner(&m, b"mlx_head\0");

    let korean = c"한국";
    assert_eq!(korean.to_bytes().len(), 6, "control: 2 chars = 6 bytes");

    // Three bytes IS the first character here — the pleasant case, and pure luck.
    let mut buf = Canary::new();
    let mut needed = -7i32;
    assert_eq!(head(korean.as_ptr(), buf.ptr(), 64, &mut needed), 0);
    assert_eq!(needed, 4);
    assert_eq!(
        &buf.bytes[..3],
        &korean.to_bytes()[..3],
        "the first three bytes, whatever they mean"
    );
    assert_eq!(
        core::str::from_utf8(&buf.bytes[..3]),
        Ok("한"),
        "three bytes happen to be one character here"
    );

    // …and two bytes is not. The module reports success, because by DP-S2 there is nothing
    // for it to object to: it copied the bytes it was asked for.
    let two = "export fn f(s: string) -> string! { return byte_slice(s, 0, 2) }";
    let (_out2, m2) = build("utf8b", "half", two);
    let f = spanner(&m2, b"mlx_f\0");
    let mut buf = Canary::new();
    let mut needed = -7i32;
    assert_eq!(
        f(korean.as_ptr(), buf.ptr(), 64, &mut needed),
        0,
        "status 0"
    );
    assert_eq!(needed, 3);
    assert!(
        core::str::from_utf8(&buf.bytes[..2]).is_err(),
        "half a character is not UTF-8 — this is DP-B1's cost, measured"
    );

    drop(m);
    drop(m2);
}

/// **The example itself**, loaded and called — `examples/account.mls`.
///
/// The tests above build their sources inline, which keeps each case next to its reason. This
/// one exists because the example is what a reader opens and what the C host gates, and
/// `golden.rs` requires every exported example function to have a named oracle call site: a
/// function nobody calls is a shape whose wrong adapter would ship silently.
#[test]
fn the_account_example_cuts_the_string_the_host_used_to_cut() {
    let (_out, m) = build(
        "example",
        "account",
        include_str!("../../../examples/account.mls"),
    );
    let acct = c"0881234567";

    let bank_code: SliceFn = unsafe { std::mem::transmute(m.symbol(b"mlx_bank_code\0").unwrap()) };
    let mut buf = Canary::new();
    let mut needed = -7i32;
    assert_eq!(bank_code(acct.as_ptr(), buf.ptr(), 64, &mut needed), 0);
    assert_eq!(&buf.bytes[..4], b"088\0", "the audit's rule, in the module");
    assert_eq!(needed, 4);

    let account_body: SliceFn =
        unsafe { std::mem::transmute(m.symbol(b"mlx_account_body\0").unwrap()) };
    let mut buf = Canary::new();
    assert_eq!(account_body(acct.as_ptr(), buf.ptr(), 64, &mut needed), 0);
    assert_eq!(&buf.bytes[..8], b"1234567\0");

    // Two spans and a literal in one result, each bounded by its own length.
    let masked: SliceFn = unsafe { std::mem::transmute(m.symbol(b"mlx_masked\0").unwrap()) };
    let mut buf = Canary::new();
    assert_eq!(masked(acct.as_ptr(), buf.ptr(), 64, &mut needed), 0);
    assert_eq!(&buf.bytes[..14], b"088-****-4567\0");
    assert_eq!(needed, 14);

    // …and the rule the §7 sample could not write at all: which bank owns this account.
    // Until spans could be compared, the module could only hand the first three bytes back
    // and let the host decide.
    let is_kookmin: extern "C" fn(*const c_char, *mut bool) -> i32 =
        unsafe { std::mem::transmute(m.symbol(b"mlx_is_kookmin\0").unwrap()) };
    let mut yes = false;
    assert_eq!(is_kookmin(c"0041234567".as_ptr(), &mut yes), 0);
    assert!(yes, "a source longer than the span must still match");
    assert_eq!(is_kookmin(acct.as_ptr(), &mut yes), 0);
    assert!(!yes, "\"088…\" is not \"004\"");

    // The module guards the length itself, so a short account is a DOMAIN failure — a
    // positive D17 code, not the span's own -2. That distinction is why the predicate is
    // fallible rather than answering `false`.
    let mut yes = true;
    assert!(
        is_kookmin(c"00".as_ptr(), &mut yes) > 0,
        "too short to have a bank code is the module's own error"
    );

    let mut buf = Canary::new();
    needed = -7;
    let st = bank_code(c"08".as_ptr(), buf.ptr(), 64, &mut needed);
    assert!(
        st > 0,
        "a guarded module reports its own error, not ML_ST_INDEX_OUT_OF_RANGE: {st}"
    );
    assert!(buf.is_pristine());
    assert_eq!(needed, -7);

    drop(m);
}

/// **Acceptance I** — the span adds no import and no export over a module that only concatenates.
///
/// The baseline is a concatenating module rather than a scalar one, because a span already
/// pulls in the concat helper set: what is measured here is that `ml_sublen`/`ml_wsub` add
/// nothing to the import table on top of that. `strlen`, `memcpy` and `malloc` would all be
/// visible if the copy had been lowered to a CRT call.
///
/// Pinned as a test rather than left as a `dumpbin` run, for the reason `byte_len` had to
/// learn one slice ago: a one-off measurement is not a guard.
#[test]
fn a_span_adds_no_import_over_a_concatenating_baseline() {
    let (base_dir, base_m) = build(
        "ibase",
        "sbase",
        "export fn f(s: string) -> string! { return s + \"-\" }",
    );
    let baseline = pe::read_imports(&base_dir.join("sbase.dll")).expect("baseline imports");
    assert!(
        !baseline.is_empty(),
        "a cdylib always imports CRT startup — an empty read means the reader failed, and \
         two empty sets would compare equal while measuring nothing"
    );

    let (out, m) = build(
        "imp",
        "simp",
        "export fn f(s: string) -> string! { return byte_slice(s, 0, 3) }",
    );
    let dll = out.join("simp.dll");
    let imports = pe::read_imports(&dll).expect("imports");
    println!("byte_slice imports = {imports:?}");
    assert_eq!(
        imports, baseline,
        "the span helpers are byte loops, not CRT calls"
    );
    // Belt and braces on top of the set comparison, and the list is short on purpose:
    // `memcpy` and `memset` are in the BASELINE (`vcruntime140`, part of every cdylib's
    // DllMain scaffolding), so banning them by name would fail on a correct module. Measured,
    // not guessed — the first version of this list had `memcpy` in it and went red.
    for banned in ["strlen", "strncpy", "strcpy", "malloc", "free"] {
        assert!(
            !imports.iter().any(|i| i.ends_with(&format!("!{banned}"))),
            "{banned} must not be imported: {imports:?}"
        );
    }

    let mut exports = pe::read_exports(&dll).expect("exports");
    exports.sort();
    assert_eq!(
        exports,
        vec![
            "ml_iface_hash_simp".to_string(),
            "ml_module_abi_version".to_string(),
            "mlx_f".to_string(),
        ],
        "byte_slice is a builtin, not an export"
    );

    drop(m);
    drop(base_m);
}

/// `int32_t mlx_f(const char* s, bool* out)` — a fallible predicate (D17).
type PredFn = extern "C" fn(*const c_char, *mut bool) -> i32;

const CMP: &str = "\
export fn is_ab(c: string) -> bool! { return byte_slice(c, 0, 2) == \"AB\" }
export fn is_ab_rhs(c: string) -> bool! { return \"AB\" == byte_slice(c, 0, 2) }
export fn not_ab(c: string) -> bool! { return byte_slice(c, 0, 2) != \"AB\" }
export fn is_abc(c: string) -> bool! { return byte_slice(c, 0, 2) == \"ABC\" }
export fn three_is_ab(c: string) -> bool! { return byte_slice(c, 0, 3) == \"AB\" }
export fn empty_is_empty(c: string) -> bool! { return byte_slice(c, 2, 2) == \"\" }
export fn matches(c: string, want: string) -> bool! { return byte_slice(c, 0, 2) == want }
";

fn pred(m: &Module, sym: &[u8]) -> PredFn {
    unsafe { std::mem::transmute(m.symbol(sym).unwrap()) }
}

fn ask(f: PredFn, s: &core::ffi::CStr) -> (i32, bool) {
    let mut out = false;
    let st = f(s.as_ptr(), &mut out);
    (st, out)
}

/// **`SPEC-string-slice-compare` acceptance A, B, C, G and H — measured on a loaded module.**
///
/// C is the one that earns the slice, and the source is longer than the span on purpose:
/// `ml_streq` walks to a NUL, so reusing it would compare all of `"ABZZZZZZ"` against `"AB"`
/// and answer **false** where the truth is **true**. The wrong answer points the opposite way
/// from the suffix bug the span slice had, which is why it needs its own case.
#[test]
fn a_span_compares_by_length_not_by_nul() {
    let (_out, m) = build("cmp", "cmp", CMP);

    // A — the rule that motivated the slice.
    let is_ab = pred(&m, b"mlx_is_ab\0");
    assert_eq!(ask(is_ab, c"AB123456"), (0, true));
    assert_eq!(ask(is_ab, c"AX123456"), (0, false));

    // C — a source LONGER than the span. This is the acceptance criterion, not a detail.
    assert_eq!(
        ask(is_ab, c"ABZZZZZZ"),
        (0, true),
        "a NUL-bounded compare would answer false here"
    );

    // B — a length mismatch is not equality, in BOTH directions.
    assert_eq!(
        ask(pred(&m, b"mlx_is_abc\0"), c"ABC12345"),
        (0, false),
        "a 2-byte span is not equal to a 3-byte literal"
    );
    assert_eq!(
        ask(pred(&m, b"mlx_three_is_ab\0"), c"ABC12345"),
        (0, false),
        "…nor a 3-byte span to a 2-byte literal"
    );

    // G — the right-hand spelling and the negated form.
    assert_eq!(ask(pred(&m, b"mlx_is_ab_rhs\0"), c"AB123456"), (0, true));
    assert_eq!(ask(pred(&m, b"mlx_not_ab\0"), c"AB123456"), (0, false));
    assert_eq!(ask(pred(&m, b"mlx_not_ab\0"), c"AX123456"), (0, true));

    // H — an empty span equals the empty literal.
    assert_eq!(
        ask(pred(&m, b"mlx_empty_is_empty\0"), c"AB123456"),
        (0, true)
    );

    drop(m);
}

/// **Acceptance D and I** — a borrowed parameter on the other side, and out of range is `-2`.
#[test]
fn an_out_of_range_span_comparison_is_a_status_not_false() {
    let (_out, m) = build("cmprange", "cmp", CMP);

    // I — compared against a `string` PARAMETER rather than a literal.
    let matches: extern "C" fn(*const c_char, *const c_char, *mut bool) -> i32 =
        unsafe { std::mem::transmute(m.symbol(b"mlx_matches\0").unwrap()) };
    let mut out = false;
    assert_eq!(matches(c"AB1234".as_ptr(), c"AB".as_ptr(), &mut out), 0);
    assert!(out);
    assert_eq!(matches(c"AB1234".as_ptr(), c"XY".as_ptr(), &mut out), 0);
    assert!(!out);

    // D — the string is shorter than the span. NOT `false`: the host has to be able to tell
    // "the range was wrong" from "the bytes differ", which is the whole of DP-C2.
    let is_ab = pred(&m, b"mlx_is_ab\0");
    let mut out = true;
    let st = is_ab(c"A".as_ptr(), &mut out);
    assert_eq!(
        st,
        mlc::abi::ML_ST_INDEX_OUT_OF_RANGE,
        "an out-of-range span is a status, not a `false` answer"
    );
    let st = is_ab(c"".as_ptr(), &mut out);
    assert_eq!(st, mlc::abi::ML_ST_INDEX_OUT_OF_RANGE);

    drop(m);
}

/// **Acceptance J** — comparing a span adds no import and no export.
#[test]
fn a_span_comparison_adds_no_import_over_a_comparing_baseline() {
    let (base_dir, base_m) = build(
        "cbase",
        "cbase",
        "export fn f(s: string) -> bool { return s == \"AB\" }",
    );
    let baseline = pe::read_imports(&base_dir.join("cbase.dll")).expect("baseline imports");
    assert!(!baseline.is_empty(), "a cdylib always imports CRT startup");

    let (out, m) = build(
        "cimp",
        "cimp",
        "export fn f(s: string) -> bool! { return byte_slice(s, 0, 2) == \"AB\" }",
    );
    let dll = out.join("cimp.dll");
    let imports = pe::read_imports(&dll).expect("imports");
    assert_eq!(
        imports, baseline,
        "`ml_subeq` is a byte loop, not a CRT call"
    );
    for banned in ["strcmp", "strncmp", "memcmp", "strlen"] {
        assert!(
            !imports.iter().any(|i| i.ends_with(&format!("!{banned}"))),
            "{banned} must not be imported: {imports:?}"
        );
    }

    let mut exports = pe::read_exports(&dll).expect("exports");
    exports.sort();
    assert_eq!(
        exports,
        vec![
            "ml_iface_hash_cimp".to_string(),
            "ml_module_abi_version".to_string(),
            "mlx_f".to_string(),
        ],
        "the comparison is a builtin, not an export"
    );

    drop(m);
    drop(base_m);
}

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
            "ml_iface_hash_receipt".to_string(),
            "ml_module_abi_version".to_string(),
            "mlx_full_name".to_string(),
            "mlx_label".to_string(),
            "mlx_receipt_line".to_string(),
            "mlx_summary".to_string(),
        ],
        "four exports plus the two reserved symbols"
    );
}

/// The `cap` hard stop inside `ml_wstr`, exercised by the call it exists for.
///
/// **This defence has only ever been argued.** codegen says of that bound: *"It exists for the
/// host that passes a string ALIASING its own output buffer: nothing in the C ABI forbids
/// that, and without the bound this loop would copy its own output forward and run off the end
/// of the caller's memory."* That is a claim about memory safety **in the host's process**, and
/// nothing in this repository had ever made the call. `STATUS.md` §9-55 is about exactly this
/// gap in a different place: a defence pinned by reading the emitted source
/// (`if off + i >= cap - 1`) rather than by running it.
///
/// So this makes the call. `first` points INTO the output buffer, which is legal C — the ABI
/// says the module may not retain the pointer (D16), not that it may not overlap. Pass 1 sizes
/// the result from the original bytes; pass 2 then overwrites those same bytes as it copies,
/// so the copy feeds on its own output. The bound is the only thing standing between that and
/// a write past `ml_cap`.
///
/// What is asserted is the property, not the garbage: **every byte at or past `cap` is
/// untouched**. The bytes inside `cap` are deliberately not pinned — they are whatever a
/// self-feeding copy produces, and freezing them would turn an implementation detail into a
/// contract.
#[test]
fn a_host_that_aliases_its_own_output_buffer_cannot_be_written_past() {
    let h = build("alias");
    let full_name: NameFn = sym(&h.m, b"mlx_full_name\0");

    // One allocation. `CAP` is what the module is told it may use; everything after it is
    // canary that must still be 0xAA when the call returns.
    const CAP: i32 = 24;
    let mut arena = [0xAAu8; 96];
    arena[..6].copy_from_slice(b"Aroha\0");

    let base = arena.as_mut_ptr();
    let mut needed = -7i32;
    // Both pointers derive from the same allocation: `first` is the string at offset 0, and
    // offset 0 is also where the result goes.
    let status = full_name(
        base as *const c_char,
        c"Kim".as_ptr(),
        base,
        CAP,
        &mut needed,
    );

    // The answer is allowed to be anything — including a success with nonsense in it. What is
    // NOT allowed is a byte past `cap`.
    println!(
        "aliased call: status={status} needed={needed} text={:?}",
        text(&arena[..CAP as usize])
    );
    assert!(
        arena[CAP as usize..].iter().all(|b| *b == 0xAA),
        "the module wrote past ml_cap under aliasing: {:?}",
        &arena[CAP as usize..CAP as usize + 16]
    );

    // And the same call with NO overlap answers correctly — so the bound above is not simply
    // refusing every call. Without this line the test would pass against a module that did
    // nothing at all.
    let mut buf = [0xAAu8; 96];
    let mut needed = -7i32;
    let status = full_name(
        c"Aroha".as_ptr(),
        c"Kim".as_ptr(),
        buf.as_mut_ptr(),
        CAP,
        &mut needed,
    );
    assert_eq!(status, 0);
    assert_eq!(text(&buf), "Kim Aroha");
    assert_eq!(needed, 10);
}

/// A negative `ml_cap` on the CONCAT path — all four piece kinds at once.
///
/// `string_return.rs` already passes `-1` and `i32::MIN`, but to `carrier_name`, which returns
/// a borrowed literal and therefore goes through `ml_strout`. `emit_concat_return` is a
/// different emitter with its own capacity test and three writers of its own, and nothing had
/// called it with a negative capacity. Scope of a claim again (#211): "a negative cap is
/// refused" was measured for one of the two string paths.
///
/// One function carries every `PieceKind` the emitter knows — bytes, digits, a span and a
/// decimal. That is for the SUCCEEDING call at the end, which drives all four writers; it does
/// **not** make the refusal stronger. Review named why, and it is worth keeping: the guard is a
/// single line ahead of every writer, so the first piece alone already decides the refusal, and
/// four kinds prove nothing about where the line sits.
///
/// For the same reason, only `status == -1` measures the guard on the refusing calls.
/// `*ml_needed` is written BEFORE it, so asserting `needed == 19` there would survive deleting
/// the line — it is asserted as a Q12 property (the host learns the size from a failed call),
/// not as evidence about the guard.
///
/// **And the guard turns out to protect more than the capacity.** Removing
/// `if ml_cap < __n { return -1; }` and running this test does not produce a wrong answer: the
/// process dies with `STATUS_ACCESS_VIOLATION` (measured). The probe call below passes
/// `ml_buf = NULL` with `ml_cap = 0`, which DP-T7 explicitly permits — and that one line is
/// the whole reason it is safe, because it returns before any writer dereferences the
/// pointer. The capacity test and the NULL-probe contract are the same line of code, which
/// neither `SPEC-string-return` nor the emitter's comments say anywhere.
#[test]
fn a_negative_capacity_is_refused_on_the_concat_path_too() {
    let dir = common::TempOut::new("concat_negcap");
    let arts = emit_artifacts(
        "export fn all(s: string, x: f64, n: i32) -> string! {\n\
         \x20 return s + \" \" + (n as string) + \" \" + byte_slice(s, 0, 2) + \" \" + fixed(x, 2)\n\
         }",
        "pieces",
        &dir,
    )
    .expect("emit pieces");
    let m = Module::load(arts.dll.to_str().unwrap()).expect("load pieces.dll");
    let all: extern "C" fn(*const c_char, f64, i32, *mut u8, i32, *mut i32) -> i32 =
        unsafe { std::mem::transmute(m.symbol(b"mlx_all\0").unwrap()) };

    // The true size first, from the probe, so the refusals below can be checked against it.
    let mut needed = -7i32;
    assert_eq!(
        all(
            c"ABCDEF".as_ptr(),
            12.5,
            42,
            core::ptr::null_mut(),
            0,
            &mut needed
        ),
        -1
    );
    assert_eq!(
        needed, 19,
        "\"ABCDEF 42 AB 12.50\" is 18 bytes, plus the NUL"
    );

    // Every negative capacity a host can pass, including the one where `cap - 1` would
    // overflow if the writers were ever reached with it.
    for cap in [-1, i32::MIN, i32::MIN + 1, -12345] {
        let mut buf = [0xAAu8; 64];
        let mut needed = -7i32;
        let status = all(
            c"ABCDEF".as_ptr(),
            12.5,
            42,
            buf.as_mut_ptr(),
            cap,
            &mut needed,
        );
        assert_eq!(status, -1, "cap={cap} must be refused");
        assert_eq!(needed, 19, "cap={cap}: the host still learns the size");
        assert!(
            buf.iter().all(|b| *b == 0xAA),
            "cap={cap}: a refused call writes nothing: {:?}",
            &buf[..16]
        );
    }

    // …and the fitting call still answers, so none of the above is a module that refuses all.
    let mut buf = [0xAAu8; 64];
    let mut needed = -7i32;
    assert_eq!(
        all(
            c"ABCDEF".as_ptr(),
            12.5,
            42,
            buf.as_mut_ptr(),
            64,
            &mut needed
        ),
        0
    );
    assert_eq!(needed, 19);
    assert_eq!(text(&buf), "ABCDEF 42 AB 12.50");

    drop(m);
    drop(dir);
}

/// **The `cap` bound has two writers that do not carry it** — audit finding, measured.
///
/// §9-56 measured that under aliasing `ml_wstr` runs to `cap - 1` and stops there, and #245
/// showed the SPAN writer after it stops too because it carries the same bound. The audit
/// asked the obvious next question: what about the two writers that do NOT take `cap` at
/// all — `ml_wint` (digits) and `ml_wfix` (decimals)? Their comment says the destination is
/// the only address in play and `ml_cap >= __n` was already checked. Both are true, and both
/// miss the point: they start at the `__o` the previous writer handed them, and under
/// aliasing that is `cap - 1`.
///
/// `receipt_line(item, qty, price) = item + " x " + qty + " = " + total` puts a digits piece
/// after a string piece. Alias `item` ahead of the output and the digits land past `cap`.
#[test]
fn a_digits_piece_after_an_aliased_string_piece_writes_past_cap() {
    let h = build("alias_digits");
    let line: LineFn = sym(&h.m, b"mlx_receipt_line\0");

    // The result ("WIDGET x 3 = 45000", 19 bytes) must FIT, or the capacity check refuses
    // the call before any writer runs — the first draft used 16 and measured exactly that
    // refusal, which is the guard doing its job and not this path. 24 fits; the string piece
    // then eats its own output up to `cap - 1`, and the digits pieces inherit that offset.
    const CAP: i32 = 24;
    const OUT_AT: usize = 2;
    let mut arena = [0xAAu8; 96];
    arena[..7].copy_from_slice(b"WIDGET\0");

    let base = arena.as_mut_ptr();
    let mut needed = -7i32;
    let status = line(
        base as *const c_char,
        3,
        15000,
        unsafe { base.add(OUT_AT) },
        CAP,
        &mut needed,
    );

    let last = OUT_AT + CAP as usize;
    println!(
        "aliased digits: status={status} needed={needed} in-cap={:?} past-cap={:?}",
        &arena[OUT_AT..last],
        &arena[last..last + 12]
    );
    assert!(
        arena[last..].iter().all(|b| *b == 0xAA),
        "a digits piece inherited an aliased offset and wrote past ml_cap: {:?}",
        &arena[last..last + 12]
    );

    // Non-overlapping still answers.
    let mut buf = [0xAAu8; 96];
    let mut needed = -7i32;
    assert_eq!(
        line(
            c"WIDGET".as_ptr(),
            3,
            15000,
            buf.as_mut_ptr(),
            64,
            &mut needed
        ),
        0
    );
    assert_eq!(text(&buf), "WIDGET x 3 = 45000");
}

/// The same question for the DECIMAL writer, which inherits `__o` exactly as the digits one
/// does. `ml_wfix` gained the same bound in the same change; this is what says so.
#[test]
fn a_decimal_piece_after_an_aliased_string_piece_also_stops_at_cap() {
    let dir = common::TempOut::new("concat_alias_fixed");
    let arts = emit_artifacts(
        "export fn tag(s: string, x: f64) -> string! { return s + \" = \" + fixed(x, 2) }",
        "aliasfx",
        &dir,
    )
    .expect("emit aliasfx");
    let m = Module::load(arts.dll.to_str().unwrap()).expect("load aliasfx.dll");
    let tag: extern "C" fn(*const c_char, f64, *mut u8, i32, *mut i32) -> i32 =
        unsafe { std::mem::transmute(m.symbol(b"mlx_tag\0").unwrap()) };

    // "WIDGET = 1234.50" is 16 bytes, 17 with the NUL; 24 fits so the call is not refused.
    const CAP: i32 = 24;
    const OUT_AT: usize = 2;
    let mut arena = [0xAAu8; 96];
    arena[..7].copy_from_slice(b"WIDGET\0");

    let base = arena.as_mut_ptr();
    let mut needed = -7i32;
    let status = tag(
        base as *const c_char,
        1234.5,
        unsafe { base.add(OUT_AT) },
        CAP,
        &mut needed,
    );

    let last = OUT_AT + CAP as usize;
    println!(
        "aliased decimal: status={status} needed={needed} past-cap={:?}",
        &arena[last..last + 12]
    );
    assert!(
        arena[last..].iter().all(|b| *b == 0xAA),
        "a decimal piece inherited an aliased offset and wrote past ml_cap: {:?}",
        &arena[last..last + 12]
    );

    let mut buf = [0xAAu8; 96];
    let mut needed = -7i32;
    assert_eq!(
        tag(
            c"WIDGET".as_ptr(),
            1234.5,
            buf.as_mut_ptr(),
            64,
            &mut needed
        ),
        0
    );
    assert_eq!(text(&buf), "WIDGET = 1234.50");
    assert_eq!(needed, 17);

    drop(m);
    drop(dir);
}

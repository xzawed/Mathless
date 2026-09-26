//! `examples/order.mls` — the state machine example `ROADMAP.md` Phase 3 names (E2).
//!
//! The first version wrote each state and event as a zero-argument internal `fn`, because the
//! language had no constants. That worked inside the module and failed at the boundary: the
//! codes were function bodies, so the header had no name for them and the fingerprint did not
//! cover them (§9-67.2). The example now declares them `export const` (SPEC-constants), and the
//! last two tests measure both sides of that change — the old form still passes the gate
//! silently, the new one moves the fingerprint.
#![cfg(windows)]

use ml_oracle::{pe, Module};
use mlc::emit::emit_artifacts;

mod common;

const SRC: &str = include_str!("../../../examples/order.mls");

// The same numbers the header now names ML_ORDER_CONST_*; a Rust test has no C header to read.
const CREATED: i32 = 1;
const PAID: i32 = 2;
const SHIPPED: i32 = 3;
const DELIVERED: i32 = 4;
const CANCELLED: i32 = 5;
const REFUNDED: i32 = 6;

const PAY: i32 = 1;
const SHIP: i32 = 2;
const DELIVER: i32 = 3;
const CANCEL: i32 = 4;
const REFUND: i32 = 5;

const E_BAD_TRANSITION: i32 = 1;
const E_UNKNOWN_STATE: i32 = 2;

type NextFn = extern "C" fn(i32, i32, *mut i32) -> i32;
type ReplayFn = extern "C" fn(i32, *const i32, i32, *mut i32) -> i32;
type TerminalFn = extern "C" fn(i32) -> bool;
type LabelFn = extern "C" fn(i32, *mut u8, i32, *mut i32) -> i32;

fn build(tag: &str) -> (common::TempOut, Module) {
    let out = common::TempOut::new(&format!("order_{tag}"));
    let arts = emit_artifacts(SRC, "order", &out).expect("emit order");
    let m = Module::load(arts.dll.to_str().unwrap()).expect("load order.dll");
    (out, m)
}

fn sym<T>(m: &Module, name: &[u8]) -> T {
    let p = m.symbol(name).unwrap();
    unsafe { std::mem::transmute_copy(&p) }
}

/// Status and value; the value stays at the sentinel when the call fails (D17).
fn next(f: NextFn, state: i32, event: i32) -> (i32, i32) {
    let mut v = -7;
    (f(state, event, &mut v), v)
}

fn replay(f: ReplayFn, start: i32, events: &[i32]) -> (i32, i32) {
    let mut v = -7;
    (f(start, events.as_ptr(), events.len() as i32, &mut v), v)
}

/// **Every edge of the table, and both ways to be refused.**
#[test]
fn every_transition_lands_where_the_table_says() {
    let (_out, m) = build("next");
    let f: NextFn = sym(&m, b"mlx_next_state\0");

    assert_eq!(next(f, CREATED, PAY), (0, PAID));
    assert_eq!(next(f, CREATED, CANCEL), (0, CANCELLED));
    assert_eq!(next(f, PAID, SHIP), (0, SHIPPED));
    assert_eq!(
        next(f, PAID, CANCEL),
        (0, REFUNDED),
        "cancelling a paid order refunds it"
    );
    assert_eq!(next(f, SHIPPED, DELIVER), (0, DELIVERED));
    assert_eq!(next(f, DELIVERED, REFUND), (0, REFUNDED));

    // An event the state does not accept is the module's domain error, and the out value is
    // untouched — the host can tell "refused" from "moved to state -7".
    assert_eq!(next(f, SHIPPED, PAY), (E_BAD_TRANSITION, -7));
    assert_eq!(
        next(f, CANCELLED, PAY),
        (E_BAD_TRANSITION, -7),
        "a terminal state takes nothing"
    );
    assert_eq!(next(f, REFUNDED, REFUND), (E_BAD_TRANSITION, -7));

    // A code outside the table is a different failure, checked before any edge.
    assert_eq!(next(f, 0, PAY), (E_UNKNOWN_STATE, -7));
    assert_eq!(next(f, 99, PAY), (E_UNKNOWN_STATE, -7));

    drop(m);
}

/// **A sequence folds through `try`, and a refusal stops it where it happened.**
#[test]
fn replay_folds_a_sequence_and_stops_at_the_first_refusal() {
    let (_out, m) = build("replay");
    let f: ReplayFn = sym(&m, b"mlx_replay\0");

    assert_eq!(replay(f, CREATED, &[PAY, SHIP, DELIVER]), (0, DELIVERED));
    assert_eq!(
        replay(f, CREATED, &[PAY, SHIP, DELIVER, REFUND]),
        (0, REFUNDED)
    );
    assert_eq!(replay(f, CREATED, &[PAY, CANCEL]), (0, REFUNDED));

    // The third event is refused: the callee's status comes back unchanged through `try`, and
    // the out value is not the state reached after two events.
    assert_eq!(
        replay(f, CREATED, &[PAY, SHIP, CANCEL]),
        (E_BAD_TRANSITION, -7)
    );

    // No events: the start state, but only a known one — without the check an empty replay
    // would hand back any number as if it were a state.
    assert_eq!(replay(f, CREATED, &[]), (0, CREATED));
    assert_eq!(replay(f, 99, &[]), (E_UNKNOWN_STATE, -7));

    drop(m);
}

/// **Terminal states, and the labels a person reads.**
#[test]
fn terminal_states_and_labels() {
    let (_out, m) = build("label");
    let terminal: TerminalFn = sym(&m, b"mlx_is_terminal\0");
    for s in [CREATED, PAID, SHIPPED, DELIVERED] {
        assert!(!terminal(s), "state {s} still takes events");
    }
    assert!(terminal(CANCELLED));
    assert!(terminal(REFUNDED));

    let label: LabelFn = sym(&m, b"mlx_state_label\0");
    let call = |s: i32| {
        let mut buf = [0u8; 32];
        let mut needed = -7;
        let st = label(s, buf.as_mut_ptr(), 32, &mut needed);
        let n = (needed.max(1) as usize) - 1;
        (st, String::from_utf8_lossy(&buf[..n]).into_owned())
    };
    assert_eq!(call(PAID), (0, "결제됨".to_string()));
    assert_eq!(call(DELIVERED), (0, "배송완료".to_string()));
    assert_eq!(call(9).0, E_UNKNOWN_STATE);

    drop(m);
}

/// **The old form: a state written as a function body passes the gate when renumbered.**
///
/// Kept as the baseline this slice was measured against (§9-67.2, `HOST_ABI.md` cites it). A
/// body-only edit keeps the header and the fingerprint, so a host built against the old
/// numbering loads the new module, passes the gate, and reads 7 as a state it does not know.
#[test]
fn a_state_written_as_a_function_body_is_outside_the_fingerprint() {
    let body = "fn paid() -> i32 { return 2 }\n\
                export fn next_state(state: i32, event: i32) -> i32! {\n\
                  if state == 1 && event == 1 { return paid() }\n\
                  return 0\n\
                }";
    let renumbered = body.replace("return 2 }", "return 7 }");
    assert_ne!(renumbered, body);

    let old = mlc::compile_to_ir(body).expect("compile");
    let new = mlc::compile_to_ir(&renumbered).expect("compile renumbered");
    assert_eq!(
        mlc::header::emit_c_header(&old, "order"),
        mlc::header::emit_c_header(&new, "order"),
        "the header gained something that depends on a function body"
    );
    assert_eq!(mlc::iface::fingerprint(&old), mlc::iface::fingerprint(&new));

    // ...while the module's answer did move.
    let out = common::TempOut::new("order_fnbody");
    let arts = emit_artifacts(&renumbered, "order", &out).expect("emit renumbered");
    let m = Module::load(arts.dll.to_str().unwrap()).expect("load renumbered");
    let f: NextFn = sym(&m, b"mlx_next_state\0");
    assert_eq!(next(f, CREATED, PAY), (0, 7));
    drop(m);
}

/// **The example as written: renumbering an exported state moves the header and the fingerprint.**
///
/// SPEC-constants acceptance D for the module that motivated it. The C host measures the other
/// half — that the moved fingerprint is REFUSED at load (`c_host.rs`, acceptance E).
#[test]
fn renumbering_an_exported_state_moves_the_header_and_the_fingerprint() {
    let renumbered = SRC.replace("export const PAID = 2", "export const PAID = 7");
    assert_ne!(
        renumbered, SRC,
        "the example no longer declares PAID as written here"
    );

    let old = mlc::compile_to_ir(SRC).expect("compile order");
    let new = mlc::compile_to_ir(&renumbered).expect("compile renumbered order");
    let (h_old, h_new) = (
        mlc::header::emit_c_header(&old, "order"),
        mlc::header::emit_c_header(&new, "order"),
    );
    assert!(h_old.contains("#define ML_ORDER_CONST_PAID 2\n"), "{h_old}");
    assert!(h_new.contains("#define ML_ORDER_CONST_PAID 7\n"), "{h_new}");
    assert_ne!(
        mlc::iface::fingerprint(&old),
        mlc::iface::fingerprint(&new),
        "a renumbered export const must move the fingerprint the host gates on"
    );
}

/// **Constants are values, not symbols** (SPEC-constants acceptance F): the export table is the
/// four functions and the two reserved symbols, exactly as before the rewrite.
#[test]
fn exported_constants_add_no_export() {
    let out = common::TempOut::new("order_exports");
    let arts = emit_artifacts(SRC, "order", &out).expect("emit order");
    let mut exports = pe::read_exports(&arts.dll).expect("read exports");
    exports.sort();
    assert_eq!(
        exports,
        [
            "ml_iface_hash_order",
            "ml_module_abi_version",
            "mlx_is_terminal",
            "mlx_next_state",
            "mlx_replay",
            "mlx_state_label",
        ]
    );
}

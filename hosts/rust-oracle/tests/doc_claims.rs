//! Prose that states a measured fact, checked against the thing it describes.
//!
//! This repository has watched the same failure three times (`STATUS.md` §7-1): a slice
//! changes a measured number, the code and one document are updated, and the other
//! documents keep asserting the old value in the present tense. The repository is public,
//! so those sentences are the outward-facing claim.
//!
//! `language_gaps.rs` already does this for the language-gap list. Until this file there
//! was nothing doing it for the numbers — which is why the export count, the gated-module
//! count, and four "not implemented anywhere" notes all drifted at once.
//!
//! Deliberately NOT `cfg(windows)`: this is text, so the ubuntu insurance job runs it too.

use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    // tests/ -> hosts/rust-oracle -> hosts -> repo root
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .to_path_buf()
}

fn read(rel: &str) -> String {
    let p = repo_root().join(rel);
    std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()))
}

/// Every number N appearing as `<prefix>N<suffix>`. Digits are ASCII, so byte slicing is
/// safe even though the surrounding prose is Korean.
fn numbers_between(hay: &str, prefix: &str, suffix: &str) -> Vec<usize> {
    let mut out = Vec::new();
    let mut rest = hay;
    while let Some(i) = rest.find(prefix) {
        let after = &rest[i + prefix.len()..];
        let digits: String = after.chars().take_while(|c| c.is_ascii_digit()).collect();
        if !digits.is_empty() && after[digits.len()..].starts_with(suffix) {
            out.push(digits.parse().expect("ascii digits"));
        }
        rest = after;
    }
    out
}

/// The reference C host gates every module it loads. The count is a protection proxy, and
/// two documents state it in the present tense — so it has to be the count in `host.c`.
///
/// It drifted once already: the fingerprint slice (#105) took it from 2 to 13, then the
/// string-concat slice (#108) added `receipt.dll` and the documents stayed at 13.
#[test]
fn the_gated_module_count_in_the_docs_is_the_count_in_host_c() {
    let host_c = read("hosts/c-host/host.c");
    let gated = host_c.matches("load(dir, \"").count();
    assert!(
        gated >= 10,
        "expected the C host to gate a corpus of modules, found {gated} — has the call \
         shape changed? This test recognises modules by `load(dir, \"`"
    );

    for doc in ["docs/SECURITY.md", "docs/HOST_ABI.md"] {
        let text = read(doc);
        let stated = numbers_between(&text, "로드하는 모듈 ", "개");
        assert!(
            !stated.is_empty(),
            "{doc} no longer states how many modules the reference host gates; either put \
             the sentence back or drop this document from the check"
        );
        for n in stated {
            assert_eq!(
                n, gated,
                "{doc} says the reference C host gates {n} modules; host.c gates {gated}"
            );
        }
    }
}

/// Both README versions describe the export table as a protection proxy. The fingerprint
/// slice added a second reserved symbol, so the count is three — and the prose said two
/// for two slices while the code block right below it listed all three.
#[test]
fn the_readmes_do_not_understate_the_export_set() {
    // The names are not invented here: this is the set `protection.rs` pins with an
    // assert_eq! against the real PE export table.
    let pinned = ["ml_iface_hash", "ml_module_abi_version", "mlx_discount"];

    let protection = read("hosts/rust-oracle/tests/protection.rs");
    for name in pinned {
        assert!(
            protection.contains(name),
            "protection.rs no longer pins '{name}' — this test's idea of the export set is \
             stale, not the README's"
        );
    }

    for doc in ["README.md", "README.ko.md"] {
        let text = read(doc);
        for name in pinned {
            assert!(
                text.contains(name),
                "{doc} does not mention '{name}', which every module exports"
            );
        }
        // Negative pins: the exact sentences that were left behind by #105.
        for stale in ["the two symbols", "심볼 두 개"] {
            assert!(
                !text.contains(stale),
                "{doc} still says '{stale}'. A module exports {} symbols today \
                 (protection.rs asserts the set)",
                pinned.len()
            );
        }
    }
}

/// D18 puts the version check on the host. For a year that was true of this repository
/// too — nothing here refused anything — and four files said so. The reference C host now
/// refuses, so no file may still say the refusal exists nowhere.
///
/// The invariant is conditional on purpose: if the gate is ever removed from `host.c`,
/// this test stops demanding that the documents claim it.
///
/// Two halves, and the second one matters more. A denylist of stale phrases only catches
/// the exact sentences that were there before — a paraphrase, or a Korean restatement,
/// walks straight through it. So each file must also carry a POSITIVE sentence saying the
/// reference host refuses. Deleting the correction is then a failure, not a silence.
/// (Grok raised this while verifying the change that introduced the denylist.)
#[test]
fn no_file_says_the_version_refusal_is_unimplemented_while_host_c_implements_it() {
    let host_c = read("hosts/c-host/host.c");
    let refuses = host_c.contains("abi() != (uint32_t)expected_abi")
        && host_c.contains("refuse %s: module abi");
    assert!(
        refuses,
        "host.c no longer refuses an ABI mismatch — acceptance D lost a gate, or this test \
         is matching the wrong lines"
    );

    let stale = [
        "not yet implemented anywhere in this repo",
        "not enforced anywhere in this repo",
        "not something this repo enforces",
        "implemented nowhere in this repo",
    ];
    for doc in [
        "runtime/ml_abi.h",
        "runtime/README.md",
        "compiler/src/abi.rs",
        "hosts/c-host/README.md",
    ] {
        let text = read(doc);
        for phrase in stale {
            assert!(
                !text.contains(phrase),
                "{doc} says the ABI version refusal is '{phrase}', but hosts/c-host/host.c \
                 implements it and acceptance D exercises it"
            );
        }
        // The positive half: absence of the old sentence is not the presence of the true one.
        let affirms = ["reference C host", "This host does reject"];
        assert!(
            affirms.iter().any(|a| text.contains(a)),
            "{doc} no longer says that the reference C host refuses a version mismatch. \
             Dropping the denial is not enough — the file has to state what is true, or a \
             reworded denial passes this test"
        );
    }
}

/// `host.c` is compiled by MSVC with `/W4 /WX`, and a non-ASCII byte in it is warning
/// C4819 ("cannot be represented in the current code page") which `/WX` turns into an
/// error. That gate is real but it costs a full acceptance-D build and only runs where
/// MSVC exists; this costs nothing and runs on the ubuntu job too.
///
/// Written after exactly that: an em dash in a comment failed the C build.
#[test]
fn the_c_sources_are_ascii_only() {
    // One call per C source compiled under /WX. A second one is a second line.
    assert_ascii("hosts/c-host/host.c");
}

fn assert_ascii(rel: &str) {
    let text = read(rel);
    if let Some((i, ch)) = text.char_indices().find(|(_, c)| !c.is_ascii()) {
        let line = text[..i].matches('\n').count() + 1;
        panic!(
            "{rel}:{line} contains the non-ASCII character {ch:?}. MSVC compiles this file \
             with /W4 /WX, where a byte outside the active code page is C4819 and therefore \
             an error. Use ASCII punctuation in C sources"
        );
    }
}

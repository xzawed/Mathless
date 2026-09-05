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
    // Comments stripped first: a `load(dir, "…")` written inside a comment would inflate the
    // count and make the documents "agree" with a number nothing loads. There is no such
    // comment today (measured, 0 hits) — this keeps it that way for free.
    let host_c = strip_c_comments(&read("hosts/c-host/host.c"));
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
    // DERIVED, not written here. The earlier version listed the three names in this file and
    // only checked that protection.rs also contained them — which its own comment described
    // as "not invented here" while inventing them. A fourth export would have slipped past
    // this guard entirely (protection.rs would still have caught it, but the READMEs would
    // have gone stale silently, which is the exact failure this file exists to stop).
    let protection = read("hosts/rust-oracle/tests/protection.rs");
    let pinned = string_literals_in_vec_after(&protection, "        exports,");
    assert!(
        pinned.len() >= 3,
        "recovered {pinned:?} from protection.rs's export assertion — has its shape changed?"
    );

    for doc in ["README.md", "README.ko.md"] {
        let text = read(doc);
        for name in &pinned {
            assert!(
                text.contains(name),
                "{doc} does not mention '{name}', which every module exports. \
                 protection.rs pins the set as {pinned:?}"
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

/// The `"..."` literals of the first `vec![ … ]` that follows `anchor`, in source order.
///
/// Used to read an expected set out of the test that owns it, so a second test cannot hold a
/// stale copy of the same list.
fn string_literals_in_vec_after(src: &str, anchor: &str) -> Vec<String> {
    let Some(a) = src.find(anchor) else {
        panic!("anchor {anchor:?} not found — the test it reads from has changed shape");
    };
    let rest = &src[a..];
    let Some(open) = rest.find("vec![") else {
        panic!("no vec![ after {anchor:?}");
    };
    let body = &rest[open + "vec![".len()..];
    let end = body.find(']').unwrap_or(body.len());
    let body = &body[..end];

    let mut out = Vec::new();
    let mut rest = body;
    while let Some(q) = rest.find('"') {
        let after = &rest[q + 1..];
        match after.find('"') {
            Some(close) => {
                out.push(after[..close].to_string());
                rest = &after[close + 1..];
            }
            None => break,
        }
    }
    out
}

/// Collapse a file to a single line of prose, so a multi-word phrase can be matched without
/// the match depending on where the line happened to wrap.
///
/// Comment markers (`//!`, `///`, `//`, a C block comment's leading `*`, Markdown bullets
/// and quote markers) are dropped from the front of each line; backticks and asterisks are
/// dropped everywhere, so inline markup does not split a phrase; runs of whitespace collapse
/// to one space. This exists so a *meaningful* phrase can be required. The alternative —
/// shortening the required text until it survives any reflow — is how the pin it serves
/// degenerated into a noun phrase that a negated sentence satisfied.
fn flatten_prose(src: &str) -> String {
    let mut words: Vec<String> = Vec::new();
    for line in src.lines() {
        let mut t = line.trim();
        loop {
            let before = t;
            for marker in ["//!", "///", "//", "*", ">", "#", "-"] {
                if let Some(rest) = t.strip_prefix(marker) {
                    t = rest.trim_start();
                }
            }
            if t == before {
                break;
            }
        }
        let cleaned: String = t.chars().filter(|c| *c != '`' && *c != '*').collect();
        words.extend(cleaned.split_whitespace().map(str::to_string));
    }
    words.join(" ")
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
        //
        // This used to be `["reference C host", "This host does reject"]` — two bare
        // substrings, and the first one is just a NOUN PHRASE. "The reference C host does
        // not refuse a version mismatch" contains it, so the sentence that most needed to
        // fail passed (STATUS §9-A A6). A positive pin has to require the predicate, and
        // the phrases below all carry subject + affirmative verb in one span.
        // Every accepted phrasing carries its own polarity. An earlier draft of this list
        // also took "reference C host does exactly that", which `runtime/ml_abi.h` used —
        // and that is an anaphor: the refusal lives in the PREVIOUS sentence, outside the
        // span being matched, so inverting that sentence would have left this green. The
        // header now states the claim in one clause instead (Grok raised the hole while
        // verifying this change).
        let flat = flatten_prose(&text);
        let affirms = [
            "reference C host does refuse",
            "reference C host does reject",
            "reference C host refuses",
            "reference C host rejects",
            "This host does refuse",
            "This host does reject",
        ];
        assert!(
            affirms.iter().any(|a| flat.contains(a)),
            "{doc} no longer states, in one sentence, that the reference C host refuses a \
             version mismatch. Dropping the denial is not enough — the file has to say what \
             is true. Accepted phrasings: {affirms:?}"
        );
        // Belt and braces: the negated forms of the same sentence, which no rewording of a
        // true claim can produce.
        for denial in [
            "reference C host does not",
            "reference C host never",
            "reference C host cannot",
            "This host does not refuse",
            "This host does not reject",
        ] {
            assert!(
                !flat.contains(denial),
                "{doc} says '{denial}', but hosts/c-host/host.c refuses on every module it \
                 loads and acceptance D exercises it"
            );
        }
    }
}

/// The one piece of prose that travels WITH the artifact.
///
/// Every generated header carries a note saying what has and has not been verified. For a
/// whole slice after link-time binding was verified with a measured run, that note still
/// told each user "Not verified: … link-time binding via an import library". A stale
/// document is bad; a stale sentence compiled into the product's own output is worse,
/// because it reaches people who never open this repository.
///
/// Conditional on the evidence existing, the same shape as the ABI-refusal guard: if the
/// link host is deleted, this stops demanding the claim.
#[test]
fn the_generated_header_does_not_deny_a_verification_that_exists() {
    let link_host = repo_root().join("hosts").join("c-host-link").join("host.c");
    if !link_host.exists() {
        return;
    }

    // The EMITTED note, not the emitter's source. Matching source text was the first shape
    // of this test and of its neighbour above; both were brittle in the same way — a
    // reworded or reflowed literal breaks the match without changing the artifact, and a
    // literal left behind unused satisfies it without reaching the artifact. What a user
    // reads is the output, so that is what is asserted.
    let ir = mlc::compile_to_ir("export fn f(x: f64) -> f64 { return x }\n")
        .expect("the probe module must compile");
    let h = mlc::header::emit_c_header(&ir, "widget");

    assert!(
        !h.contains("link-time binding via an"),
        "the generated header still tells its reader 'Not verified: … link-time binding via \
         an import library', but hosts/c-host-link/host.c links against the packaged .lib \
         and acceptance D runs it:\n{h}"
    );
    assert!(
        h.contains("hosts/c-host-link"),
        "the generated header's verification note does not mention the link host. Dropping \
         the denial is not enough — the note is what a user reads to know which consumption \
         paths were actually proved:\n{h}"
    );
}

/// The error constant carries its module, and the emitter cannot quietly drop it again.
///
/// `SPEC-error-prefix` §3-F. The unprefixed `ML_ERR_<NAME>` survived four slices while the
/// fingerprint constant beside it was given a prefix *because this debt was recorded* — the
/// rule was decided and simply not applied here. This is the assertion that keeps it applied.
///
/// It reads the emitter, not the output: a golden can be re-blessed, and a re-bless that
/// silently accepted a bare `ML_ERR_` is exactly the failure mode this file exists for.
#[test]
fn the_emitter_prefixes_error_constants_with_the_module() {
    // Run the emitter and read WHAT IT WROTE. The first version of this test matched source
    // text in header.rs instead — including a `format!` call character for character — and
    // Grok showed it had a hole big enough to drive the regression through: leave
    // `error_macro` defined but stop calling it, and every source-text assertion still
    // passed while the emitted constants lost their prefix. It also failed on harmless
    // changes (rustfmt wrapping the call, renaming the helper).
    //
    // Text generation needs no build, so this stays off `cfg(windows)` and the ubuntu job
    // runs it. The module name is deliberately NOT one of the corpus names: the rule is
    // being checked, not one fixture.
    let ir = mlc::compile_to_ir(
        "error E_NEG = 3\n\
         error E_LATE = 4\n\
         export fn take(x: f64) -> f64! {\n\
         \x20 if x < 0.0 { fail E_NEG }\n\
         \x20 return x\n\
         }\n",
    )
    .expect("the probe module must compile");

    let h = mlc::header::emit_c_header(&ir, "widget");
    let pas = mlc::header::emit_delphi_unit(&ir, "widget", "widget");

    for (binding, text, expected) in [
        ("the C header", &h, "#define ML_WIDGET_ERR_E_NEG 3"),
        ("the Delphi unit", &pas, "ML_WIDGET_ERR_E_NEG = 3;"),
    ] {
        assert!(
            text.contains(expected),
            "{binding} does not carry the module in the error constant's name (expected \
             '{expected}'). Two modules that both declare a common error name would collide \
             again — measured as C4005 under /W4 /WX before Q14 was closed:\n{text}"
        );
        assert!(
            !text.contains("ML_ERR_"),
            "{binding} still emits a bare 'ML_ERR_', which is the collision itself:\n{text}"
        );
    }

    // The guard that must NOT be added (DP-Q3): it would let the first header win and turn
    // that loud C4005 into a silent wrong meaning.
    assert!(
        !h.contains("#ifndef ML_WIDGET_ERR"),
        "an #ifndef guard around a per-module error constant makes a genuine conflict silent"
    );
}

/// Thousands separators the way the documents write them: `9728` -> `"9,728"`.
fn with_commas(n: u64) -> String {
    let digits = n.to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// `runtime/ml_abi.h` declares the reserved half of the ABI by hand, and nothing compiles
/// it, so nothing noticed when it fell behind: every module has exported `ml_iface_hash`
/// since #105 and the header that exists to list the reserved symbols did not mention it.
///
/// This derives the list from the emitter instead of trusting the file. Any reserved `ml_*`
/// declaration `header.rs` writes into a generated header must also be here — with or
/// without parameters — and the truncation status must carry the same value in both places.
///
/// It also forbids a module-specific `mlx_*` declaration. The file used to declare
/// `mlx_discount` from one example — the kind of detail that goes stale in a file nobody
/// compiles, and the reason D4 was open at all.
#[test]
fn the_hand_written_abi_header_declares_every_reserved_symbol_the_compiler_emits() {
    let header_rs = read("compiler/src/header.rs");
    let abi_h = read("runtime/ml_abi.h");

    // Declarations the emitter writes, recovered from its string literals. Every literal on
    // a line is a candidate; one that names an `ml_` symbol and ends a declaration is a
    // reserved export. Deliberately not limited to `(void);` — a reserved export that takes
    // an argument would otherwise slip past exactly the way `ml_iface_hash` did (Grok raised
    // this while verifying the narrower first version).
    let mut reserved: Vec<&str> = Vec::new();
    for line in header_rs.lines() {
        for (i, part) in line.split('"').enumerate() {
            if i % 2 == 1 && part.contains(" ml_") && part.ends_with(");") && !part.contains('\\') {
                reserved.push(part);
            }
        }
    }
    reserved.sort_unstable();
    reserved.dedup();
    assert!(
        reserved.len() >= 2,
        "expected header.rs to emit at least the two reserved symbols, recovered {reserved:?} \
         — has the emitter's shape changed?"
    );

    for decl in &reserved {
        assert!(
            abi_h.contains(decl),
            "compiler/src/header.rs emits '{decl}' into every generated header, but \
             runtime/ml_abi.h does not declare it. That file is the hand-written list of \
             reserved symbols; nothing compiles it, so only this test can notice"
        );
    }

    for line in abi_h.lines() {
        assert!(
            !(line.contains("mlx_") && line.trim_end().ends_with(");")),
            "runtime/ml_abi.h declares a module function ('{}'). Module exports belong in \
             the module's generated header — a specific mlx_ name here goes stale unnoticed",
            line.trim()
        );
    }

    // It also TEACHES a naming rule, and that rule has to be the one the emitter follows.
    // It did not: after Q14 renamed error constants, this file still told a third-party host
    // author they would find `ML_ERR_<NAME>` in a generated header — while, four dozen lines
    // lower, correctly describing `ML_<MODULE>_IFACE_HASH`. One hand-written contract
    // teaching two naming rules is the exact state Q14 existed to remove.
    //
    // Derived from the emitter, not asserted as a literal: build a header and read the shape
    // back out of it.
    let ir = mlc::compile_to_ir(
        "error E_NEG = 3\nexport fn take(x: f64) -> f64! { if x < 0.0 { fail E_NEG } return x }\n",
    )
    .expect("the probe module must compile");
    let probe = mlc::header::emit_c_header(&ir, "widget");
    // Asserted, NOT used as an `if` gate. Behind a gate, a change to the emitted shape would
    // skip the two checks below instead of failing them — the same silent-skip pattern this
    // file keeps finding elsewhere. Grok pointed it out one review after the gate was
    // written; if the shape moves, this line is where it stops.
    assert!(
        probe.contains("ML_WIDGET_ERR_E_NEG"),
        "the emitter no longer produces ML_<MODULE>_ERR_<NAME>; the contract this test holds \
         ml_abi.h to is derived from that shape:\n{probe}"
    );
    assert!(
        abi_h.contains("ML_<MODULE>_ERR_<NAME>"),
        "runtime/ml_abi.h does not describe the error-constant shape the emitter produces. A \
         generated header defines ML_<MODULE>_ERR_<NAME>; this file is what a third-party \
         host author reads to learn that"
    );
    assert!(
        !abi_h.contains("ML_ERR_<NAME>"),
        "runtime/ml_abi.h still teaches the pre-Q14 shape ML_ERR_<NAME>, which no generated \
         header has used since 2026-09-03"
    );

    // The one negative status that exists is defined in both places, and the values must
    // agree: a translation unit can see both, and the `#ifndef` guard means a mismatch is
    // NOT a redefinition error — it silently resolves to whichever was seen first.
    //
    // Asserted as two positives, not as `assert_eq!` of two `contains` flags. That earlier
    // shape passed when the definition was missing from BOTH files, which is a vacuous
    // success of exactly the kind section 7-1 warns about (Grok caught it in review).
    let define = "#define ML_ST_INSUFFICIENT_BUFFER (-1)";
    assert!(
        header_rs.contains(define),
        "compiler/src/header.rs no longer emits '{define}' — if the truncation status \
         changed value, runtime/ml_abi.h has to change with it"
    );
    assert!(
        abi_h.contains(define),
        "runtime/ml_abi.h no longer declares '{define}' while header.rs still emits it. \
         Both are `#ifndef`-guarded, so a divergence is silent: the translation unit keeps \
         whichever definition it saw first"
    );
}

/// The artifact set, checked against the emitter rather than against a remembered list.
///
/// `SPEC-linkable-bindings` §3-F. The set went from three files to four in that slice, and
/// the number appears in prose in three documents — the exact shape that drifted for the
/// export count and the gated-module count before it. So the extensions are read out of
/// `emit_artifacts`'s `names` array, and every document that describes the set has to name
/// each one.
///
/// A fifth artifact fails this test until the documents mention it.
#[test]
fn every_artifact_the_emitter_writes_is_named_in_the_docs() {
    let emit_rs = read("compiler/src/emit.rs");
    let start = emit_rs
        .find("let names = [")
        .expect("emit.rs no longer has the `names` array this test reads the artifact set from");
    let end = emit_rs[start..]
        .find("];")
        .expect("unterminated `names` array")
        + start;
    let block = &emit_rs[start..end];

    let marker = "format!(\"{module_name}";
    let mut exts: Vec<&str> = Vec::new();
    for (i, _) in block.match_indices(marker) {
        let rest = &block[i + marker.len()..];
        if let Some(close) = rest.find('"') {
            exts.push(&rest[..close]);
        }
    }
    exts.sort_unstable();
    exts.dedup();
    assert!(
        exts.len() >= 3,
        "recovered only {exts:?} from emit.rs — has the array's shape changed?"
    );

    // Three documents was the set when this was written, and three of the files that describe
    // the same artifacts sat outside it (STATUS §9-A A2). `LICENSE-OUTPUT-EXCEPTION` is the
    // sharpest of them: it ENUMERATES the artifacts and grants rights over them, so an
    // artifact missing from that list is a legal sentence that does not cover what `mlc`
    // hands the user. `docs/STATUS.md` §1 states the set as current fact.
    for doc in [
        "README.md",
        "README.ko.md",
        "docs/HOST_ABI.md",
        "LICENSE-OUTPUT-EXCEPTION",
        "docs/STATUS.md",
        // D23 is the decision record for a LICENCE GRANT, and it enumerated four items while
        // LICENSE-OUTPUT-EXCEPTION §1 listed five — the import library was missing from the
        // decision that points at that licence. A grant that under-lists what it grants is the
        // worst place for this drift, and it sat outside this guard until 2026-09-05.
        "docs/DECISIONS.md",
    ] {
        let text = read(doc);
        for ext in &exts {
            assert!(
                text.contains(&format!("`{ext}`")),
                "{doc} describes what `mlc build` writes but never mentions `{ext}`, which \
                 emit.rs packages. The artifact set is {exts:?}"
            );
        }
    }

    // And the CLI itself, which is the one place a user is TOLD what was written. It cannot be
    // checked the same way — the paths come from `arts.<field>.display()`, not from a literal
    // extension — so the check is that it prints one line per artifact.
    let main_rs = read("compiler/src/main.rs");
    let reported = [
        "arts.dll",
        "arts.header",
        "arts.delphi_unit",
        "arts.import_lib",
    ]
    .iter()
    .filter(|f| main_rs.contains(&format!("{f}.display()")))
    .count();
    assert_eq!(
        reported,
        exts.len(),
        "`mlc build` writes {} artifacts {exts:?} but its success output names {reported}. \
         A file written and not reported is one the user does not know they have",
        exts.len()
    );

    // D23 specifically, because the whole-file check above is not enough for it. D23 is the
    // decision record for a LICENCE GRANT: it enumerates what belongs to the user. A
    // file-wide `contains` is satisfied by the D18 addendum mentioning the same extension
    // somewhere else, so the grant could quietly under-list again and stay green — measured,
    // and Grok raised it independently. The grant's own sentence is therefore checked.
    let decisions = read("docs/DECISIONS.md");
    let at = decisions
        .find("D23 산출물 라이선스")
        .expect("docs/DECISIONS.md no longer has a D23 licence entry — this test reads it");
    let grant: String = decisions[at..]
        .lines()
        .take(3)
        .collect::<Vec<_>>()
        .join(" ");
    for ext in &exts {
        assert!(
            grant.contains(&format!("`{ext}`")),
            "D23 grants the user what `mlc` produces but its own enumeration omits `{ext}`. \
             LICENSE-OUTPUT-EXCEPTION §1 lists it, so the decision record under-states the \
             licence it points at. The artifact set is {exts:?}.\nD23 says: {grant}"
        );
    }
}

/// The README's `mlc build` transcript, against what the CLI actually prints.
///
/// The block is an illustration, not a capture, and the filenames matched — but the note on
/// the `.pas` line did not travel with it, and that note is the one that says the Delphi
/// binding is unverified. A README that lists the unit with no qualifier reads as "this
/// works"; `mlc` itself is careful to say otherwise on that exact line (STATUS §9-A A11).
///
/// Pinned to the `.pas` LINE, not the block: appended to any other line the note would still
/// be "in" the block while saying nothing about the unit.
#[test]
fn the_readme_transcripts_carry_the_draft_note_the_cli_prints() {
    // Read out of the `println!` that prints the DELPHI UNIT, not out of the file at large.
    // A bare search would keep passing on a literal left behind after the print was deleted —
    // the same "matched the emitter's source, not what it emits" hole #130 fixed for the
    // generated header (Grok raised it here). If that println! goes, this test stops finding
    // its anchor and fails, which is the intended behaviour: the note is a claim about output.
    let main_rs = read("compiler/src/main.rs");
    let at = main_rs
        .find("arts.delphi_unit.display()")
        .expect("compiler/src/main.rs no longer prints the Delphi unit's path");
    let from = main_rs[..at]
        .rfind("println!(")
        .expect("the Delphi unit's path is no longer printed by a println!");
    let block = &main_rs[from..at];
    let i = block
        .find("(DRAFT: ")
        .expect("the line `mlc` prints for the .pas no longer carries a (DRAFT: …) note");
    let note: String = block[i..]
        .chars()
        .take_while(|c| *c != ')' && *c != '\n')
        .collect();
    let note = format!("{note})");

    for doc in ["README.md", "README.ko.md"] {
        let text = read(doc);
        let line = text
            .lines()
            .find(|l| l.contains("discount.pas"))
            .unwrap_or_else(|| panic!("{doc} no longer shows a `mlc build` transcript"));
        assert!(
            line.contains(&note),
            "{doc}'s transcript line for the Delphi unit is\n  {line}\nbut `mlc` prints\n  \
             {note}\nThe note is what tells a reader the Delphi binding is unverified"
        );
    }
}

/// A `const NAME: u64 = 1_234;` literal, read out of Rust source text.
fn u64_const(src: &str, name: &str) -> u64 {
    let marker = format!("const {name}: u64 = ");
    let i = src
        .find(&marker)
        .unwrap_or_else(|| panic!("protection.rs no longer defines {name} — this test reads it"));
    let literal: String = src[i + marker.len()..]
        .chars()
        .take_while(|c| c.is_ascii_digit() || *c == '_')
        .collect();
    literal
        .replace('_', "")
        .parse()
        .unwrap_or_else(|_| panic!("{name} is not a decimal literal"))
}

/// The artifact-size proxy, checked in the direction nothing checked before: the documents
/// must carry what was actually measured.
///
/// D3 was decided as "assert the exact value" and CI disproved the premise in one run — the
/// same commit and the same pinned rustc produced 9,728 B here and 9,216 B on
/// `windows-latest`, one `FileAlignment` block apart, because the pin covers rustc and not
/// MSVC `link.exe`. The number ten documents publish as a project fact is a *this-machine*
/// number.
///
/// So the guard is not "the docs match one constant" but "the docs carry BOTH observations".
/// A document that quietly goes back to a single exact byte count fails here.
///
/// Only PRESENT-TENSE statements are checked. Every `docs/slices/SPEC-*.md` also carries the
/// old number, but `slices/README.md` says plainly that each SPEC is the design record of
/// its own moment — rewriting those would be rewriting history, not fixing a claim.
#[test]
fn the_published_module_size_carries_both_measurements() {
    let protection = read("hosts/rust-oracle/tests/protection.rs");
    let observed = [
        u64_const(&protection, "DISCOUNT_DLL_MEASURED_DEV"),
        u64_const(&protection, "DISCOUNT_DLL_MEASURED_CI"),
    ];

    // The READMEs were NOT in this list when the size guard was written, and both kept
    // publishing "about 9.7 KB" — the dev machine's value alone — for a slice after the
    // measurement that disproved it. They are the outermost documents in a public
    // repository, so leaving them out was the wrong half to leave out.
    for doc in [
        "docs/SECURITY.md",
        "docs/STATUS.md",
        "README.md",
        "README.ko.md",
    ] {
        let text = read(doc);
        for n in observed {
            let stated = with_commas(n);
            assert!(
                text.contains(&stated),
                "{doc} does not state '{stated} B'. The stripped module measures \
                 {} B on the development machine and {} B on GitHub's windows-latest \
                 runner; a document that publishes only one of them presents a \
                 machine-specific number as a project fact",
                observed[0],
                observed[1]
            );
        }
    }
}

/// Acceptance D is the only thing in this repository that compiles a generated `.h` as C.
/// An example whose header is not included there has a surface no C compiler has ever
/// read: the Rust oracle checks the module's behaviour, not whether the header it ships
/// beside it is valid C11 under `/W4 /WX`.
///
/// This was measured as a real hole — 14 of 18 — and the worst of the four outside it was
/// `shapes`, the file written to collect "export shapes where a mis-written C ABI adapter
/// would compile and return a plausible wrong value" (`STATUS.md` N1).
#[test]
fn every_example_header_is_compiled_by_the_c_host() {
    // Comments stripped: a commented-out `#include "shapes.h"` would satisfy the check below
    // while the C compiler never sees that header — the coverage hole this test exists to
    // close, reopened in a way that reads as closed. None exists today (measured).
    let host_c = strip_c_comments(&read("hosts/c-host/host.c"));
    let dir = repo_root().join("examples");

    let mut examples: Vec<String> = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("{}: {e}", dir.display()))
        .filter_map(|entry| {
            let path = entry.expect("dir entry").path();
            if path.extension().and_then(|e| e.to_str()) != Some("mls") {
                return None;
            }
            Some(path.file_stem()?.to_str()?.to_string())
        })
        .collect();
    examples.sort();

    assert!(
        examples.len() >= 18,
        "found only {} example(s) under examples/ — the corpus does not shrink, so this is \
         more likely a path problem than a deletion",
        examples.len()
    );

    let missing: Vec<&String> = examples
        .iter()
        .filter(|stem| !host_c.contains(&format!("#include \"{stem}.h\"")))
        .collect();

    assert!(
        missing.is_empty(),
        "hosts/c-host/host.c does not include the generated header for {} of {} examples: \
         {missing:?}. Their headers are never compiled as C, so an invalid one ships \
         unnoticed. Emit them in c_host.rs and include them here",
        missing.len(),
        examples.len()
    );
}

/// `host.c` is compiled by MSVC with `/W4 /WX`, and a non-ASCII byte in it is warning
/// C4819 ("cannot be represented in the current code page") which `/WX` turns into an
/// error. That gate is real but it costs a full acceptance-D build and only runs where
/// MSVC exists; this costs nothing and runs on the ubuntu job too.
///
/// Written after exactly that: an em dash in a comment failed the C build.
#[test]
fn the_c_sources_are_ascii_only() {
    // DISCOVERED, not listed. The earlier version named the two hosts by hand, so a third C
    // host would have been added outside the check — and this is a rule about the language
    // the compiler reads, which applies to every C source, not to two chosen ones.
    // `runtime/ml_abi.h` is compiled by acceptance D, so it is covered there.
    let hosts = repo_root().join("hosts");
    let mut sources: Vec<PathBuf> = Vec::new();
    for entry in std::fs::read_dir(&hosts).expect("hosts/") {
        let dir = entry.expect("dir entry").path();
        if !dir.is_dir() {
            continue;
        }
        for f in std::fs::read_dir(&dir).expect("host dir") {
            let p = f.expect("dir entry").path();
            if p.extension().and_then(|e| e.to_str()) == Some("c") {
                sources.push(p);
            }
        }
    }
    sources.sort();
    assert!(
        sources.len() >= 2,
        "expected at least the two C hosts under hosts/, found {sources:?}"
    );

    for src in &sources {
        let rel = src
            .strip_prefix(repo_root())
            .expect("under the repo root")
            .to_string_lossy()
            .replace('\\', "/");
        assert_ascii(&rel);
    }
}

/// The link host's whole claim is that it never looks a symbol up by name.
///
/// `SPEC-linkable-bindings` §3-B says the host binds through the import library "without
/// ever calling GetProcAddress". A test that only checks the program's OUTPUT cannot tell
/// the difference — a host that quietly fell back to dynamic loading would print the same
/// `LINK_GATE_OK`. So the claim is pinned where it can be checked: in the source.
#[test]
fn the_link_host_never_resolves_a_symbol_by_name() {
    // CODE, not prose. The first version of this test failed on the file's own comment
    // saying "there is deliberately no GetProcAddress here" — a guard that forbids naming
    // the thing you are not doing punishes the explanation, so the comments come out first.
    let host = strip_c_comments(&read("hosts/c-host-link/host.c"));
    for dynamic in ["GetProcAddress", "LoadLibrary", "FreeLibrary", "HMODULE"] {
        assert!(
            !host.contains(dynamic),
            "hosts/c-host-link/host.c mentions '{dynamic}'. That host exists to prove \
             LINK-time binding; if it resolves anything dynamically it is proving the thing \
             hosts/c-host already proves"
        );
    }
    // And it really does call the module and the gate, rather than being an empty shell
    // that trivially satisfies the check above.
    for expected in [
        "mlx_discount(",
        "ml_iface_hash()",
        "ml_module_abi_version()",
    ] {
        assert!(
            host.contains(expected),
            "hosts/c-host-link/host.c does not call '{expected}' — it would pass the \
             no-dynamic-loading check by doing nothing"
        );
    }
}

/// C source with `/* … */` and `// …` removed, so a check can look at what the code does
/// rather than at what its comments talk about.
///
/// Deliberately naive: it does not understand string literals, which is fine for the one
/// file it is used on (no `//` or `/*` inside any string there) and would be over-building
/// for anything this test needs.
fn strip_c_comments(src: &str) -> String {
    let bytes = src.as_bytes();
    let mut out = String::with_capacity(src.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i..].starts_with(b"/*") {
            match src[i + 2..].find("*/") {
                Some(end) => i += 2 + end + 2,
                None => break,
            }
        } else if bytes[i..].starts_with(b"//") {
            match src[i..].find('\n') {
                Some(end) => i += end,
                None => break,
            }
        } else {
            let ch = src[i..].chars().next().expect("char boundary");
            out.push(ch);
            i += ch.len_utf8();
        }
    }
    out
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

/// The one directory `mlc` can leave behind, and whether the READMEs admit it.
///
/// `emit_artifacts` stages its four files inside `out_dir` so the set lands all-or-nothing,
/// and removes the stage on success and on failure. A process that is KILLED never gets to,
/// and STATUS §5-5.9 recorded that "nowhere says so" — recorded, but not measured, for four
/// sessions.
///
/// It is measured now: killing `mlc build` as soon as the stage appears leaves
/// `.mlc-stage-<pid>-<n>` in the output directory, and a later build into the same directory
/// still succeeds and writes all four artifacts. So it is litter, not a broken state — which
/// is worth saying out loud, because a user who finds it cannot tell those apart.
///
/// The prefix is read out of the emitter rather than typed here, so renaming the directory
/// makes this fail instead of quietly leaving both READMEs describing a name nothing creates.
#[test]
fn the_readmes_admit_the_staging_directory_a_killed_build_leaves() {
    let emit_rs = read("compiler/src/emit.rs");
    let marker = "\".mlc-stage-";
    let i = emit_rs
        .find(marker)
        .expect("emit.rs no longer builds a `.mlc-stage-` name — this test reads the prefix");
    let prefix: String = emit_rs[i + 1..]
        .chars()
        .take_while(|c| *c != '{' && *c != '"')
        .collect();
    assert!(
        prefix.starts_with(".mlc-stage-"),
        "recovered {prefix:?} from emit.rs, which is not the staging prefix"
    );

    for doc in ["README.md", "README.ko.md"] {
        let text = read(doc);
        assert!(
            text.contains(&prefix),
            "{doc} never mentions `{prefix}`, the directory `mlc build` leaves in the user's \
             output directory when it is killed. A user who finds it cannot tell litter from \
             a broken build unless a document says which it is"
        );
    }
}

/// The slice index has to be a table, and it has to list every SPEC.
///
/// `CLAUDE.md` calls `docs/slices/README.md` the canonical list of closed slices. It was not
/// one table: five stray blank lines split it, and Markdown needs a header row per block, so
/// nine of the twenty-one rows — the whole recent half, from 문자열 입력 onward — rendered on
/// GitHub as literal `| pipe | text |` paragraphs. The canonical index was unreadable at
/// exactly the point a reader looks for the newest work.
///
/// Two invariants, because the fragmentation is invisible in a diff and the drift is invisible
/// in the rendering:
///   - no blank line may sit BETWEEN two table rows (that is what splits a table);
///   - every `SPEC-*.md` must be linked from the index.
#[test]
fn the_slice_index_is_one_table_and_lists_every_spec() {
    let index = read("docs/slices/README.md");
    let lines: Vec<&str> = index.lines().collect();
    for (i, w) in lines.windows(3).enumerate() {
        assert!(
            !(w[0].starts_with('|') && w[1].trim().is_empty() && w[2].starts_with('|')),
            "docs/slices/README.md:{} is a blank line between two table rows, which splits the \
             table — every row after it renders as literal pipe text, because a Markdown table \
             block needs its own header row",
            i + 2
        );
    }

    let dir = repo_root().join("docs").join("slices");
    let mut specs: Vec<String> = std::fs::read_dir(&dir)
        .expect("read docs/slices")
        .filter_map(|e| {
            let n = e.ok()?.file_name().to_string_lossy().into_owned();
            (n.starts_with("SPEC-") && n.ends_with(".md")).then_some(n)
        })
        .collect();
    specs.sort();
    assert!(
        specs.len() >= 20,
        "expected the whole family, got {specs:?}"
    );
    for s in &specs {
        assert!(
            index.contains(&format!("({s})")),
            "docs/slices/README.md does not link {s}. The index is what CLAUDE.md calls the \
             canonical list of closed slices, so a SPEC missing from it is a slice nobody can \
             find"
        );
    }
}

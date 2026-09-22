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

    // STATUS.md was NOT in this list until 2026-09-11, and the gap did exactly what a gap
    // does: it said "로드하는 모듈 15개" while `host.c` loaded 18, and went on saying it
    // through the slice that changed the number. The other two documents were corrected the
    // same day BY THIS GUARD; the one outside it simply kept lying.
    //
    // That is the shape worth remembering: a guard's SCOPE is a claim too. This one asserted
    // "the documents agree with host.c" while checking two of the three that make the claim.
    //
    // One consequence, learned immediately: this cannot tell a CLAIM from a QUOTATION of an
    // old claim. Writing "it used to say 로드하는 모듈 15개" in one of these files fails the
    // guard — which happened while documenting this very fix. Teaching it about blockquotes
    // would trade a loud, obvious failure for a silent exemption, so the rule is the other
    // way round: when recounting a superseded number in these files, do not spell it with
    // this prefix.
    for doc in ["docs/SECURITY.md", "docs/HOST_ABI.md", "docs/STATUS.md"] {
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
    // One of the three carries the module's name, so what protection.rs pins is
    // `ml_iface_hash_{module}` and what a document can state is `ml_iface_hash_<module>` —
    // the same two spellings the reserved-symbol guard above reconciles, through the same
    // one translation.
    let pinned: Vec<String> = string_literals_in_vec_after(&protection, "        exports,")
        .iter()
        .map(|s| placeholder_to_module(s))
        .collect();
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

/// **No document calls the Delphi arm unbuilt while two gates build it.**
///
/// `CONTRIBUTING.md` is the only document an outside contributor reads to set a machine up,
/// and one paragraph of it made three claims that were all false for sixteen days:
///
///   - *"The generated `.pas` has never been compiled by anything"* — `MATHLESS_GATE_FPC`
///     compiles every generated unit under `-Mdelphi -Sew` on every push, and **the same file
///     says so twenty lines above.** A document that contradicts itself is worse than one that
///     is merely stale: the reader cannot tell which half to act on.
///   - *"D14's Delphi arm is BLOCKED"* — `MATHLESS_GATE_DELPHI` passes on the dev machine
///     (§9-20); what is left is CI, which is a different claim.
///   - *"the unit ships marked DRAFT"* — `grep -rn DRAFT compiler/ runtime/ hosts/` is **0**.
///     The banner was rewritten twice and the document followed neither time.
///
/// Conditional on the source, the same shape as the ABI-refusal guard above: the three facts
/// are asserted first, so if a gate is removed this test says the gate is gone rather than
/// quietly excusing every document that denies it.
///
/// **Two halves, and the reason is the one Grok gave for the ABI-refusal guard.** A denylist
/// catches only the sentences that were there — a paraphrase walks through it. So
/// `CONTRIBUTING.md` must also NAME both gate variables. That is deliberately a weaker
/// positive than the ABI guard's sentence match: the shape of the true statement here changes
/// every time the Delphi arm moves, and a guard pinned to today's phrasing would go red for a
/// correction. The variable names are what a contributor actually needs to type.
///
/// **One cost, met immediately and left in place.** This repository records what a corrected
/// sentence used to say, and the first draft of the correction QUOTED the three false claims —
/// which turned the guard red on the very commit that fixed them. The sibling candidate guard
/// solves the same collision by skipping blockquote lines; that was rejected here, because a
/// denial hidden in a blockquote is exactly what this guard is for and `CONTRIBUTING.md` uses
/// blockquotes for notes. So the correction notes in that file DESCRIBE the old sentences
/// instead of quoting them, and say so. If you are writing the next correction, do the same.
#[test]
fn no_document_says_the_generated_unit_is_unbuilt_while_the_gates_build_it() {
    let fpc = read("hosts/rust-oracle/tests/fpc_units.rs");
    let delphi = read("hosts/rust-oracle/tests/delphi_host.rs");
    let header = read("compiler/src/header.rs");

    assert!(
        fpc.contains("GATE_FPC_OK"),
        "fpc_units.rs no longer prints GATE_FPC_OK — the gate that compiles every generated \
         unit is gone, or this test matches the wrong line"
    );
    assert!(
        delphi.contains("fn bds_exe"),
        "delphi_host.rs no longer has the bds.exe fallback — MATHLESS_GATE_DELPHI can no \
         longer build through the IDE, or this test matches the wrong line"
    );
    assert!(
        !header.contains("DRAFT"),
        "header.rs emits DRAFT again. This guard exists because the documents kept saying the \
         unit ships marked DRAFT after it stopped doing so; if it starts again, the denylist \
         below is wrong, not the documents"
    );

    for (path, text) in every_markdown_file() {
        let flat = flatten_prose(&text);
        for stale in [
            "has never been compiled by anything",
            "무엇에도 컴파일된 적이 없",
            "Delphi arm is BLOCKED",
            "Delphi 쪽은 BLOCKED",
            "ships marked DRAFT",
            "DRAFT로 나간다",
            "needs an edition whose dcc64 compiles from a command line",
            "명령줄 컴파일이 되는 에디션을 필요로 한다",
        ] {
            assert!(
                !flat.contains(stale),
                "{path} says '{stale}', but MATHLESS_GATE_FPC compiles every generated unit, \
                 MATHLESS_GATE_DELPHI builds through bds.exe when dcc64 refuses, and nothing \
                 in the tree emits DRAFT"
            );
        }
    }

    let contributing = read("CONTRIBUTING.md");
    for gate in ["MATHLESS_GATE_FPC", "MATHLESS_GATE_DELPHI"] {
        assert!(
            contributing.contains(gate),
            "CONTRIBUTING.md no longer names {gate}. Dropping the false sentence is not \
             enough — the one document a new contributor reads has to say which gate covers \
             the Delphi arm, or the absence reads as absence of the gate"
        );
    }
}

/// **The READMEs name every built-in the compiler has, and every gate CI requires.**
///
/// Both counts were wrong in the same direction: the public first screen claimed LESS than the
/// product does. `README.md` said *"There are four built-ins — floor, ceil, round, trunc"* while
/// `typeck.rs` also declares `len`, `byte_len`, `byte_slice` and `fixed`; it said *"Three host
/// paths, all measured"* and named the Rust oracle and the two C hosts, while `ci.yml` has
/// required an Object Pascal host that loads x64 modules and calls them since 2026-09-09.
///
/// Understating is not the harmless direction. A reader deciding whether this can reach their
/// host counts the hosts, and half of D14's flagship arm was missing from the count.
///
/// **Both halves are derived from the source, not from a list here.** The built-ins come from
/// `Rounder::ALL` plus the `*_BUILTIN` constants in `typeck.rs`; the gates come from the
/// `MATHLESS_GATE_*: require` lines in `ci.yml`. Adding either without telling users turns
/// this red, which is the case that actually happened four times.
///
/// Names, not counts. A count is one number to update and nothing to check it against — the
/// §1 LOC block is what that looks like after three months. A name is checkable against the
/// thing that defines it.
#[test]
fn the_readmes_name_every_builtin_and_every_required_gate() {
    let typeck = read("compiler/src/typeck.rs");
    let ci = read(".github/workflows/ci.yml");
    let en = read("README.md");
    let ko = read("README.ko.md");

    let mut builtins: Vec<String> = Vec::new();
    for line in typeck.lines() {
        let t = line.trim();
        if let Some(rest) = t.strip_prefix("pub const ") {
            if let Some((name, tail)) = rest.split_once(": &str = ") {
                if name.ends_with("_BUILTIN") {
                    builtins.push(
                        tail.trim()
                            .trim_end_matches(';')
                            .trim_matches('"')
                            .to_string(),
                    );
                }
            }
        }
        // The rounders are spelled as match arms rather than constants.
        if let Some(rest) = t.strip_prefix("Rounder::") {
            if let Some((_, name)) = rest.split_once("=> ") {
                builtins.push(
                    name.trim()
                        .trim_end_matches(',')
                        .trim_matches('"')
                        .to_string(),
                );
            }
        }
    }
    builtins.sort();
    builtins.dedup();
    assert!(
        builtins.len() >= 8,
        "only {} built-ins parsed out of typeck.rs ({builtins:?}) — fewer than the eight known \
         to exist, so this guard would pass by checking almost nothing",
        builtins.len()
    );

    let mut gates: Vec<String> = Vec::new();
    for line in ci.lines() {
        let t = line.trim();
        let Some((name, value)) = t.split_once(':') else {
            continue;
        };
        if name.starts_with("MATHLESS_GATE_") && value.trim() == "require" {
            gates.push(name.to_string());
        }
    }
    gates.sort();
    gates.dedup();
    assert!(
        gates.len() >= 3,
        "only {} required gates parsed out of ci.yml ({gates:?}) — fewer than the three known \
         to be required",
        gates.len()
    );

    for (doc, text) in [("README.md", &en), ("README.ko.md", &ko)] {
        for b in &builtins {
            assert!(
                text.contains(b.as_str()),
                "{doc} does not name the built-in `{b}`, which compiler/src/typeck.rs declares. \
                 The READMEs are where a reader learns what the language has; a built-in that \
                 ships without appearing here is one nobody can find"
            );
        }
        for g in &gates {
            assert!(
                text.contains(g.as_str()),
                "{doc} does not name {g}, which .github/workflows/ci.yml sets to `require`. \
                 A host path CI proves on every push, missing from the page that says which \
                 host paths are proven, understates the product"
            );
        }
    }
}

/// **Every PR number the slice index cites is a PR that exists.**
///
/// `CLAUDE.md` calls `docs/slices/README.md` the canonical list of closed slices, and the last
/// column of every row is how a reader gets from a slice to the change that made it. One of
/// them pointed at **#97, which has never existed in this repository** — `try` was implemented
/// by #98 and #99, and the SPEC's own body says so. The wrong number had spread to three more
/// documents.
///
/// Derived from git, not from a list: the merged-commit subjects carry `(#N)`, so the set of
/// real PR numbers is `git log`'s to give. A citation outside that set is a dead pointer, and
/// nothing else in this file could see it — every other guard compares documents to source
/// code, and a PR number is neither.
///
/// Numbers are compared as a SET, not as a range. #97 sits between two numbers that do exist,
/// so any bounds check would have passed it.
#[test]
fn every_pr_the_slice_index_cites_exists() {
    let out = std::process::Command::new("git")
        .args(["log", "--format=%s", "--all"])
        .current_dir(repo_root())
        .output()
        .expect("run git log");
    assert!(out.status.success(), "git log failed");
    let subjects = String::from_utf8_lossy(&out.stdout);

    let mut real: Vec<u32> = Vec::new();
    for (idx, _) in subjects.match_indices("(#") {
        let digits: String = subjects[idx + 2..]
            .chars()
            .take_while(char::is_ascii_digit)
            .collect();
        if subjects[idx + 2 + digits.len()..].starts_with(')') {
            if let Ok(n) = digits.parse() {
                real.push(n);
            }
        }
    }
    real.sort_unstable();
    real.dedup();
    assert!(
        real.len() >= 200,
        "only {} merged PR numbers parsed out of git log — this guard would pass by having \
         nothing to compare against",
        real.len()
    );

    let index = read("docs/slices/README.md");
    for line in index.lines() {
        if !line.starts_with("| [") || !line.contains("](SPEC-") {
            continue;
        }
        for (idx, _) in line.match_indices('#') {
            let digits: String = line[idx + 1..]
                .chars()
                .take_while(char::is_ascii_digit)
                .collect();
            if digits.is_empty() {
                continue;
            }
            let n: u32 = digits.parse().expect("ascii digits");
            assert!(
                real.contains(&n),
                "docs/slices/README.md cites #{n}, which is not among the {} PR numbers in \
                 git log. The last column is how a reader gets from a slice to the change \
                 that closed it; a number that never existed sends them nowhere",
                real.len()
            );
        }
    }
}

/// **A message a host README quotes is a message that host can print.**
///
/// `hosts/c-host-link/README.md` quoted the refusal as `refuse: interface …`. The host prints
/// `refuse discount: interface …` — the module name is in there, and it is in there for a
/// reason: that host links TWO modules and the whole point of #215 was that it can now name
/// which one drifted. The README was quoting the message from before that slice.
///
/// A phantom transcript is worse than a stale sentence. A reader who greps the source for the
/// line they were shown finds nothing and concludes the check is not there.
///
/// Scope and needle are both measured. Spans are taken from backticks and filtered to the ones
/// that look like program output — `refuse…`, `GATE_…`, `ok …` — and checked against the
/// host's own source AND its harness, because the gate banners are printed by the test, not by
/// the host. Checking the host source alone reported six false misses; adding the harness left
/// exactly one, and that one was the defect.
///
/// The probe stops at the first `…`, so a README may elide the varying half of a format
/// string without defeating the check.
#[test]
fn every_message_a_host_readme_quotes_exists_in_that_host() {
    let harness_dir = repo_root().join("hosts").join("rust-oracle").join("tests");
    for (dir, sources) in [
        ("c-host", vec!["hosts/c-host/host.c", "c_host.rs"]),
        (
            "c-host-link",
            vec!["hosts/c-host-link/host.c", "c_host.rs", "link_host.rs"],
        ),
        (
            "delphi-host",
            vec![
                "hosts/delphi-host/host.dpr",
                "delphi_host.rs",
                "fpc_units.rs",
            ],
        ),
    ] {
        // `.github/workflows/ci.yml` is in every blob because a host README legitimately
        // quotes the gate setting that drives it (`MATHLESS_GATE_FPC_HOST: require`), and
        // ci.yml is where CLAUDE.md says that list is canonical. It adds no risk of a false
        // pass: the one real defect below is a `refuse …` string, which appears in no yml.
        let mut blob = read(".github/workflows/ci.yml");
        for s in &sources {
            let p = if s.contains('/') {
                repo_root().join(s)
            } else {
                harness_dir.join(s)
            };
            if let Ok(t) = std::fs::read_to_string(&p) {
                blob.push_str(&t);
            }
        }
        assert!(
            blob.len() > 1000,
            "hosts/{dir}: read almost nothing from {sources:?}, so this guard would pass by \
             having no text to search"
        );

        let readme = read(&format!("hosts/{dir}/README.md"));
        let mut checked = 0usize;
        for span in readme.split('`').skip(1).step_by(2) {
            if span.contains('\n') {
                continue;
            }
            let looks_printed =
                span.starts_with("refuse") || span.contains("GATE_") || span.starts_with("ok ");
            if !looks_printed {
                continue;
            }
            let probe = span.split('…').next().unwrap_or(span).trim();
            if probe.is_empty() {
                continue;
            }
            checked += 1;
            assert!(
                blob.contains(probe),
                "hosts/{dir}/README.md quotes `{span}`, but no such text appears in {sources:?}. \
                 A reader who greps for the line they were shown finds nothing and concludes \
                 the check is absent"
            );
        }
        assert!(
            checked >= 2,
            "hosts/{dir}/README.md: only {checked} quoted messages matched the filter, fewer \
             than the two known to be there — the filter stopped seeing them"
        );
    }
}

/// **The language reference does not deny a feature it goes on to document.**
///
/// All three defects this guards were the same shape, and all three were refuted by
/// `docs/LANGUAGE.md` itself, ten to twenty-five lines further down:
///
///   - the type line called `string` parameter-position-only while the file documents
///     `-> string!` return, and omitted arrays entirely;
///   - the `try` item limited callees to internal `fn` while the item ten lines below says
///     export can be called with `try` since #101;
///   - the literal item said ASCII-only while the file documents `return "승인"` from
///     2026-09-14. The real constraint was never the character set — it is the position.
///
/// A reference that contradicts itself is worse than one that is merely behind: the reader
/// cannot tell which half is current, and the wrong half is the one that says "you cannot".
///
/// Each pair is a denial and the evidence that refutes it, both in this one file. Keyed on
/// the evidence so the guard is conditional: if `-> string!` return were withdrawn, the
/// denial would be true again and this stops demanding it.
#[test]
fn the_language_reference_does_not_deny_what_it_documents() {
    let lang = flatten_prose(&read("docs/LANGUAGE.md"));
    for (evidence, denial, what) in [
        ("-> string! 반환", "파라미터 위치 전용", "string return"),
        (
            "export도 try로 부를 수 있다",
            "v1의 피호출자는 내부 fn만",
            "try on exports",
        ),
        (
            "return \"승인\"",
            "문자열 리터럴 \"KR\" — ASCII만",
            "non-ASCII literals",
        ),
    ] {
        if !lang.contains(evidence) {
            continue;
        }
        assert!(
            !lang.contains(denial),
            "docs/LANGUAGE.md documents {what} (it contains `{evidence}`) and also denies it \
             (`{denial}`). A reference that contradicts itself leaves the reader unable to \
             tell which half is current, and the wrong half is the one that says no"
        );
    }
}

/// **A shipped SPEC does not still ask for the confirmation it already got.**
///
/// Six SPECs carried a §4 heading reading *미확정, 사용자 확인 필요* while their own line 3
/// said 확정 · 구현 완료. That is not a cosmetic mismatch: `CLAUDE.md` rule 8 makes "find the
/// DP items awaiting user confirmation" a procedure, and an agent following it reads these
/// headings and goes asking for a confirmation that was given — in one case over three weeks
/// earlier, for a feature that has been in the compiler since.
///
/// Wholly derivable from inside each file: the status line says whether the slice shipped, and
/// the DP heading says whether its decisions are open. Those two cannot both be true.
///
/// The DP TABLES are left alone. They are the record of what was decided and why, and this
/// guard says nothing about them — only about a heading that describes them as pending.
#[test]
fn a_shipped_spec_does_not_still_request_confirmation() {
    let dir = repo_root().join("docs").join("slices");
    let mut checked = 0usize;
    for entry in std::fs::read_dir(&dir).expect("read docs/slices") {
        let path = entry.expect("a directory entry").path();
        let name = path
            .file_name()
            .expect("a file name")
            .to_string_lossy()
            .into_owned();
        if !name.starts_with("SPEC-") || !name.ends_with(".md") {
            continue;
        }
        let text = std::fs::read_to_string(&path).expect("read a SPEC");
        let shipped = text
            .lines()
            .take(8)
            .any(|l| l.contains("상태:") && l.contains("구현 완료"));
        if !shipped {
            continue;
        }
        checked += 1;
        // Four spellings, and each one was found by widening after the previous round looked
        // finished. `사용자 확인 필요` caught six headings; the shorter match caught six more
        // written `권고, **사용자 확인 필요**`; Grok then found two BODY sentences saying the
        // same thing as `사용자 확인이 필요` and `사용자 확인 전까지`; and the fourth is a
        // correction note of my own from #261 that quoted the phrase it was correcting.
        for pending in [
            "사용자 확인 필요",
            "사용자 확인이 필요",
            "사용자 확인 전까지",
            "확인 전이며",
        ] {
            assert!(
                !text.contains(pending),
                "docs/slices/{name} says 구현 완료 in its status line and still says \
                 `{pending}`. CLAUDE.md rule 8 makes finding open DP items a procedure, so that \
                 sentence sends the next agent to ask for a confirmation already given"
            );
        }
    }
    assert!(
        checked >= 20,
        "only {checked} shipped SPECs were examined, fewer than the twenty known to exist — \
         the status-line match stopped working and this guard would pass by skipping everything"
    );
}

/// **"Run what that job runs" runs what that job runs.**
///
/// `CONTRIBUTING.md` tells a contributor to reproduce CI locally and then gives a command that
/// sets ONE gate. CI requires three. On a machine without Free Pascal the other two **skip**,
/// the suite prints green and exits 0, the contributor pushes, and CI goes red — after review
/// time has already been spent.
///
/// The block itself is the only thing that made this invisible: a skipped gate is loud on
/// stdout and silent in the exit code, which is the exact failure mode the paragraph
/// underneath that block warns about for pipes. It warned about the wrapper and not about
/// itself.
///
/// Derived from `ci.yml`, so it cannot drift the way a hand-copied list does: every
/// `MATHLESS_GATE_*: require` there must appear in the code fence under the "run what that job
/// runs" heading. Adding a fourth required gate to CI without telling contributors turns this
/// red.
///
/// `MATHLESS_GATE_DELPHI` is deliberately NOT implied by this: it is not `require` in ci.yml,
/// because the runner has no Delphi. The guard asks only for parity with what CI actually
/// enforces.
#[test]
fn the_local_command_block_runs_every_gate_ci_requires() {
    let ci = read(".github/workflows/ci.yml");
    let mut required: Vec<String> = Vec::new();
    for line in ci.lines() {
        let t = line.trim();
        let Some((name, value)) = t.split_once(':') else {
            continue;
        };
        if name.starts_with("MATHLESS_GATE_") && value.trim() == "require" {
            required.push(name.to_string());
        }
    }
    required.sort();
    required.dedup();
    assert!(
        required.len() >= 3,
        "only {} required gates parsed out of ci.yml ({required:?}) — fewer than the three \
         known to be required, so this guard would demand almost nothing",
        required.len()
    );

    let contributing = read("CONTRIBUTING.md");
    let fences: Vec<&str> = contributing.split("```").skip(1).step_by(2).collect();
    let blocks: Vec<&&str> = fences
        .iter()
        .filter(|f| f.contains("cargo test --workspace"))
        .collect();
    assert!(
        !blocks.is_empty(),
        "CONTRIBUTING.md has no fenced block running `cargo test --workspace` — the guard can \
         no longer find the command it checks"
    );

    for block in &blocks {
        for gate in &required {
            assert!(
                block.contains(gate.as_str()),
                "a CONTRIBUTING.md command block runs `cargo test --workspace` without setting \
                 {gate}, which .github/workflows/ci.yml requires. On a machine missing that \
                 toolchain the gate SKIPS, the suite exits 0, and CI is the one that says no"
            );
        }
    }
}

/// **No document states a Rust version that is not the pinned one.**
///
/// The global rule puts dependency versions in the build file and nowhere else. This
/// repository copies the pin into five documents instead — `CONTRIBUTING.md`, both READMEs,
/// `docs/STATUS.md`, `docs/phase1/WBS.md` — and nothing compared any of them to
/// `rust-toolchain.toml`. All five are right today, which is the whole problem: raising the
/// pin makes five documents false at once and a person is the only thing that would notice.
///
/// Same shape as the abi-constant guard: the source states the value, the documents may
/// repeat it, and repeating it wrongly fails. That is the form the global rule allows — "put
/// the number in a machine-checkable form or do not put it in a document".
///
/// Matches any `1.<minor>.<patch>` so a document cannot escape by being stale in a way the
/// needle does not expect; versions that are not Rust releases live in other shapes here
/// (`ml-iface/1`, SDK `10.0.20348`) and do not match.
#[test]
fn no_document_states_a_rust_version_other_than_the_pin() {
    let toml = read("rust-toolchain.toml");
    let pin = toml
        .lines()
        .find_map(|l| {
            let t = l.trim();
            let rest = t.strip_prefix("channel")?.trim_start().strip_prefix('=')?;
            Some(rest.trim().trim_matches('"').to_string())
        })
        .expect("rust-toolchain.toml states a channel");
    assert!(
        pin.starts_with("1."),
        "rust-toolchain.toml's channel is `{pin}`, which is not a numbered release — this \
         guard compares documents against a version number and has nothing to compare"
    );

    let mut seen = 0usize;
    for (path, text) in every_markdown_file() {
        if path == "docs/HISTORY.md" {
            continue;
        }
        let bytes: Vec<char> = text.chars().collect();
        for (idx, _) in text.match_indices("1.") {
            let tail: String = text[idx..].chars().take(12).collect();
            let mut parts = tail.splitn(3, '.');
            let (Some(_), Some(minor), Some(rest)) = (parts.next(), parts.next(), parts.next())
            else {
                continue;
            };
            let patch: String = rest.chars().take_while(char::is_ascii_digit).collect();
            if minor.len() < 2 || !minor.chars().all(|c| c.is_ascii_digit()) || patch.is_empty() {
                continue;
            }
            // A digit immediately before means this is the tail of a longer number.
            let before = text[..idx].chars().next_back();
            if before.is_some_and(|c| c.is_ascii_digit() || c == '.') {
                continue;
            }
            let found = format!("1.{minor}.{patch}");
            if !(found.starts_with("1.8") || found.starts_with("1.9")) {
                continue;
            }
            seen += 1;
            assert_eq!(
                found, pin,
                "{path} states Rust {found}, but rust-toolchain.toml pins {pin}. The pin is \
                 the source; a document may repeat it, and repeating it wrongly is what this \
                 catches"
            );
        }
        let _ = &bytes;
    }
    assert!(
        seen >= 5,
        "only {seen} Rust version mentions were found across the documents, fewer than the \
         five known to exist — the scan stopped seeing them and would pass by checking nothing"
    );
}

/// **The glossary defines the vocabulary the documents actually use.**
///
/// `README.md` calls `docs/GLOSSARY.md` *"the terms this repository uses precisely"*. It
/// defined nine, and none of them was a term the repository actually leans on. Measured across
/// the 45 documents under `docs/` excluding the history file: `DP-` appears 585 times,
/// 슬라이스 392, 수용 237, 지문 177, Q12 163, 게이트 162, `mlx_` 132, 오라클 130, E2 104. The
/// glossary defined **zero** of them, and had not been touched since 2026-08-30.
///
/// That is not a document being wrong. It is a document being absent while claiming to be
/// present, which no other guard here can see: every one of them checks a claim against a
/// source, and this file's failure was that it made no claims.
///
/// **An explicit list is right here, unlike everywhere else in this file.** The audit's
/// standing complaint is hand-written scopes, and it holds when the list is a SAMPLE of
/// something derivable. Here the list IS the deliverable — "these words need defining" is the
/// contract, not an approximation of one. The numbers above are the measurement behind it and
/// live in this comment, not in the document.
///
/// Only that the term is defined, never how. A definition that drifts is a separate problem,
/// and the document answers it by pointing at an owner instead of restating one — which is the
/// shape Grok proposed when it named the risk of extending this file at all: process
/// definitions copied in become the stale second copy.
#[test]
fn the_glossary_defines_the_vocabulary_the_docs_use() {
    let glossary = read("docs/GLOSSARY.md");
    for term in [
        "슬라이스",
        "SPEC",
        "DP-",
        "수용",
        "게이트",
        "오라클",
        "코퍼스",
        "골든",
        "E0",
        "E1",
        "E2",
        "지문",
        "매니페스트",
        "드리프트",
        "Q12",
        "caller-allocates",
        "프록시",
        "mlx_",
    ] {
        assert!(
            glossary.contains(term),
            "docs/GLOSSARY.md does not define `{term}`, which the documents under docs/ use \
             heavily. README calls this file the place where this repository's terms are \
             precise; a term it omits is one a reader has to infer from usage"
        );
    }
}

/// **A slice decided against is not "not done yet".**
///
/// The index marks `SPEC-symbol-embedded-hash.md` ⛔ 하지 않는다 — DP-H3(b) was measured,
/// prototyped against real DLLs, and REJECTED on 2026-09-05 (#141) because it makes a
/// conscientious host receive a silent `0xC0000139` where it gets a readable refusal today.
/// `CLAUDE.md` rule 3 says not to reopen it before two recorded conditions hold.
///
/// Two live documents still described it as merely pending — `아직 하지 않았다`. That is a
/// different claim from "decided against", and it is the one that gets picked up: a session
/// reading "not done yet" in `OPEN_QUESTIONS.md` has no reason to look for a rejection.
///
/// The sibling guard covers ✅ rows becoming candidates. This is the ⛔ case, which that guard
/// cannot see because a rejected slice never appears in the candidate list — it appears as an
/// open question.
///
/// **Honest limit**: the needle is two phrasings, so a reworded "not done yet" escapes it. The
/// alternative was a positive pin requiring every mention to carry the rejection, and that
/// fires on `SPEC-error-prefix.md`, which legitimately recorded DP-H3(b) as open on 2026-09-03,
/// two days before the decision. `docs/slices/` and `docs/HISTORY.md` are exempt for that
/// reason: they are records of what was true when written.
#[test]
fn a_rejected_slice_is_not_described_as_merely_pending() {
    let index = read("docs/slices/README.md");
    assert!(
        index.contains("⛔") && index.contains("SPEC-symbol-embedded-hash.md"),
        "docs/slices/README.md no longer marks SPEC-symbol-embedded-hash.md ⛔. If the decision \
         was reopened, this guard is wrong — but CLAUDE.md rule 3 names the conditions"
    );

    for (path, text) in every_markdown_file() {
        if path == "docs/HISTORY.md" || path.starts_with("docs/slices/") {
            continue;
        }
        let flat = flatten_prose(&text);
        let chars: Vec<char> = flat.chars().collect();
        let nd: Vec<char> = "DP-H3(b)".chars().collect();
        for start in 0..chars.len().saturating_sub(nd.len()) {
            if chars[start..start + nd.len()] != nd[..] {
                continue;
            }
            let hi = (start + nd.len() + 90).min(chars.len());
            let window: String = chars[start..hi].iter().collect();
            for pending in ["아직 하지 않았", "아직 안 했"] {
                assert!(
                    !window.contains(pending),
                    "{path} describes DP-H3(b) as pending, but docs/slices/README.md marks it \
                     ⛔ 하지 않는다 (#141, 2026-09-05). \"Not done yet\" invites a session to do \
                     it; \"decided against\" sends them to the reasons. Context: …{window}…"
                );
            }
        }
    }
}

/// **No document calls interface metadata unimplemented while every module ships a fingerprint.**
///
/// `ARCHITECTURE.md`'s Packaging step said *"인터페이스 메타는 ⏳ 미구현"*. Every module has
/// exported `ml_iface_hash_<module>()` since #105, every generated header pins
/// `ML_<MODULE>_IFACE_HASH`, and both reference C hosts refuse a module whose fingerprint
/// disagrees. The same file, 33 lines below, *relies* on the fingerprint existing — it explains
/// that a same-signature swap gets past it. One document, two answers.
///
/// Scoped to the phrase 인터페이스 메타, which occurs in exactly one place in the tree. That is
/// the measurement this guard rests on rather than a judgement: there is no legitimate second
/// user of the phrase to carve out, so a future one is worth a red.
#[test]
fn no_document_calls_interface_metadata_unimplemented() {
    let iface = read("compiler/src/iface.rs");
    assert!(
        iface.contains("ml-iface/1"),
        "compiler/src/iface.rs no longer builds the ml-iface/1 manifest. If the fingerprint \
         was withdrawn, this guard is wrong and ARCHITECTURE.md would be right"
    );

    for (path, text) in every_markdown_file() {
        if path == "docs/HISTORY.md" {
            continue;
        }
        let flat = flatten_prose(&text);
        let chars: Vec<char> = flat.chars().collect();
        let nd: Vec<char> = "인터페이스 메타".chars().collect();
        for start in 0..chars.len().saturating_sub(nd.len()) {
            if chars[start..start + nd.len()] != nd[..] {
                continue;
            }
            let hi = (start + nd.len() + 40).min(chars.len());
            let window: String = chars[start..hi].iter().collect();
            assert!(
                !(window.contains("미구현") || window.contains('⏳')),
                "{path} calls 인터페이스 메타 unimplemented, but compiler/src/iface.rs builds the \
                 ml-iface/1 manifest, every module exports ml_iface_hash_<module>, and both \
                 reference C hosts refuse a drifted one. Context: …{window}…"
            );
        }
    }
}

/// **No document says C is the only host with an automated gate.**
///
/// It stopped being true on 2026-09-09, when `MATHLESS_GATE_FPC_HOST: require` put an Object
/// Pascal host that loads x64 modules and calls them into CI. `CLAUDE.md`, `DECISIONS.md` and
/// `STATUS.md` followed; `ROADMAP.md` and `COMPETITIVE.md` did not, and both are in the
/// reading order a new session is told to take.
///
/// **The needle needs a word boundary, and finding out why is the point.** `C뿐` matches inside
/// `FPC뿐` — so the first draft of this guard flagged `CLAUDE.md` and `DECISIONS.md` for saying
/// the CORRECT thing, *"CI 게이트는 C·FPC뿐"*. A guard that fires on the true sentence is one
/// somebody deletes (§9-A A6 is the same shape from the other direction: a needle with no
/// polarity passing the false sentence). Requiring the preceding character not to be an ASCII
/// letter separates them, and that separation was measured on all 53 documents before this
/// was written, not assumed.
///
/// **One exemption, `docs/HISTORY.md`**, because it is the history file: its tables record
/// which documents carried this claim and when. Exempting it is not a loophole for a live
/// document, because a live document is not in it.
#[test]
fn no_document_says_c_is_the_only_gated_host() {
    let ci = read(".github/workflows/ci.yml");
    assert!(
        ci.contains("MATHLESS_GATE_FPC_HOST: require"),
        ".github/workflows/ci.yml no longer requires MATHLESS_GATE_FPC_HOST. If the Pascal \
         host gate was withdrawn, this guard is wrong and the documents would be right"
    );

    for (path, text) in every_markdown_file() {
        if path == "docs/HISTORY.md" {
            continue;
        }
        let flat: String = flatten_prose(&text);
        let chars: Vec<char> = flat.chars().collect();
        for needle in ["C뿐", "C 쪽만", "only C"] {
            let nd: Vec<char> = needle.chars().collect();
            for start in 0..chars.len().saturating_sub(nd.len()) {
                if chars[start..start + nd.len()] != nd[..] {
                    continue;
                }
                // `FPC뿐` contains `C뿐`. Only a standalone C is a claim about the C host.
                if start > 0 && chars[start - 1].is_ascii_alphabetic() {
                    continue;
                }
                let lo = start.saturating_sub(70);
                let hi = (start + nd.len() + 70).min(chars.len());
                let window: String = chars[lo..hi].iter().collect();
                if !(window.contains("게이트") || window.to_lowercase().contains("gate")) {
                    continue;
                }
                panic!(
                    "{path} says the gated host is `{needle}`, but ci.yml requires \
                     MATHLESS_GATE_FPC_HOST — an Object Pascal host loads x64 modules and \
                     calls them on every push. Context: …{window}…"
                );
            }
        }
    }
}

/// **No document calls array return unimplemented while codegen emits it.**
///
/// `CLAUDE.md` names `docs/HOST_ABI.md`'s "현재 구현된 경계" as the canonical statement of what
/// the ABI covers. That section said 배열 반환 was ⏳ 미구현 while the same file, 139 lines
/// below, headed a section *"문자열 반환·배열 반환 모두 구현·실측 완료"* — the canonical source
/// contradicting itself, with the wrong half first. A second copy sat in the D16 ownership
/// section and sent the reader to *"아래 '가변 길이 데이터' 절"*, which is the section that
/// refutes it.
///
/// Proximity, not a phrase list. The two occurrences were spelled differently
/// (`배열 반환·구조체·콜백·호스트 함수 등록은 ⏳ 미구현` and `배열 반환과 문자열을 품은 struct는
/// 여전히 ⏳ 미구현`), so a denylist would have caught one and let the other through — the exact
/// failure §9-A A6 records. This looks for `배열 반환` within 45 characters of 미구현 or ⏳ in
/// flattened prose, which both spellings trip and a third would too.
///
/// **No exemption, and that is measured.** Across all 53 documents the pattern matched exactly
/// twice, both in `HOST_ABI.md`, and both were the defect. `docs/` is not carved out here the
/// way it is for the bare fingerprint name, because there is nothing under `docs/` that
/// legitimately says this — the SPECs and the history record the feature as SHIPPED.
#[test]
fn no_document_calls_array_return_unimplemented() {
    let codegen = read("compiler/src/codegen.rs");
    assert!(
        codegen.contains("RetAbi::ArrayOut"),
        "codegen.rs no longer emits RetAbi::ArrayOut. If array return was withdrawn, this \
         guard is wrong and the documents would be right"
    );

    const WINDOW: usize = 45;
    for (path, text) in every_markdown_file() {
        let flat = flatten_prose(&text);
        let chars: Vec<char> = flat.chars().collect();
        let needle: Vec<char> = "배열 반환".chars().collect();
        for start in 0..chars.len().saturating_sub(needle.len()) {
            if chars[start..start + needle.len()] != needle[..] {
                continue;
            }
            let lo = start.saturating_sub(WINDOW);
            let hi = (start + needle.len() + WINDOW).min(chars.len());
            let window: String = chars[lo..hi].iter().collect();
            assert!(
                !(window.contains("미구현") || window.contains('⏳')),
                "{path} calls 배열 반환 unimplemented, but compiler/src/codegen.rs emits \
                 RetAbi::ArrayOut and SPEC-array-return closed acceptance A~H on 2026-09-12. \
                 Context: …{window}…"
            );
        }
    }
}

/// **No live document teaches a symbol no module exports.**
///
/// The fingerprint export was renamed to `ml_iface_hash_<module>` by
/// `SPEC-qualified-iface-hash` (#215), because every module exporting the same bare name meant
/// a linked host could check at most one of them. `runtime/README.md` kept listing the export
/// as a bare `ml_iface_hash()` in two places, and `hosts/c-host-link/README.md` kept telling
/// hosts to call it. A host author who copies that name gets NULL from `GetProcAddress` and
/// falls into the un-checked state `SPEC-iface-hash` §5.1 warns about — quietly, because a
/// skipped fingerprint check looks exactly like a passing one.
///
/// The hand-written contract header one directory over was already right: `runtime/ml_abi.h`
/// declares `ml_iface_hash_<module>` and even records that it *"was a bare `ml_iface_hash`
/// until SPEC-qualified-iface-hash"*. **One folder taught two different symbol names.**
///
/// **Scope is derived, not hand-listed**, which is the complaint the 2026-09-22 audit made
/// about every other guard here. The split already exists in the tree: `docs/` holds design
/// records and dated history — `SPEC-iface-hash.md` opens with a banner saying its body is a
/// record of the old name, `SPEC-qualified-iface-hash.md` quotes the bare name as the DEFECT
/// it measured, and `STATUS.md` §7-4 and §9-31 record the same. Rewriting any of those would
/// be rewriting history. Every `.md` OUTSIDE `docs/` is live product-facing prose, and there
/// the bare name is simply wrong.
///
/// Matching `ml_iface_hash(` with the paren attached is what separates the two names: the
/// qualified form is `ml_iface_hash_discount(`, which does not contain it.
#[test]
fn no_live_document_names_the_bare_fingerprint_export() {
    let codegen = read("compiler/src/codegen.rs");
    assert!(
        codegen.contains("ml_iface_hash_{}"),
        "codegen.rs no longer emits a module-qualified fingerprint export. If the bare name \
         came back, this guard is wrong and the documents are right — check \
         SPEC-qualified-iface-hash.md before deleting it"
    );

    for (path, text) in every_markdown_file() {
        let rel = path.replace('\\', "/");
        if rel.starts_with("docs/") {
            continue;
        }
        assert!(
            !text.contains("ml_iface_hash("),
            "{path} names a bare `ml_iface_hash(`, but every module exports \
             `ml_iface_hash_<module>` (compiler/src/codegen.rs). A host author copying this \
             name gets NULL from GetProcAddress and skips the fingerprint check, which looks \
             the same as passing it. Files under docs/ are exempt because they record the \
             rename; this file is not a record."
        );
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
    let pas = mlc::header::emit_delphi_unit(&ir, "widget");

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
///
/// **One of the two is a PATTERN, not a declaration.** Since `SPEC-qualified-iface-hash` the
/// fingerprint export is `ml_iface_hash_<module>`, so the emitter's literal carries a format
/// placeholder and `runtime/ml_abi.h` cannot declare the symbol at all — the name is not
/// fixed. It documents the shape instead, and [`placeholder_to_module`] is the single
/// translation between the two spellings. That is a real narrowing of what this can prove:
/// for that symbol it now checks that the file DESCRIBES the export, not that it declares
/// it. Both were only ever textual — nothing compiles `ml_abi.h` — so what is lost is the
/// declaration's shape, and what is kept is the name and the signature.
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
        let want = placeholder_to_module(decl);
        assert!(
            abi_h.contains(&want),
            "compiler/src/header.rs emits '{decl}' into every generated header, but \
             runtime/ml_abi.h does not carry '{want}'. That file is the hand-written list of \
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
    //
    // The first half used to grep `header.rs` for the literal `#define
    // ML_ST_INSUFFICIENT_BUFFER (-1)`. That broke the day the value moved into
    // `abi::ML_ST_INSUFFICIENT_BUFFER` and the emitter started interpolating it: the guard
    // failed while the emitted header was unchanged and MORE correct than before. Section 7
    // says it plainly -- guard the artifact, not the code that writes it. So this calls the
    // emitter and reads what comes out.
    let define = format!(
        "#define ML_ST_INSUFFICIENT_BUFFER ({})",
        mlc::abi::ML_ST_INSUFFICIENT_BUFFER
    );
    let define = define.as_str();
    let emitted = {
        let ir = mlc::compile_to_ir("export fn f(s: string) -> string! { return s }")
            .expect("a string return is what makes the header emit the guard");
        mlc::header::emit_c_header(&ir, "probe")
    };
    assert!(
        emitted.contains(define),
        "a generated header no longer defines '{define}'. The truncation status is the one \
         negative both bindings must agree on, so if it changed value, runtime/ml_abi.h has \
         to change with it"
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
        // `CLAUDE.md` joined 2026-09-22. It states the artifact set TWICE and is the file
        // every session loads, so a fifth artifact would have turned six documents red and
        // left this one quietly wrong — the blind-spot shape §7-3 names.
        "CLAUDE.md",
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
    //
    // **This one reads the SOURCE, and that is the shape STATUS §7 says is wrong** ("guard the
    // artifact, not the code that makes it" — got wrong three times on `header.rs`). Measured
    // 2026-09-16: turn the `.dll` line into `let _unused = format!(…)` and the module's own path
    // disappears from what a user reads, while the text below is still present and this stays
    // green. It is kept because it is cheap and runs without MSVC, but the check that actually
    // binds is `emit_robustness::the_success_output_names_every_file_the_build_wrote`, which
    // runs the binary and reads its stdout. A test here cannot: `CARGO_BIN_EXE_mlc` exists only
    // for the crate that declares the binary, and this is a different crate.
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
        .find("(Delphi: ")
        .expect("the line `mlc` prints for the .pas no longer carries a (Delphi: …) note");
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
    //
    // `CONTRIBUTING.md` joined on 2026-09-22 for the same reason, found the same way. It
    // publishes both values four times AND told the reader that "the four documents that
    // publish it" are guarded — while being an unguarded fifth. A list that names its own
    // scope is the one place a missing entry reads as a promise.
    for doc in [
        "docs/SECURITY.md",
        "docs/STATUS.md",
        "README.md",
        "README.ko.md",
        "CONTRIBUTING.md",
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
    // Both fingerprints, not one. This host links two modules, and until
    // SPEC-qualified-iface-hash they exported the same `ml_iface_hash`, so "the host calls
    // the fingerprint" was satisfied while `schedule` was called with no interface check at
    // all — the linker chose which module the single call reached (STATUS §7-4). Naming both
    // is what makes this guard say "every linked module is gated" rather than "a gate exists".
    for expected in [
        "mlx_discount(",
        "ml_iface_hash_discount()",
        "ml_iface_hash_schedule()",
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
/// `"uint64_t ml_iface_hash_{dll_name}(void);"` → `"uint64_t ml_iface_hash_<module>(void);"`.
///
/// The reserved declarations above are recovered from `header.rs`'s own string literals, and
/// one of them now spells the module's name into the symbol. Where a generated header has a
/// real name, the emitter's literal has a format placeholder and `runtime/ml_abi.h` has the
/// pattern `<module>` — because a file that serves every module cannot pick one. This is the
/// only place those three spellings are reconciled.
///
/// Anything between `{` and `}` becomes `<module>`, so a positional `{}` reads the same as a
/// named `{dll_name}`. An unclosed `{` stops the rewrite and leaves the rest verbatim, which
/// makes the assertion fail loudly rather than quietly matching less.
fn placeholder_to_module(decl: &str) -> String {
    let mut out = String::with_capacity(decl.len());
    let mut rest = decl;
    while let Some(open) = rest.find('{') {
        let Some(close) = rest[open..].find('}') else {
            break;
        };
        out.push_str(&rest[..open]);
        out.push_str("<module>");
        rest = &rest[open + close + 1..];
    }
    out.push_str(rest);
    out
}

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

/// The hand-written C and C++ sources must be pure ASCII, because MSVC reads them in the
/// machine's ANSI code page and `/W4 /WX` turns "not representable there" into an error.
///
/// Found by writing an em-dash into a `host.c` comment. On this development machine (code
/// page 949) the acceptance-D gate failed immediately:
///
/// ```text
/// host.c(1): warning C4819: <character not representable in code page 949>
/// host.c(1): error C2220: warning treated as error
/// ```
///
/// The reason this is a test and not just "the build caught it" is that the build would NOT
/// have caught it everywhere. An em-dash *is* representable in CP1252, so on a runner with
/// that code page the same file compiles clean — the failure is a property of the reader, not
/// of the file, which is exactly the shape that reaches one developer and not the next. It is
/// the same argument the generated headers already make for identifiers (`reserved.rs`), and
/// the hand-written hosts had no equivalent.
///
/// Text only, so the ubuntu insurance job runs it too — which is the point: it needs no MSVC.
#[test]
fn the_hand_written_c_sources_are_pure_ascii() {
    let root = repo_root();
    // Discovered, not listed: a hand-written host added next year is covered without anyone
    // remembering this file. Generated artifacts are not here — they live in temp dirs, and
    // `compiler/tests/emit.rs` already asserts the header is ASCII.
    let mut sources: Vec<(String, PathBuf)> = Vec::new();
    for dir in ["hosts/c-host", "hosts/c-host-link", "runtime"] {
        let Ok(entries) = std::fs::read_dir(root.join(dir)) else {
            continue;
        };
        for e in entries.flatten() {
            let p = e.path();
            let ext = p.extension().and_then(|x| x.to_str()).unwrap_or("");
            if matches!(ext, "c" | "h" | "cpp" | "hpp") {
                let name = p.file_name().and_then(|x| x.to_str()).unwrap_or("?");
                sources.push((format!("{dir}/{name}"), p));
            }
        }
    }
    let mut checked = 0;
    for (rel, path) in &sources {
        let bytes = std::fs::read(path).unwrap_or_else(|e| panic!("read {rel}: {e}"));
        checked += 1;
        if let Some(i) = bytes.iter().position(|b| !b.is_ascii()) {
            let line = 1 + bytes[..i].iter().filter(|b| **b == b'\n').count();
            panic!(
                "{rel}:{line} holds the non-ASCII byte 0x{:02X}. MSVC reads this file in the \
                 machine's ANSI code page, so under `/W4 /WX` it is C4819 -> C2220 on any code \
                 page that cannot represent it — a build that fails for one developer and not \
                 the next. Write it in ASCII.",
                bytes[i]
            );
        }
    }
    assert!(
        checked >= 3,
        "expected at least the two C hosts and the ABI header; found {checked} ({:?}). A guard \
         that scans an empty set passes forever — if these moved, point it at where they went",
        sources.iter().map(|(r, _)| r).collect::<Vec<_>>()
    );
}

/// **The reference dynamic host must be able to gate every module name the compiler accepts.**
///
/// Two numbers in two languages that have to agree: `ML_MAX_MODULE_NAME` in
/// `compiler/src/abi.rs`, and the `#define` of the same name in `hosts/c-host/host.c` that
/// sizes the buffer `gate()` builds `ml_iface_hash_<module>` into. C cannot read the Rust
/// constant, so the host carries a copy — and a copy that nothing checks is how the two drift.
///
/// They HAD drifted, in the only direction that matters: the host's buffer was an
/// independently chosen `80`, which gated 65 characters and refused at 66, while the compiler
/// had no bound at all and `mlc build` produced a 70-character module at exit 0. The
/// compiler shipped modules its own reference host could not gate
/// (`SPEC-module-name-length` §0.1).
///
/// This reads both sources rather than running anything, so unlike the acceptance-D gates it
/// needs no MSVC and runs on **both** CI jobs — which is the point, because the Windows job
/// is the only one that would otherwise notice, and only if a long name were in the corpus.
#[test]
fn the_c_host_can_gate_every_module_name_the_compiler_accepts() {
    let abi = read("compiler/src/abi.rs");
    let compiler_bound: usize =
        numbers_between(&abi, "pub const ML_MAX_MODULE_NAME: usize = ", ";")
            .first()
            .copied()
            .unwrap_or_else(|| {
                panic!(
                    "compiler/src/abi.rs no longer declares ML_MAX_MODULE_NAME — either put it \
                 back or drop this guard, but do not leave the host's copy unchecked"
                )
            });

    let host = read("hosts/c-host/host.c");
    // Empty suffix on purpose: the number ends the line, and this working tree checks C
    // sources out with CRLF, so a `"\n"` suffix matched nothing and the guard failed for a
    // reason that had nothing to do with the bound.
    let host_bound: usize = numbers_between(&host, "#define ML_MAX_MODULE_NAME ", "")
        .first()
        .copied()
        .unwrap_or_else(|| {
            panic!(
                "hosts/c-host/host.c no longer defines ML_MAX_MODULE_NAME — gate() sizes its \
                 symbol buffer from it"
            )
        });

    assert!(
        host_bound >= compiler_bound,
        "the C host's ML_MAX_MODULE_NAME is {host_bound} but the compiler accepts names up \
         to {compiler_bound}. A module with a name between the two compiles, links and \
         loads, and is then REFUSED by the reference dynamic host with `module name too \
         long to form its fingerprint symbol` — far from the cause. The compiler sets the \
         bound; the host follows it"
    );

    // And the buffer really is derived from that define rather than a number beside it.
    assert!(
        host.contains("char symbol[sizeof \"ml_iface_hash_\" + ML_MAX_MODULE_NAME]"),
        "hosts/c-host/host.c must size gate()'s buffer from ML_MAX_MODULE_NAME and the \
         prefix, or this guard checks a constant nothing uses"
    );
}

/// **No test creates a temp directory by hand — `common::TempOut` is the only way.**
///
/// This has recurred twice. `STATUS.md` §5-5.7 measured the first version (clear at the
/// START, never at the end: 1,720 trees / 976 MB on one development machine), and #219
/// answered it with `TempOut`, whose Drop also runs when a test panics and whose removal
/// retries past the Windows DLL-lock race. Then a NEW file copied the older helper it had
/// replaced, and eight trees were left behind in a single afternoon (§9-44.3).
///
/// Twice means the helper existing is not enough, because nothing made a new file use it.
/// This is what makes it stick. Measured before it was written: every leftover tree in the
/// working directory — `mlc_w6` ×7, `mlc_gco_*` ×6, `mlc_arr_*`, `mlc_calls`, `mlc_str_*`,
/// `mlc_amb` — belonged to a file still building its path by hand, and none to a converted one.
///
/// The needle is `create_dir_all`, not `temp_dir()`: a test may legitimately NAME a temp path
/// it never creates (`diagnostics.rs` builds one only to watch the compiler refuse before
/// anything is written). Creating is what leaves something behind.
#[test]
fn no_test_creates_a_temp_directory_by_hand() {
    // The needles are ASSEMBLED, not written. Spelled literally they appear in this file — in
    // the search itself — and the guard reported its own source as an offender. Exempting
    // this file by name would have worked and would have been worse: an exemption hides a
    // real offender the day someone adds one here. Same problem the gated-module guard above
    // records ("it cannot tell a CLAIM from a QUOTATION"), answered the same way, by making
    // the text not match rather than by carving out a hole.
    let creates = format!("fs::{}", "create_dir_all");
    let names_temp = format!("env::{}()", "temp_dir");
    // Assembled for the same reason, and it bit the same way: written literally, this file
    // counted as a THIRD definition of the helper.
    let defines_helper = format!("pub struct {}", "TempOut");
    let drops_it = format!("impl {} for", "Drop");
    let removes = format!("remove_{}", "dir_all");

    let root = repo_root();
    let mut offenders = Vec::new();
    // RECURSIVE, and that is not incidental: `common/` is itself a subdirectory, so a
    // top-level-only walk would miss a leaky helper copied into exactly the place a helper
    // most plausibly goes. (Review named this; it was a hole, not a boundary.)
    // `src/bin` is in scope, and that was measured the hard way: `mlprobe` landed in the same
    // session as this guard, created its scratch tree by hand, and left 27 of them — in the
    // one directory a `tests/`-only walk does not reach. A developer-run binary is the same
    // kind of thing as a test for this purpose. Library code stages its own builds with its
    // own documented cleanup (`emit.rs`), which is a different question this does not answer.
    let mut stack: Vec<PathBuf> = [
        "hosts/rust-oracle/tests",
        "compiler/tests",
        "compiler/src/bin",
    ]
    .iter()
    .map(|d| root.join(d))
    .collect();
    let mut seen_files = 0usize;
    let mut helpers = 0usize;
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            panic!(
                "{} is missing — this guard would pass by checking nothing",
                dir.display()
            );
        };
        for entry in entries {
            let path = entry.expect("a directory entry").path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            if path.extension().and_then(|e| e.to_str()) != Some("rs") {
                continue;
            }
            seen_files += 1;
            let text = std::fs::read_to_string(&path).expect("read a test file");
            // The file that DEFINES the helper is the one place a create belongs. Recognised
            // by that definition rather than by its path, so the exemption is a property and
            // not a name: a file stops being exempt the moment it stops being the helper.
            if text.contains(&defines_helper) {
                helpers += 1;
                continue;
            }
            // A file that removes its own tree in a `Drop` has answered the question. Checked
            // as a property, like the helper exemption above: a binary cannot import the test
            // helper, so it has to carry its own, and what matters is that cleanup survives an
            // unwind — not which type provides it.
            // …and the Drop must actually REMOVE something. `impl Drop for` alone is too
            // loose: a file with an unrelated Drop would be exempt without cleaning anything
            // up. Review named that; it costs one more substring to close.
            if text.contains(&drops_it) && text.contains(&removes) {
                continue;
            }
            for (n, line) in text.lines().enumerate() {
                // A create on a path the same file derived from the system temp directory.
                if line.contains(&creates) && text.contains(&names_temp) {
                    let rel = path
                        .strip_prefix(&root)
                        .expect("a path under the root")
                        .to_string_lossy()
                        .replace('\\', "/");
                    offenders.push(format!("{rel}:{}", n + 1));
                }
            }
        }
    }
    assert!(
        seen_files >= 30,
        "only {seen_files} test files were read — the walk is checking almost nothing"
    );
    assert_eq!(
        helpers, 2,
        "expected exactly the two byte-identical copies of tests/common/mod.rs to define \
         `TempOut`, found {helpers} — a third definition would be a third exemption, and \
         nothing else asked for one"
    );
    assert!(
        offenders.is_empty(),
        "these build a temp directory by hand instead of holding `common::TempOut`, so the \
         tree survives whenever the test panics — which is exactly when you are iterating \
         (STATUS §5-5.7, §9-44.3):\n  {}\n\nUse `common::TempOut::new(\"<tag>\")` and drop the \
         manual cleanup; bind it as `_out` if nothing reads it, never `_`, which would delete \
         the tree while the module is still loaded.",
        offenders.join("\n  ")
    );
}

#[allow(dead_code)]
fn file_name(p: &Path) -> String {
    p.file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned()
}

/// Every integer constant `compiler/src/abi.rs` declares, as `(name, value)`.
///
/// Parsed rather than imported on purpose. `mlc::abi::ML_MAX_MODULE_NAME` would give the
/// value, but not the NAME as it is spelled — and the spelling is what a document writes.
fn abi_constants() -> Vec<(String, i64)> {
    let src = read("compiler/src/abi.rs");
    let mut out = Vec::new();
    for line in src.lines() {
        let Some(rest) = line.trim_start().strip_prefix("pub const ") else {
            continue;
        };
        let Some((name, rest)) = rest.split_once(':') else {
            continue;
        };
        let Some((_ty, value)) = rest.split_once('=') else {
            continue;
        };
        if let Ok(v) = value.trim().trim_end_matches(';').trim().parse::<i64>() {
            out.push((name.trim().to_string(), v));
        }
    }
    out
}

fn is_ident(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

/// Every value this line assigns to `name`, in the shapes markdown actually uses:
/// `NAME = -2`, `` `NAME = -2` ``, `**NAME = 64**`, `NAME (-1)`-style parentheses.
///
/// Numbers are returned, not spellings, because `-122` and `-1` share a prefix — a substring
/// search would have called the rejected alternative in `SPEC-string-return` §T6 a match for
/// the chosen value and passed a line it had not actually checked.
fn assigned_on_line(line: &str, name: &str) -> Vec<i64> {
    let mut out = Vec::new();
    let mut from = 0;
    while let Some(i) = line[from..].find(name) {
        let start = from + i;
        let end = start + name.len();
        from = end;

        // A longer identifier that merely CONTAINS this one is a different constant.
        if line[..start].chars().next_back().is_some_and(is_ident) {
            continue;
        }
        let after = &line[end..];
        if after.chars().next().is_some_and(is_ident) {
            continue;
        }

        // Markdown puts decoration between the name and the value; step over it.
        //
        // Two shapes carry a value, and the docstring above has promised both since this
        // function was written — `NAME = -2` and the parenthesised `NAME (-1)`. Only the first
        // was implemented: `(` was stripped AFTER `=` had been consumed, so a line with
        // parentheses and no `=` fell out of the loop. Measured 2026-09-22: of 22 lines
        // mentioning ML_ST_INSUFFICIENT_BUFFER only 4 were evaluated, and two of the skipped
        // ones are the Q12 status table in `HOST_ABI.md` — the shipped contract. If `-1` ever
        // changed, the document a third-party host author reads would go false with nothing
        // red.
        //
        // The parenthesised form requires `=` or a backtick before the digits, and that is not
        // fussiness. `NAME (2026-09-14 실측)` is a date, and accepting a bare number inside
        // parentheses would read it as the value. Measured across the tree: 5 lines carry a
        // real value that way and every one has `=` or a backtick; the single bare `(-1)` is in
        // `HISTORY.md`, where a number in parentheses may well be a value this repository no
        // longer uses. Leaving that one unparsed is the safe direction.
        // The backtick has to be looked for in the span actually consumed between `(` and the
        // digits -- NOT in the tail of the line. Grok caught the first version doing the
        // latter, which made any backtick anywhere later on the line qualify, so
        // `NAME (2026-09-14 실측)` followed by any code span would have been read as 2026. Both
        // plants used to check this change happened to be the shape that worked.
        let dec = after.trim_start_matches([' ', '`', '*']);
        let rest = match dec.strip_prefix('(') {
            Some(open) => {
                let inner = open.trim_start_matches([' ', '`', '*']);
                let consumed = &open[..open.len() - inner.len()];
                match inner.strip_prefix('=') {
                    Some(r) => r,
                    None if consumed.contains('`') => inner,
                    None => continue,
                }
            }
            None => match dec.strip_prefix('=') {
                Some(r) => r,
                None => continue,
            },
        };
        let rest = rest.trim_start_matches([' ', '`', '*', '(']);
        let neg = rest.starts_with('-');
        let digits: String = rest[usize::from(neg)..]
            .chars()
            .take_while(|c| c.is_ascii_digit())
            .collect();
        if digits.is_empty() {
            continue;
        }
        let v: i64 = digits.parse().expect("ascii digits");
        out.push(if neg { -v } else { v });
    }
    out
}

/// Every `.md` in the working tree, as `(path relative to the root, contents)`.
///
/// Walked rather than listed, because the needle is self-identifying: a constant's name is
/// its own evidence that the line is making a claim about it, so there is no document a walk
/// could wrongly include. Where the needle is NOT self-identifying — `.dll` appears in prose
/// that is not enumerating artifacts — a walk would be the wrong instrument and the guard
/// names its documents instead.
///
/// (`the_slice_index_is_one_table_and_lists_every_spec` already walks `docs/slices/`, so this
/// is not the first guard here to find its own scope. An earlier draft of this comment said
/// it was.)
fn every_markdown_file() -> Vec<(String, String)> {
    every_file_ending_in(&[".md"])
}

/// The walk itself, so that two scopes cannot drift into two different walks.
///
/// A second copy of this loop is the same defect shape as a second copy of the slice-index
/// parser: when one learns to skip a directory and the other does not, the difference shows up
/// as a guard that quietly reads less than it claims.
fn every_file_ending_in(suffixes: &[&str]) -> Vec<(String, String)> {
    let root = repo_root();
    let mut out = Vec::new();
    let mut stack = vec![root.clone()];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).expect("read a directory") {
            let entry = entry.expect("a directory entry");
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().into_owned();
            if path.is_dir() {
                if name != "target" && name != ".git" {
                    stack.push(path);
                }
            } else if suffixes.iter().any(|s| name.ends_with(s)) {
                let rel = path
                    .strip_prefix(&root)
                    .expect("a path under the root")
                    .to_string_lossy()
                    .replace('\\', "/");
                out.push((
                    rel,
                    std::fs::read_to_string(&path).expect("read a document"),
                ));
            }
        }
    }
    out.sort();
    out
}

/// **Every `§9-N` this repository cites resolves to a heading in `docs/HISTORY.md`.**
///
/// `docs/STATUS.md` grew a dated session log — `### 9-1.` through `### 9-62.` — inside the
/// document whose own line 10 promises it holds 현재와 다음만. It reached 3,774 lines, 62% of
/// the file, in a file that line 3 tells every new session to read FIRST. The global rule
/// sends completed-work narrative to a history document; this repository had none, so it
/// accumulated where it could.
///
/// **This guard is what makes moving it safe.** Those entries are not inert: 46 files outside
/// `STATUS.md` cite `§9-N` 150 times, and six of them are product source comments —
/// `compiler/src/codegen.rs`, `header.rs`, `iface.rs`, `ir.rs`, `lexer.rs`, `lib.rs`. A move
/// that dropped, renumbered or mangled one entry would strand those citations silently,
/// because nothing about a prose reference fails at compile time.
///
/// Derived, not listed: collect every `§9-<n>` mentioned anywhere in the tree, collect every
/// `### 9-<n>.` heading in `docs/HISTORY.md`, and require the first set inside the second.
/// The relation was measured to hold BEFORE the move (36 distinct numbers cited, 62 headings,
/// numbering 1–62 with no gaps, 0 dangling), so a failure here means the move broke something
/// rather than that the repository was already broken.
#[test]
fn every_cited_history_entry_has_a_heading() {
    let history = read("docs/HISTORY.md");

    let mut headings: Vec<u32> = Vec::new();
    for line in history.lines() {
        let Some(rest) = line.strip_prefix("### 9-") else {
            continue;
        };
        let Some((num, _)) = rest.split_once('.') else {
            continue;
        };
        if let Ok(n) = num.parse::<u32>() {
            headings.push(n);
        }
    }
    assert!(
        headings.len() >= 62,
        "docs/HISTORY.md carries {} `### 9-N.` headings, fewer than the 62 that existed in \
         docs/STATUS.md before the move. Entries were lost, not moved",
        headings.len()
    );

    let mut cited: Vec<(String, u32)> = Vec::new();
    for (path, text) in every_file_ending_in(&[".md", ".rs", ".c", ".h", ".dpr", ".mls", ".yml"]) {
        for (idx, _) in text.match_indices("§9-") {
            let digits: String = text[idx + "§9-".len()..]
                .chars()
                .take_while(char::is_ascii_digit)
                .collect();
            if let Ok(n) = digits.parse::<u32>() {
                cited.push((path.clone(), n));
            }
        }
    }
    assert!(
        cited.len() >= 150,
        "only {} `§9-N` citations found across the tree, fewer than the 150 measured before \
         the move — this scan is reading less than it did, so it would pass by finding nothing",
        cited.len()
    );

    for (path, n) in &cited {
        assert!(
            headings.contains(n),
            "{path} cites §9-{n}, but docs/HISTORY.md has no `### 9-{n}.` heading. Moving the \
             log out of docs/STATUS.md must preserve every heading exactly: a prose reference \
             does not fail at compile time, so a stranded one is silent"
        );
    }
}

/// **No document states a value for an `abi.rs` constant that `abi.rs` does not state.**
///
/// This file's own header says the repository watched a measured number drift three times
/// and that until it existed nothing checked the numbers. It still did not check THESE
/// numbers, and the hole was measured rather than argued: lowering `ML_MAX_MODULE_NAME`
/// from 64 to 48 left the whole workspace — 516 tests, four gates required — green, while
/// three documents went on saying 64 in the present tense, one of them in the very table
/// cell that reads "**문서에 숫자를 적지 않는다**".
///
/// The rule the global instructions give is "put the number in a machine-checkable form or
/// do not put it in a document". This is that form: the document may carry the number, and
/// the source decides whether it is still true.
///
/// **A line that also states the true value passes.** A decision table that shows a rejected
/// alternative beside the chosen one (`SPEC-string-return` §T6 weighs `-122` against `-1`)
/// is not drift — it is the record of a choice, and erasing it would cost more than it saves.
#[test]
fn no_document_states_a_stale_value_for_an_abi_constant() {
    let constants = abi_constants();
    assert!(
        constants.len() >= 4,
        "compiler/src/abi.rs declared {} integer constants — it had four, so either the \
         parse broke or the constants moved, and in both cases this guard is now checking \
         nothing",
        constants.len()
    );

    let mut stale = Vec::new();
    for (path, text) in every_markdown_file() {
        for (n, line) in text.lines().enumerate() {
            for (name, value) in &constants {
                let found = assigned_on_line(line, name);
                // The line states the truth somewhere on it; anything else beside it is a
                // rejected alternative, not a stale claim.
                if found.is_empty() || found.contains(value) {
                    continue;
                }
                for wrong in found {
                    stale.push(format!(
                        "{path}:{} says `{name} = {wrong}` — compiler/src/abi.rs says {value}",
                        n + 1
                    ));
                }
            }
        }
    }

    assert!(
        stale.is_empty(),
        "a document states a value the source no longer holds. The source is \
         compiler/src/abi.rs; fix the document, or drop the number and let the source say \
         it:\n{}",
        stale.join("\n")
    );
}

/// **A document that names the output-licence exception lists everything the exception
/// covers.**
///
/// `every_artifact_the_emitter_writes_is_named_in_the_docs` already checks this, against a
/// hand-written list of six files — and its own comment says why that list matters: D23
/// enumerated four artifacts while `LICENSE-OUTPUT-EXCEPTION` §1 listed five, "a grant that
/// under-lists what it grants is the worst place for this drift", and it sat outside the
/// guard until 2026-09-05.
///
/// The identical sentence sat ONE DOCUMENT OVER and was not in the list. `OPEN_QUESTIONS.md`
/// summarised the same grant as `.dll`·`.h`·`.pas`·중간 Rust — four again, no `.lib` —
/// because the summary was written on 2026-09-02 and `.lib` arrived on 09-03. Measured, not
/// supposed: seven documents name the exception, six listed `.lib`, one did not.
///
/// So this guard takes the scope the OTHER one cannot: naming `LICENSE-OUTPUT-EXCEPTION` is
/// a document volunteering that it describes the grant, which makes the set discoverable
/// instead of remembered. The artifacts themselves come from the licence, not from here — it
/// is the document that grants the rights, so it is the document that says what they cover.
#[test]
fn every_document_that_cites_the_output_licence_lists_what_it_covers() {
    // The `(`.ext`)` spellings inside the licence's own enumeration, in its order.
    let licence = read("LICENSE-OUTPUT-EXCEPTION");
    let covered: Vec<String> = licence
        .lines()
        .filter_map(|l| {
            let l = l.trim();
            let rest = l.strip_prefix("- the ")?;
            let open = rest.find("(`.")?;
            let close = rest[open..].find("`)")?;
            Some(rest[open + 2..open + close].to_string())
        })
        .collect();
    assert_eq!(
        covered.len(),
        4,
        "recovered {covered:?} from LICENSE-OUTPUT-EXCEPTION §1 — it enumerated four \
         extensions plus the intermediate Rust, which has no extension to cite. If the \
         licence changed shape, teach this guard the new one rather than dropping it"
    );

    for (path, text) in every_markdown_file() {
        if !text.contains("LICENSE-OUTPUT-EXCEPTION") {
            continue;
        }
        for ext in &covered {
            assert!(
                text.contains(&format!("`{ext}`")),
                "{path} cites LICENSE-OUTPUT-EXCEPTION but never mentions `{ext}`, which the \
                 licence grants rights over. A summary that under-lists a grant reads as a \
                 narrower grant — the licence covers {covered:?}"
            );
        }
    }
}

/// The slice index's closed rows: `| [<title>](SPEC-….md) | ✅ … |`. The link text is the title.
///
/// Two guards read this list, and they have to read it the SAME way. When the index format
/// moves, one parser following and the other not is worse than neither following: the stale
/// one returns an empty list and its assertions pass by iterating over nothing. That is the
/// `closed.len()` floor below, and it belongs to the parser, not to either caller.
///
/// Every 2-or-more-word PREFIX of a title comes back too, and that is the hole this guard had
/// been green through since it was written. The index writes a title in full —
/// `배열 **입력** 파라미터` — and a candidate list writes the short name a person would use,
/// `배열 입력`. The short name does not contain the long one, so the match fails. Measured:
/// **24 of the 29 closed titles are three words or more**, so most of them are evadable this
/// way, and the guard only ever caught `배열 반환` because that title happens to BE its own
/// short name.
///
/// The failure is not hypothetical — it is the one this repository already wrote down. The
/// 2026-09-11 correction in `docs/slices/README.md` records that the candidate list's first
/// item was `배열 IN`, and the guard built to stop that recurrence could not see it. Planting
/// `배열 입력` as a candidate left the suite at 31 green before this change.
///
/// 74 prefixes across 29 titles, and zero of them match the live candidate list or `CLAUDE.md`
/// today — measured before the change, so the widening is strictly a widening.
///
/// Titles come back through `flatten_prose`, and **both callers must flatten their haystack
/// the same way**. Stripping `**` from the needle alone is not enough, and that was measured
/// rather than reasoned: with `title.replace("**", "")` against raw text, planting
/// `다음은 배열 **반환**이다.` in CLAUDE.md left the guard GREEN — the emphasis sits inside the
/// phrase, so the contiguous substring is never there. Grok raised it while verifying the
/// commit that introduced this helper, after a red-then-green run had already looked complete.
/// Flattening also collapses newlines, so a title split across a line wrap stops hiding too.
fn closed_slice_titles() -> Vec<String> {
    let readme = read("docs/slices/README.md");
    let mut closed: Vec<String> = Vec::new();
    for line in readme.lines() {
        let Some(rest) = line.strip_prefix("| [") else {
            continue;
        };
        let Some((title, tail)) = rest.split_once("](SPEC-") else {
            continue;
        };
        if !tail.contains('✅') {
            continue;
        }
        let full = flatten_prose(title);
        let words: Vec<&str> = full.split(' ').filter(|w| !w.is_empty()).collect();
        for n in 2..words.len() {
            closed.push(words[..n].join(" "));
        }
        closed.push(full);
    }
    closed.sort();
    closed.dedup();
    assert!(
        closed.len() >= 25,
        "the index rows stopped parsing — found {}, which is fewer than the slices that are \
         known to be closed, so a guard built on this would pass by reading nothing",
        closed.len()
    );
    closed
}

/// **`CLAUDE.md` does not name a closed slice.** It is the file every session loads.
///
/// `docs/slices/README.md` has corrected "a closed slice is still listed as a candidate"
/// three times, and next to the third it writes the cause: closing a slice adds a row to the
/// index — *`CLAUDE.md` puts that in the procedure* — and **nothing removes it from the
/// candidate list**. The file it names by name was the fourth occurrence. Rule 3 listed
/// `배열 반환` as a future candidate while the index row read ✅ 구현 완료 and the compiler had
/// been emitting `RetAbi::ArrayOut` since #206; rule 2, one line above it, forbids reopening a
/// closed slice.
///
/// The sibling guard below bounds its scan to a named section. **This one scans the whole
/// file, and the reason is measured, not stylistic**: of the 29 closed index titles, exactly
/// one appeared anywhere in `CLAUDE.md`, and it was the defect. With no legitimate mention to
/// carve out there is no sentence boundary to parse — and a whole-file scan cannot quietly
/// stop failing the way a literal section marker does when somebody renames a heading.
///
/// `docs/STATUS.md` and `docs/phase1/WBS.md` are deliberately NOT here. The same measurement
/// found 10 and 7 titles in them, and every one is a closure record — a `✅ 닫힘` section
/// heading or a completed WBS row. Pointing this guard at those files would make it a
/// false-positive machine, which is how a guard gets deleted.
///
/// If a rule ever genuinely needs to name a closed slice, this test failing is the right
/// outcome: it makes the author say so here, with a reason, instead of leaving a candidate.
#[test]
fn claude_md_does_not_name_a_closed_slice() {
    let claude = flatten_prose(&read("CLAUDE.md"));
    for title in closed_slice_titles() {
        assert!(
            !claude.contains(&title),
            "CLAUDE.md names `{title}`, which `docs/slices/README.md` marks ✅ 구현 완료. \
             Every session loads CLAUDE.md, so a closed slice named there is the one the next \
             session picks up — and rule 2 in that same file forbids reopening a closed \
             slice. The candidate list has one source, `docs/slices/README.md`; do not copy \
             it here."
        );
    }
}

/// **A SPEC the index calls closed says so in its own header.**
///
/// The index guard above opens `docs/slices/README.md` and `read_dir`s the SPEC names. It
/// never opens a SPEC. So the row and the file it links could disagree for as long as nobody
/// happened to read both, and they did: eleven SPECs the index marked ✅ 구현 완료 carried a
/// header saying 구현 대기 / 구현 중 / 구현 착수, one said *"구현은 시작하지 않았다"* about a
/// feature wired through every stage of the compiler, and three named no implementation state
/// at all.
///
/// `docs/slices/README.md` disclaims that each SPEC is "a design record at the time of
/// writing", and for the BODY that is right — a §5 that says "BLOCKED" is a fact about that
/// day and rewriting it would be rewriting history. **The `상태:` header is not body.** It is
/// the first thing in the file, it has no date attached to the implementation clause, and it
/// reads as the current state. Grok was asked whether the disclaimer covers it and answered
/// NOT-COVERED, for the reason the disclaimer itself gives: it immunises then-facts and sends
/// live status to `docs/STATUS.md`, never recasting the header as historical.
///
/// **Allowlist, not denylist**, and the difference is which way it breaks. A denylist of
/// 구현 대기 / 구현 중 / 구현 착수 would pass the three headers that name no state at all, and
/// would pass the next spelling somebody invents. Requiring the done-token means a SPEC that
/// invents a phrasing goes red and someone has to look. That is the direction this repository
/// has already chosen for every other guard it kept.
///
/// Bounded to the first 30 lines because that is where every header is today (30 files put it
/// on line 3; `SPEC-iface-hash.md` on line 24), and because the bound is what makes a DELETED
/// header fail. Searching the whole file would let a stray later 상태: line answer for a
/// header that is gone.
#[test]
fn a_closed_spec_says_so_in_its_own_header() {
    const DONE: &str = "구현 완료";
    // The done-token alone is a bare substring, and §9-A A6 is what that costs here: a
    // positive pin with no polarity passes the sentence that most needed to fail. Grok raised
    // it verifying this test — `구현 완료 예정` contains `구현 완료` and means the opposite, and
    // it is a phrasing somebody writing a SPEC before implementing it would plausibly reach
    // for. These are the forms that carry the token and negate it; `구현 대기`·`구현 중`·
    // `구현 착수` need no entry because they do not contain the token at all.
    const NOT_DONE: [&str; 4] = [
        "구현 완료 예정",
        "구현 완료 전",
        "구현 완료 아님",
        "구현 완료가 아니",
    ];
    let index = read("docs/slices/README.md");

    let mut offenders: Vec<String> = Vec::new();
    let mut checked = 0usize;
    for line in index.lines() {
        let Some(rest) = line.strip_prefix("| [") else {
            continue;
        };
        let Some((_, tail)) = rest.split_once("](") else {
            continue;
        };
        let Some((file, cells)) = tail.split_once(')') else {
            continue;
        };
        if !file.starts_with("SPEC-") || !file.ends_with(".md") || !cells.contains('✅') {
            continue;
        }
        checked += 1;

        let spec = read(&format!("docs/slices/{file}"));
        let header = spec.lines().take(30).find(|l| l.contains("상태:"));
        match header {
            None => offenders.push(format!(
                "{file}: the index marks it ✅ but its first 30 lines carry no `상태:` line at all"
            )),
            Some(h) if !h.contains(DONE) => {
                offenders.push(format!("{file}: index ✅, header says `{}`", h.trim()))
            }
            Some(h) if NOT_DONE.iter().any(|n| h.contains(n)) => offenders.push(format!(
                "{file}: index ✅, and the header carries `{DONE}` inside a phrase that negates \
                 it: `{}`",
                h.trim()
            )),
            Some(_) => {}
        }
    }

    assert!(
        checked >= 25,
        "the index rows stopped parsing — matched {checked} closed rows, fewer than the slices \
         known to be closed, so this guard would pass by reading nothing"
    );
    assert!(
        offenders.is_empty(),
        "{} SPEC header(s) contradict the index row that links them. The header is the first \
         thing a reader sees and carries no date on its implementation clause, so it reads as \
         the current state — the index's \"design record at the time of writing\" disclaimer \
         covers the body, not this line:\n  {}",
        offenders.len(),
        offenders.join("\n  ")
    );
}

/// **A slice cannot be both closed and a candidate**, in the file that says so about itself.
///
/// `docs/slices/README.md` carries two lists: an index of closed slices at the top and
/// "next slices (no SPEC yet)" at the bottom. Closing a slice adds a row to the first —
/// `CLAUDE.md` puts that in the procedure. **Nothing removes it from the second**, and that
/// file has corrected the resulting lie three times: 문자열 연결 (2026-09-03), 배열 입력
/// (2026-09-11), 배열 반환 (2026-09-15). The third one had been wrong for three days while the
/// row twelve lines above it read ✅ 구현 완료.
///
/// It is the shape `STATUS.md` §7-1 records as this repository's most repeated failure — a
/// document calling something open that the same document calls closed — and a human reading
/// the candidate list to pick the next slice is exactly who it misleads.
///
/// Matching is on the index row's OWN title text, stripped of `**`. That is what a copy into
/// the candidate list looks like, and it is what happened all three times.
#[test]
fn no_closed_slice_is_still_listed_as_a_candidate() {
    let readme = read("docs/slices/README.md");

    let (_, section) = readme
        .split_once("## 다음 슬라이스 (SPEC 미작성)")
        .expect("the candidate section's heading moved — this guard reads it by name");

    // Blockquote lines are excluded, and the first run of this guard is why: that section
    // opens with notes recording that a slice IS closed ("반올림 내장 함수도 닫혔다 — 위 색인,
    // #83"), and the three corrections name the very slices they removed. Those sentences are
    // the opposite of the defect. The candidate list is the prose that is not quoted.
    let candidates: String = flatten_prose(
        &section
            .lines()
            .filter(|l| !l.trim_start().starts_with('>'))
            .collect::<Vec<_>>()
            .join("\n"),
    );

    for title in &closed_slice_titles() {
        assert!(
            !candidates.contains(title.as_str()),
            "`{title}` is in the index as ✅ 구현 완료 AND in the candidate list below it. \
             That is the third-time failure the file documents about itself: closing a slice \
             adds the row above and nothing removes the line below."
        );
    }
}

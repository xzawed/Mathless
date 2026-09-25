//! Facts a document is required to carry, checked against the code that makes them true.

use super::*;

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

    // `CLAUDE.md` is checked for the GATES only — built-ins are the READMEs' job. It enumerates
    // the required gates in the same sentence that says the canonical list is `ci.yml` and must
    // not be copied here, which is a rule and its violation in one breath. The copy is worth
    // keeping (a session has to know which gates exist) so the fix is to make the copy
    // self-maintaining rather than to delete it: a fourth required gate turns this red.
    let claude = read("CLAUDE.md");
    for g in &gates {
        assert!(
            claude.contains(g.as_str()),
            "CLAUDE.md does not name {g}, which .github/workflows/ci.yml sets to `require`. \
             Every session loads CLAUDE.md and takes its gate list as the merge bar; a gate \
             missing from it is one nobody runs before pushing"
        );
    }
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
        // Dated records are not summaries of today's grant. Measured when the STATUS sections
        // moved to `docs/history/`: one hit, `status-4.md`'s §4-3, written 2026-09-02, the day
        // before `.lib` existed. It was true that day. STATUS.md itself still cites the licence
        // and still lists the full set, so it stays in.
        if is_dated_record(&path) {
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

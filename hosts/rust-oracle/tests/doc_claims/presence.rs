//! Facts a document is required to carry, checked against the code that makes them true.

use super::*;

/// **Wherever a live document states how many modules the reference C host gates, it is the
/// count in `host.c`.** No document has to state it: `host.c` is the home.
///
/// Why: the number lagged twice on record (13 against 14 after #108, 15 against 18 on
/// 2026-09-11), and the only repair three required copies allowed was editing all three. SPECs
/// and dated records are records of their moment and are not read.
#[test]
fn the_gated_module_count_in_the_docs_is_the_count_in_host_c() {
    // Comments stripped first: a `load(dir, "…")` written inside a comment would inflate the
    // count and make the documents "agree" with a number nothing loads.
    let host_c = strip_c_comments(&read("hosts/c-host/host.c"));
    let gated = host_c.matches("load(dir, \"").count();
    assert!(
        gated >= 10,
        "expected the C host to gate a corpus of modules, found {gated} — has the call \
         shape changed? This test recognises modules by `load(dir, \"`"
    );

    // It cannot tell a claim from a quotation of an old one: when recounting a superseded
    // number, do not spell it with this prefix.
    for (path, text) in every_markdown_file() {
        if is_dated_record(&path) || path.starts_with("docs/slices/SPEC-") {
            continue;
        }
        for n in numbers_between(&text, "로드하는 모듈 ", "개") {
            assert_eq!(
                n, gated,
                "{path} says the reference C host gates {n} modules; host.c gates {gated}. \
                 Write \"로드하는 모듈 전부\" instead of a number: host.c is where it lives"
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
                names_identifier(text, b),
                "{doc} does not name the built-in `{b}`, which compiler/src/typeck.rs declares. \
                 The READMEs are where a reader learns what the language has; a built-in that \
                 ships without appearing here is one nobody can find"
            );
        }
        for g in &gates {
            assert!(
                names_identifier(text, g),
                "{doc} does not name {g}, which .github/workflows/ci.yml sets to `require`. \
                 A host path CI proves on every push, missing from the page that says which \
                 host paths are proven, understates the product"
            );
        }
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

    // The HOMES of the artifact set, and only those: the READMEs tell a user what they get,
    // `HOST_ABI.md` is the contract a host author builds against, and `LICENSE-OUTPUT-EXCEPTION`
    // with D23 GRANT rights over the artifacts — a grant that under-lists what it grants is the
    // worst place for this drift. `STATUS.md` and `CLAUDE.md` were required copies until the
    // one-home pass; they now point here instead of restating the set.
    for doc in [
        "README.md",
        "README.ko.md",
        "docs/HOST_ABI.md",
        "LICENSE-OUTPUT-EXCEPTION",
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

/// **`docs/SECURITY.md` publishes both measured module sizes; a document that states one
/// states both.**
///
/// Why: the same commit and the same pinned rustc gave 9,728 B on the development machine and
/// 9,216 B on `windows-latest` (the pin covers rustc, not MSVC `link.exe`), so one value alone
/// is a this-machine number presented as a project fact. SECURITY is the home; the other four
/// documents used to be required to carry the pair and now point there. SPECs and dated
/// records are not read.
#[test]
fn the_published_module_size_carries_both_measurements() {
    let protection = read("hosts/rust-oracle/tests/protection.rs");
    let observed = [
        u64_const(&protection, "DISCOUNT_DLL_MEASURED_DEV"),
        u64_const(&protection, "DISCOUNT_DLL_MEASURED_CI"),
    ];
    let pair = observed.map(with_commas);

    let security = read("docs/SECURITY.md");
    for stated in &pair {
        assert!(
            security.contains(stated.as_str()),
            "docs/SECURITY.md does not state '{stated} B'. It is where the measured module \
             size lives, and it has to carry both: {} B on the development machine and {} B \
             on GitHub's windows-latest runner",
            observed[0],
            observed[1]
        );
    }

    // The documents that published the pair before the one-home pass. Stating it stays
    // allowed; stating half of it does not.
    for doc in [
        "docs/STATUS.md",
        "README.md",
        "README.ko.md",
        "CONTRIBUTING.md",
    ] {
        let text = read(doc);
        let has = pair.each_ref().map(|s| text.contains(s.as_str()));
        assert!(
            has[0] == has[1],
            "{doc} states one measured module size without the other ({} B dev, {} B CI). \
             Point to docs/SECURITY.md, or state both",
            observed[0],
            observed[1]
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

/// **A document that names the output-licence exception lists everything the exception
/// covers.**
///
/// `every_artifact_the_emitter_writes_is_named_in_the_docs` already checks this, against a
/// hand-written list of the set's homes — and its own comment says why that list matters: D23
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

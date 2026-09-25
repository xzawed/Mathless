//! Facts a document is required to carry, checked against the code that makes them true.

use super::*;

/// **Wherever a live document states how many modules the reference C host gates, it is the
/// count in `host.c`** — the home; no document has to state it.
///
/// Why: the number lagged twice on record (13 against 14, then 15 against 18). SPECs and dated
/// records are not read. (#118, #316)
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

/// **The READMEs name every built-in the compiler has and every gate CI requires.**
///
/// Why: the public first screen said four built-ins of eight and three host paths of four. Both
/// sets come from source, and names match as whole identifiers. (#267, #316)
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

/// **Every home of the artifact set names every artifact `emit.rs` packages.**
///
/// Why: the set went from three files to four, and the output-licence grant and D23 must not
/// under-list what they grant. (#124, #316)
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

/// **The `.pas` line of the README transcripts carries the note `mlc` prints on that line.**
///
/// Why: the note says the Delphi binding is unverified; without it the transcript reads as
/// "this works". (#137)
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
/// Why: one commit gave 9,728 B here and 9,216 B on `windows-latest`, so one value alone is a
/// this-machine number. SPECs and dated records are not read. (#121, #316)
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

/// **The READMEs name the staging directory a killed `mlc build` leaves behind.**
///
/// Why: it is litter, not a broken build (measured), and a user who finds it cannot tell which
/// unless a document says so. The prefix is read from the emitter. (#146)
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

/// **A document that names the output-licence exception lists everything it covers.**
///
/// Why: `OPEN_QUESTIONS.md` summarised the grant without `.lib`, one document over from the
/// guarded list; naming the licence volunteers the document for the check. (#227)
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

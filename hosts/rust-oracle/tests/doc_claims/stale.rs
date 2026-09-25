//! Claims that stopped being true: no document may still assert a state the code has left.

use super::*;

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
///   - *"the unit ships marked DRAFT"* — `grep -rn DRAFT compiler/ runtime/ hosts/` finds
///     **only this file** — its own denylist and messages.
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

/// **No document still states a claim the code has left** — one row per claim, below.
///
/// Why: five of these were functions of their own, and four copied one scan loop with one
/// off-by-one (a needle that ends the text was never tried). A new stale claim is a row, every
/// row runs, and every failure is reported — not only the first.
#[test]
fn no_document_still_states_a_claim_the_code_has_left() {
    let docs = every_markdown_file();
    let mut failures = Vec::new();
    for (i, row) in STALE.iter().enumerate() {
        assert!(
            !row.needles.is_empty() && STALE[..i].iter().all(|r| r.id != row.id),
            "row {}: a row needs a needle and an id no other row has",
            row.id
        );
        // A row whose evidence is gone is reported with the rest instead of panicking here,
        // which would hide every hit already found (Grok).
        let gone: Vec<String> = row
            .evidence
            .iter()
            .filter(|(file, marker)| !read(file).contains(marker))
            .map(|(file, marker)| {
                format!(
                    "{}: {file} no longer contains {marker:?}, the fact that made this claim \
                     false. If that fact is gone, the row is wrong and the documents may be right",
                    row.id
                )
            })
            .collect();
        if !gone.is_empty() {
            failures.extend(gone);
            continue;
        }
        for (path, text) in &docs {
            let in_scope = match row.scope {
                Scope::AllMarkdown => true,
                Scope::Live => !is_dated_record(path),
                Scope::LiveExcept(prefixes) => {
                    !is_dated_record(path) && !prefixes.iter().any(|p| path.starts_with(p))
                }
                Scope::OutsideDocs => !path.starts_with("docs/"),
            };
            if !in_scope {
                continue;
            }
            let hay: Vec<char> = if row.flatten {
                flatten_prose(text).chars().collect()
            } else {
                text.chars().collect()
            };
            for needle in row.needles {
                let nd: Vec<char> = needle.chars().collect();
                for start in starts(&hay, &nd) {
                    let letter = |c: Option<&char>| c.is_some_and(char::is_ascii_alphabetic);
                    let embedded = letter(start.checked_sub(1).and_then(|b| hay.get(b)))
                        || letter(hay.get(start + nd.len()));
                    if row.boundary && embedded {
                        continue;
                    }
                    let lo = start.saturating_sub(row.window.0);
                    let hi = (start + nd.len() + row.window.1).min(hay.len());
                    let window: String = hay[lo..hi].iter().collect();
                    let folded = window.to_lowercase();
                    let in_context = row.context.is_empty()
                        || row
                            .context
                            .iter()
                            .any(|c| folded.contains(&c.to_lowercase()));
                    let is_stale = row.forbidden.is_empty()
                        || row.forbidden.iter().any(|f| window.contains(f));
                    if in_context && is_stale {
                        failures.push(format!(
                            "{}: {path} says `{needle}`, but {}. Context: …{window}…",
                            row.id, row.truth
                        ));
                    }
                }
            }
        }
    }
    assert!(
        failures.is_empty(),
        "{} failure(s) — a document still states a claim, or a row's evidence is gone:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

/// Where a row looks. Each row names its own: one shared exemption list would quietly narrow
/// the row that deliberately reads the records too (Grok, reviewing this table's design).
#[derive(Clone, Copy)]
enum Scope {
    /// Every `.md`, dated records included.
    AllMarkdown,
    /// Every `.md` except dated records (`is_dated_record`).
    Live,
    /// Every `.md` except dated records and anything under these prefixes.
    LiveExcept(&'static [&'static str]),
    /// Every `.md` outside `docs/` — product-facing prose only.
    OutsideDocs,
}

/// One claim the code has left. `id` is the name the guard had as a function, so references
/// to it still grep to its row.
struct Stale {
    id: &'static str,
    /// `(file, marker)`: the facts that make the claim false. One missing means the ROW is
    /// wrong, not the documents.
    evidence: &'static [(&'static str, &'static str)],
    scope: Scope,
    /// Match against `flatten_prose` (markup and line breaks gone) instead of the raw text.
    flatten: bool,
    needles: &'static [&'static str],
    /// Neither neighbour of a hit may be an ASCII letter: `C뿐` sits inside `FPC뿐`, and
    /// `only C` inside `only CI` (Grok).
    boundary: bool,
    /// Chars kept before and after a hit.
    window: (usize, usize),
    /// A hit is stale only if one of these is in its window. Empty: the needle is the claim.
    forbidden: &'static [&'static str],
    /// If not empty, a hit counts only when one of these is also in the window, case-folded.
    context: &'static [&'static str],
    /// What is true instead.
    truth: &'static str,
}

const STALE: &[&Stale] = &[
    &A_REJECTED_SLICE_IS_NOT_DESCRIBED_AS_MERELY_PENDING,
    &NO_DOCUMENT_CALLS_INTERFACE_METADATA_UNIMPLEMENTED,
    &NO_DOCUMENT_SAYS_C_IS_THE_ONLY_GATED_HOST,
    &NO_DOCUMENT_CALLS_ARRAY_RETURN_UNIMPLEMENTED,
    &NO_LIVE_DOCUMENT_NAMES_THE_BARE_FINGERPRINT_EXPORT,
];

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
const A_REJECTED_SLICE_IS_NOT_DESCRIBED_AS_MERELY_PENDING: Stale = Stale {
    id: "a_rejected_slice_is_not_described_as_merely_pending",
    evidence: &[
        ("docs/slices/README.md", "⛔"),
        ("docs/slices/README.md", "SPEC-symbol-embedded-hash.md"),
    ],
    scope: Scope::LiveExcept(&["docs/slices/"]),
    flatten: true,
    needles: &["DP-H3(b)"],
    boundary: false,
    window: (0, 90),
    forbidden: &["아직 하지 않았", "아직 안 했"],
    context: &[],
    truth: "docs/slices/README.md marks it ⛔ 하지 않는다 (#141, 2026-09-05). \"Not done yet\" \
            invites a session to do it; \"decided against\" sends them to the reasons",
};

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
const NO_DOCUMENT_CALLS_INTERFACE_METADATA_UNIMPLEMENTED: Stale = Stale {
    id: "no_document_calls_interface_metadata_unimplemented",
    evidence: &[("compiler/src/iface.rs", "ml-iface/1")],
    scope: Scope::Live,
    flatten: true,
    needles: &["인터페이스 메타"],
    boundary: false,
    window: (0, 40),
    forbidden: &["미구현", "⏳"],
    context: &[],
    truth: "compiler/src/iface.rs builds the ml-iface/1 manifest, every module exports \
            ml_iface_hash_<module>, and both reference C hosts refuse a drifted one",
};

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
const NO_DOCUMENT_SAYS_C_IS_THE_ONLY_GATED_HOST: Stale = Stale {
    id: "no_document_says_c_is_the_only_gated_host",
    evidence: &[(
        ".github/workflows/ci.yml",
        "MATHLESS_GATE_FPC_HOST: require",
    )],
    scope: Scope::Live,
    flatten: true,
    needles: &["C뿐", "C 쪽만", "only C"],
    boundary: true,
    window: (70, 70),
    forbidden: &[],
    context: &["게이트", "gate"],
    truth: "ci.yml requires MATHLESS_GATE_FPC_HOST — an Object Pascal host loads x64 modules \
            and calls them on every push",
};

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
const NO_DOCUMENT_CALLS_ARRAY_RETURN_UNIMPLEMENTED: Stale = Stale {
    id: "no_document_calls_array_return_unimplemented",
    evidence: &[("compiler/src/codegen.rs", "RetAbi::ArrayOut")],
    scope: Scope::AllMarkdown,
    flatten: true,
    needles: &["배열 반환"],
    boundary: false,
    window: (45, 45),
    forbidden: &["미구현", "⏳"],
    context: &[],
    truth: "compiler/src/codegen.rs emits RetAbi::ArrayOut and SPEC-array-return closed \
            acceptance A~H on 2026-09-12",
};

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
const NO_LIVE_DOCUMENT_NAMES_THE_BARE_FINGERPRINT_EXPORT: Stale = Stale {
    id: "no_live_document_names_the_bare_fingerprint_export",
    evidence: &[("compiler/src/codegen.rs", "ml_iface_hash_{}")],
    scope: Scope::OutsideDocs,
    flatten: false,
    needles: &["ml_iface_hash("],
    boundary: false,
    window: (0, 0),
    forbidden: &[],
    context: &[],
    truth: "every module exports ml_iface_hash_<module> (compiler/src/codegen.rs): a host \
            author copying the bare name gets NULL from GetProcAddress and skips the \
            fingerprint check, which looks the same as passing it. Files under docs/ are \
            exempt because they record the rename",
};

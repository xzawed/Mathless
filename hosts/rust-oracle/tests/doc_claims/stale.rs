//! Claims that stopped being true: no document may still assert a state the code has left.

use super::*;

/// **No file says the ABI version refusal is unimplemented, and each says the reference host
/// refuses.**
///
/// Why: four files kept saying nothing refused after `host.c` began to. A denylist misses a
/// paraphrase, so the true sentence is required as well. (#118)
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
/// Why: CONTRIBUTING called the unit never compiled, BLOCKED and marked DRAFT for sixteen days,
/// all false. A correction describes an old sentence rather than quoting it. (#262)
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
            names_identifier(&contributing, gate),
            "CONTRIBUTING.md no longer names {gate}. Dropping the false sentence is not \
             enough — the one document a new contributor reads has to say which gate covers \
             the Delphi arm, or the absence reads as absence of the gate"
        );
    }
}

/// **`LANGUAGE.md` does not deny a feature it goes on to document.**
///
/// Why: three items said "you cannot" — string return, `try` on exports, non-ASCII literals —
/// while the same file documented them. Each denial is checked against its evidence. (#275)
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
/// Why: five guards were functions of their own and four copied one scan loop with one
/// off-by-one. A new stale claim is a row; every row runs, every failure is reported. (#315)
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
                Scope::Live => !is_record(path),
                Scope::LiveExcept(prefixes) => {
                    !is_record(path) && !prefixes.iter().any(|p| path.starts_with(p))
                }
                Scope::OutsideDocs => !path.starts_with("docs/"),
                Scope::Only(paths) => paths.contains(&path.as_str()),
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
    /// Every `.md`, records (dated ones and SPECs) included.
    AllMarkdown,
    /// Every `.md` except records (`is_record`: dated records and SPECs).
    Live,
    /// Every `.md` except records and anything under these prefixes.
    LiveExcept(&'static [&'static str]),
    /// Every `.md` outside `docs/` — product-facing prose only.
    OutsideDocs,
    /// Exactly these `.md` files, for a claim only they ever made.
    Only(&'static [&'static str]),
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
    &THE_READMES_DO_NOT_UNDERSTATE_THE_EXPORT_SET,
];

/// **A slice decided against is not "not done yet".**
///
/// Why: DP-H3(b) was rejected (#141) but two documents called it pending, which invites a
/// session to do it. SPECs and dated records are exempt. (#270)
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

/// **No document calls interface metadata unimplemented while every module ships a
/// fingerprint.**
///
/// Why: `ARCHITECTURE.md` called it ⏳ 미구현 while the same file relied on it. (#269)
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
/// Why: two documents in the reading order kept saying so after the Pascal host joined CI. The
/// boundary keeps `C뿐` inside `FPC뿐` and `only C` inside `only CI` from matching. (#268)
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
/// Why: `HOST_ABI.md`'s canonical section said ⏳ 미구현 in two spellings; proximity catches both
/// where a phrase list caught one. Dated records are read too — none says it. (#266)
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
/// Why: two READMEs outside `docs/` kept the bare `ml_iface_hash()` after #215; a host author
/// copying it gets NULL and skips the check, which looks the same as passing it. (#263)
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

/// **The READMEs do not call the export set two symbols.**
///
/// Why: both kept "two" for two slices after #105 added a third. They no longer enumerate the
/// set — SECURITY does — so what is left to pin is that sentence. (#118, #316)
const THE_READMES_DO_NOT_UNDERSTATE_THE_EXPORT_SET: Stale = Stale {
    id: "the_readmes_do_not_understate_the_export_set",
    evidence: &[(
        "hosts/rust-oracle/tests/protection.rs",
        "ml_iface_hash_{module}",
    )],
    scope: Scope::Only(&["README.md", "README.ko.md"]),
    flatten: false,
    needles: &["the two symbols", "심볼 두 개"],
    boundary: false,
    window: (0, 0),
    forbidden: &[],
    context: &[],
    truth: "every module exports three symbols since #105 — protection.rs pins the set and \
            docs/SECURITY.md lists it",
};

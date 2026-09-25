//! Size and shape of what a session reads: each document, and each file of this suite, opens
//! in one Read.

use super::*;

/// **The slice index is one table, and it links every SPEC.**
///
/// Why: stray blank lines split it and nine rows rendered as literal pipes; a SPEC missing from
/// the index is a slice nobody can find. (#151)
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

/// **The "여기서 시작한다" block has to be somewhere a session can start.**
///
/// Why: it had grown to 759 lines of dated entries — a log under a heading that says start
/// here. Live entries get 120 lines; the oldest moves to `start-block-NNN.md`. (#289)
#[test]
fn the_start_here_block_is_a_starting_point_not_a_log() {
    // The budget is on the NARRATIVE, not the whole block, and the difference was measured
    // rather than assumed. After the 2026-09-23 move the block was 198 lines: 48 of live
    // entry and 150 of index. The index grows by ONE ROW PER SESSION and each row is one
    // line — bounded by construction, and it was the navigation the block needed. The entries
    // are what grew to 759 lines. Budgeting the block punished the part that cannot run away
    // and left two lines of headroom for the part that can; the first version did exactly
    // that, and it fired on the commit that closed the slice.
    //
    // (2026-09-25: the index itself moved to docs/history/start-block-index.md when the whole
    // file got a budget of its own. Budgeting the entries rather than the block still holds —
    // the block now keeps only the `그 앞의 항목들` heading that points there.)
    const BUDGET: usize = 120;
    let status = read("docs/STATUS.md");
    let lines: Vec<&str> = status.lines().collect();

    let start = lines
        .iter()
        .position(|l| l.starts_with("> ## ▶"))
        .expect("docs/STATUS.md no longer has the ▶ block that CLAUDE.md sends sessions to");
    let end = lines[start + 1..]
        .iter()
        .position(|l| l.starts_with("## "))
        .map(|i| start + 1 + i)
        .unwrap_or(lines.len());

    // The index heading separates the two. Its absence is a floor: without it this would
    // measure the whole block again and quietly go back to punishing the index.
    let index_at = lines[start..end]
        .iter()
        .position(|l| l.contains("그 앞의 항목들"))
        .map(|i| start + i)
        .expect(
            "the ▶ block no longer has its `그 앞의 항목들` heading. That heading is what \
             separates the live entries from the pointer to the dated index \
             (docs/history/start-block-index.md), and this guard budgets only the first — if \
             the pointer is gone, the moved entries have nowhere to be found from and the \
             budget below is measuring the wrong thing",
        );

    let len = index_at - start;
    assert!(
        len <= BUDGET,
        "the ▶ block's live entries run {len} lines, over the {BUDGET}-line budget (the dated \
         index below them is not counted — it grows one line per session and is bounded). The \
         heading says this is where a session starts, and CLAUDE.md sends every session to it; \
         at {len} lines of narrative it is a log instead. Move the oldest DATED ENTRY \
         VERBATIM into a new docs/history/start-block-NNN.md (the next number) and add a dated \
         row to docs/history/start-block-index.md, then verify byte-for-byte against `git show`. If there is only ONE entry, the entry itself is too \
         long: its retrospective belongs in that session's §9-N, and durable rules belong in \
         §7 — leave a pointer, not a summary. Do not trim by deleting, and do not move the \
         standing N·D·X·R·A \
         queue that follows the entries — it is a list, not a record."
    );

    // The standing queue does not leave with the dated entries. Moving it was the mistake the
    // docstring records, and it is silent: the rows read the same in either file, so only their
    // absence shows. An open row stays in STATUS; a closed one moves to the closed registry
    // under the same number (`status_holds_no_closed_item` keeps the two apart), so a row is
    // looked for on every page — never in the session log or a start-block record.
    let pages = status_pages();
    for row in ["**X1**", "**X2**", "**X3**", "**R1**", "**R3**"] {
        assert!(
            pages.iter().any(|(_, text)| text.contains(row)),
            "the standing queue row {row} is in neither docs/STATUS.md nor its closed registry. \
             The queue is a list a session reads for what to do next, not a dated record: an \
             open row stays in STATUS, a closed one moves to docs/history/status-closed-NNN.md \
             under the same number — other documents cite the rows by number."
        );
    }
}

/// **`STATUS.md` fits in one Read: 45,000 bytes, `\r` not counted.**
///
/// Why: the Read tool showed a quarter of it per call and the ▶ block sat on the fourth page.
/// At 2.06 bytes per token the 25,000-token cap is about 51.5 KB. (#310)
#[test]
fn status_fits_in_one_read() {
    const BUDGET: usize = 45_000;
    let len = read("docs/STATUS.md").replace('\r', "").len();
    assert!(
        len <= BUDGET,
        "docs/STATUS.md is {len} bytes (LF), over the {BUDGET}-byte budget. The budget sits \
         below the Read tool's 25,000-token cap (about 51.5 KB for this file) so the ratio of \
         prose to code can move; past that cap the tool returns the file in pages and a \
         session sees only the first. Do not trim by deleting: move CLOSED items \
         byte-identical to docs/history/status-closed-NNN.md under the same heading and \
         number, where `STATUS §<n>` citations still resolve (status_holds_no_closed_item). \
         Open items and current facts stay in full."
    );
}

/// **Every file under `docs/history/` fits in one Read (40,000 bytes), and every link STATUS
/// makes into it lands on a file that exists.**
///
/// Why: an archive no session can open whole trades one problem for another. (#310)
#[test]
fn every_history_file_fits_in_one_read() {
    const BUDGET: usize = 40_000;
    let files: Vec<(String, String)> = every_markdown_file()
        .into_iter()
        .filter(|(path, _)| path.starts_with("docs/history/"))
        .collect();
    assert!(
        !files.is_empty(),
        "docs/history/ holds no markdown files, so nothing STATUS.md collapsed has anywhere to be"
    );
    for (path, text) in &files {
        let len = text.replace('\r', "").len();
        assert!(
            len <= BUDGET,
            "{path} is {len} bytes (LF), over the {BUDGET}-byte budget. Split it into two files \
             and point STATUS.md at both — a history file that cannot be read in one call is \
             the problem this directory was made to solve"
        );
    }

    let status = read("docs/STATUS.md");
    let mut linked = 0;
    for (at, _) in status.match_indices("](history/") {
        let rest = &status[at + 2..];
        // A link may carry a fragment (`#…`) or a title (`"…"` after a space); the path ends
        // at whichever comes first. Grok's review found the title case reading as a missing file.
        let end = rest
            .find([')', '#', ' '])
            .expect("a markdown link into history/ that never closes");
        let target = format!("docs/{}", &rest[..end]);
        linked += 1;
        assert!(
            files.iter().any(|(path, _)| *path == target),
            "docs/STATUS.md links to {target}, which does not exist"
        );
    }
    assert!(
        linked > 0,
        "docs/STATUS.md links into docs/history/ nowhere, so the archive is unreachable from the \
         file a session reads"
    );
}

/// **`HISTORY.md`, the index of the session log, fits in one Read.**
///
/// Why: at 378 KB the Read tool refused it outright. It holds one-line `### 9-N.` stubs so the
/// `§9-N` citations land on a heading; same budget and `\r` rule as STATUS. (#312)
#[test]
fn the_history_index_fits_in_one_read() {
    const BUDGET: usize = 45_000;
    let len = read("docs/HISTORY.md").replace('\r', "").len();
    assert!(
        len <= BUDGET,
        "docs/HISTORY.md is {len} bytes (LF), over the {BUDGET}-byte budget — it is the index of \
         the session log, and past the Read tool's cap a session sees only its first page. A \
         session's record goes to a NEW file, docs/history/9-N.md, and HISTORY.md gains one stub \
         line `### 9-N. <title> → [본문](history/9-N.md)` at the top — never the body. When the \
         stubs alone outgrow the budget, ROLL: move the oldest stubs, unchanged except that \
         their link loses `history/`, into the next docs/history/index-NNN.md, and leave \
         HISTORY.md a link to that page. The citation and stub guards read the rolled pages \
         too, so `§9-N` keeps landing on exactly one heading."
    );
}

/// **Every stub in the HISTORY index has its file and every file its stub; ▶ records are
/// numbered without gaps.**
///
/// Why: a missing file strands the citations on its stub, a stub-less file is an entry the
/// index hides, and gapless numbers make "the next number" unambiguous. (#312)
#[test]
fn the_history_archive_is_indexed_and_numbered() {
    let pages = history_index_pages();
    let history = pages[0].1.clone();
    let files: Vec<(String, String)> = every_markdown_file()
        .into_iter()
        .filter(|(path, _)| path.starts_with("docs/history/"))
        .collect();

    // `docs/history/9-<digits>.md` → the number in its name.
    let entry_number = |path: &str| -> Option<u32> {
        path.strip_prefix("docs/history/9-")?
            .strip_suffix(".md")?
            .parse()
            .ok()
    };

    let mut stubs: Vec<u32> = Vec::new();
    for (page, text, prefix) in &pages {
        if page != "docs/HISTORY.md" {
            let name = page
                .strip_prefix("docs/history/")
                .expect("a rolled page lives under docs/history/");
            assert!(
                history.contains(&format!("](history/{name})")),
                "{page} holds rolled stubs, but HISTORY.md does not link it — the entries on it \
                 are unreachable from the index a session opens"
            );
        }
        // Stubs only: from a page's first stub to its last, every line is a stub. Body text
        // between them is how the index would turn back into the log it replaced.
        let lines: Vec<&str> = text.lines().collect();
        let is_stub = |l: &&str| l.starts_with("### 9-");
        if let (Some(first), Some(last)) = (
            lines.iter().position(is_stub),
            lines.iter().rposition(is_stub),
        ) {
            for (i, line) in lines[first..=last].iter().enumerate() {
                assert!(
                    is_stub(line),
                    "{page}:{} sits between the stubs but is not one: {line:?}. The index holds \
                     one line per entry — the entry's text belongs in its own docs/history/9-N.md",
                    first + i + 1
                );
            }
        }
        for line in &lines {
            let Some(rest) = line.strip_prefix("### 9-") else {
                continue;
            };
            let Some((num, _)) = rest.split_once(". ") else {
                continue;
            };
            let Ok(n) = num.parse::<u32>() else {
                continue;
            };
            let target = format!("docs/history/9-{n}.md");
            assert!(
                line.contains(&format!("]({prefix}9-{n}.md)")),
                "{page}'s `### 9-{n}.` heading does not link {prefix}9-{n}.md. Each heading on an \
                 index page is a one-line stub pointing at its entry's own file"
            );
            let Some((_, body)) = files.iter().find(|(path, _)| *path == target) else {
                panic!("{page} stubs §9-{n}, but {target} does not exist");
            };
            assert!(
                body.replace('\r', "").starts_with(&format!("### 9-{n}. ")),
                "{target} does not begin with its own `### 9-{n}.` heading — the file holds \
                 another entry, or the entry lost its heading"
            );
            stubs.push(n);
        }
    }
    // Newest first, as the preface says: 가장 큰 번호부터 9-1로 내려간다 — across the pages too,
    // since a roll moves the OLDEST stubs. The log broke that once before the split — §9-53 sat
    // between §9-60 and §9-59 — and nothing noticed until the split's review read the stubs in a
    // row (Grok, 2026-09-25).
    for pair in stubs.windows(2) {
        assert!(
            pair[0] > pair[1],
            "the HISTORY index lists §9-{} above §9-{}: the stubs run newest first, largest \
             number at the top, as its preface says",
            pair[0],
            pair[1]
        );
    }
    let mut unique = stubs.clone();
    unique.sort_unstable();
    unique.dedup();
    assert_eq!(
        unique.len(),
        stubs.len(),
        "the HISTORY index stubs some `§9-N` more than once"
    );

    let mut entries: Vec<u32> = files
        .iter()
        .filter_map(|(path, _)| entry_number(path))
        .collect();
    entries.sort_unstable();
    assert!(
        entries.len() >= 62,
        "docs/history/ holds {} `9-N.md` entries, fewer than the 62 the log had when it first \
         moved out of STATUS.md",
        entries.len()
    );
    for n in &entries {
        assert!(
            stubs.contains(n),
            "docs/history/9-{n}.md has no stub in HISTORY.md, so the index hides it"
        );
    }

    // Three digits, always: `.parse()` alone would take `start-block-47.md` as 47, and the next
    // number would sort out of place in every listing (Grok).
    let mut blocks: Vec<u32> = Vec::new();
    for (path, _) in &files {
        let Some(stem) = path
            .strip_prefix("docs/history/start-block-")
            .and_then(|rest| rest.strip_suffix(".md"))
        else {
            continue;
        };
        if stem == "index" {
            continue;
        }
        assert!(
            stem.len() == 3 && stem.bytes().all(|b| b.is_ascii_digit()),
            "{path}: a ▶ record is named start-block-NNN.md with exactly three digits"
        );
        blocks.push(stem.parse().expect("three ascii digits"));
    }
    blocks.sort_unstable();
    assert!(
        !blocks.is_empty(),
        "docs/history/ holds no start-block-NNN.md records"
    );
    for (i, n) in blocks.iter().enumerate() {
        assert_eq!(
            *n,
            i as u32 + 1,
            "start-block records must be numbered 001, 002, … without gaps or repeats; found \
             {blocks:?}"
        );
    }
    for (path, body) in &files {
        if path.starts_with("docs/history/start-block-")
            && path != "docs/history/start-block-index.md"
        {
            assert!(
                body.replace('\r', "").starts_with("> ### "),
                "{path} does not begin with a quoted `> ### <date>` heading — a ▶ record moves \
                 byte-identical, quote marks included"
            );
        }
    }
}

/// **The documents a session reads first each fit in one Read and together stay under 100 KB.**
///
/// Why: per-file caps left the four at 110.7 KB together. The set is derived from STATUS §9
/// step 1, the total is the user's decision, and `CLAUDE.md` stays under 200 lines. (#313)
#[test]
fn the_session_start_documents_fit_in_one_read() {
    const EACH: usize = 45_000;
    const TOTAL: usize = 100_000;
    const CLAUDE_LINES: usize = 200;

    let status = read("docs/STATUS.md").replace('\r', "");
    // Inside §9 only: the first `1. 이 문서` anywhere in the file would do today, but a line of
    // that shape written earlier would silently become the reading order (Grok).
    let order = status
        .lines()
        .skip_while(|l| !l.starts_with("## 9. "))
        .find(|l| l.starts_with("1. 이 문서"))
        .expect(
            "docs/STATUS.md §9 step 1 — the reading order this guard derives its set from — is \
             gone or reworded. Put the order back, or teach this guard where it went",
        );
    let mut docs = vec!["docs/STATUS.md".to_string()];
    for span in order.split('`').skip(1).step_by(2) {
        if span.ends_with(".md") {
            docs.push(span.to_string());
        }
    }
    assert!(
        docs.len() >= 4,
        "derived only {docs:?} from STATUS §9 step 1 — the line changed shape, so this guard is \
         checking fewer documents than a session actually reads"
    );

    let mut total = 0usize;
    for doc in &docs {
        let len = read(doc).replace('\r', "").len();
        assert!(
            len <= EACH,
            "{doc} is {len} bytes (LF), over {EACH}: a session is told to read it first, and \
             past one read it sees only the first page. Collapse closed items to one line and \
             move their narrative, byte-identical, under docs/history/"
        );
        total += len;
    }
    assert!(
        total <= TOTAL,
        "the documents in STATUS §9 step 1 ({docs:?}) weigh {total} bytes (LF) together, over \
         {TOTAL}. Every session pays this before doing anything; shrink the largest, or take a \
         document out of the reading order if a session does not need it up front"
    );

    let claude = read("CLAUDE.md").lines().count();
    assert!(
        claude <= CLAUDE_LINES,
        "CLAUDE.md is {claude} lines, over the {CLAUDE_LINES} the user's global rules allow for a \
         file loaded every session — move detail to the document it describes and leave a \
         one-line pointer"
    );
}

/// **Every row of the slice index is one line — at most 400 bytes.**
///
/// Why: closed rows kept their narrative and the index grew from 4.4 to 36.5 KB in 27 days; a
/// row cap stops it at the first paragraph written into a cell. (#313)
#[test]
fn every_slice_index_row_is_one_line() {
    const ROW: usize = 400;
    let index = read("docs/slices/README.md").replace('\r', "");
    let mut rows = 0usize;
    for (no, line) in index.lines().enumerate() {
        if !(line.starts_with("| [") && line.contains("](SPEC-")) {
            continue;
        }
        rows += 1;
        assert!(
            line.len() <= ROW,
            "docs/slices/README.md:{} is {} bytes, over {ROW}: a row says what the slice added in \
             one line and links its SPEC and PRs. The story of how it got there belongs in the \
             slice's §9-N record",
            no + 1,
            line.len()
        );
    }
    assert!(
        rows >= 25,
        "found only {rows} index rows — the table's shape changed, so this guard would pass by \
         reading nothing"
    );
}

/// **Every file of this suite opens in one Read, and every file under `doc_claims/` compiles.**
///
/// Why: the suite was one 205 KB file, and a file under `tests/doc_claims/` that the root does
/// not declare compiles nothing — its guards would vanish without a red. (#314)
#[test]
fn the_guard_suite_fits_in_one_read_per_file() {
    const BUDGET: usize = 45_000;
    let root_path = "hosts/rust-oracle/tests/doc_claims.rs";
    let root = read(root_path).replace('\r', "");
    let root_lines: Vec<&str> = root.lines().collect();
    let mut files = vec![(root_path.to_string(), root.clone())];
    // `expect`, not `if let Ok`: this guard lives in that directory, and a walk that skips a
    // missing one would pass by checking nothing (Grok, twice).
    let entries = std::fs::read_dir(repo_root().join("hosts/rust-oracle/tests/doc_claims"))
        .expect("hosts/rust-oracle/tests/doc_claims/ holds this suite's theme files");
    for entry in entries {
        let path = entry.expect("a directory entry").path();
        let name = path
            .file_name()
            .expect("a file name")
            .to_string_lossy()
            .into_owned();
        let stem = name.strip_suffix(".rs").unwrap_or_else(|| {
            panic!("tests/doc_claims/{name}: only this suite's modules live here, as <theme>.rs")
        });
        // A crate root resolves `mod x;` next to itself, so a theme is compiled only by
        // `#[path = "doc_claims/x.rs"]` directly above `mod x;`.
        let attr = format!("#[path = \"doc_claims/{name}\"]");
        let decl = format!("mod {stem};");
        assert!(
            path.is_file() && root_lines.windows(2).any(|w| w[0] == attr && w[1] == decl),
            "tests/doc_claims/{name} is not declared in {root_path} as `{attr}` followed by \
             `{decl}`, so it compiles nothing and every guard in it is silently gone"
        );
        let text = std::fs::read_to_string(&path).expect("read a suite module");
        files.push((
            format!("hosts/rust-oracle/tests/doc_claims/{name}"),
            text.replace('\r', ""),
        ));
    }
    for (path, text) in &files {
        assert!(
            text.len() <= BUDGET,
            "{path} is {} bytes, over {BUDGET}: it no longer opens in one Read. Move a theme's \
             guards into tests/doc_claims/<theme>.rs and declare it in {root_path} as \
             `#[path = \"doc_claims/<theme>.rs\"]` followed by `mod <theme>;`",
            text.len()
        );
    }
}

/// **A live document's count of correction notes only goes down.**
///
/// Why: repairs appended when and how a sentence was wrong, so live documents grew a second,
/// dated history. A note is 정정, a dated 감사, `used to say` or `was wrong`; records and SPECs
/// are not read, and a document not listed starts at zero. (#317)
#[test]
fn live_documents_do_not_accumulate_correction_notes() {
    // Measured 2026-09-25, after CLAUDE.md dropped its five. Removing a note means lowering the
    // number here in the same change; that is what keeps the count from growing back.
    // CLAUDE.md's one is the rule that forbids the notes, which has to name them.
    const CEILINGS: &[(&str, usize)] = &[
        ("CLAUDE.md", 1),
        ("CONTRIBUTING.md", 2),
        ("README.md", 1),
        ("docs/ARCHITECTURE.md", 2),
        ("docs/DECISIONS.md", 4),
        ("docs/GLOSSARY.md", 1),
        ("docs/HOST_ABI.md", 5),
        ("docs/LANGUAGE.md", 4),
        ("docs/OPEN_QUESTIONS.md", 6),
        ("docs/VISION.md", 1),
        ("docs/phase1/SPEC.md", 1),
        ("docs/phase1/WBS.md", 2),
        ("docs/slices/README.md", 6),
    ];

    let dated_audits = |text: &str| {
        text.match_indices(" 감사")
            .filter(|(at, _)| {
                let before = &text.as_bytes()[..*at];
                before.len() >= 10 && {
                    let d = &before[before.len() - 10..];
                    d[4] == b'-'
                        && d[7] == b'-'
                        && d.iter()
                            .enumerate()
                            .all(|(i, b)| i == 4 || i == 7 || b.is_ascii_digit())
                }
            })
            .count()
    };
    let docs = every_markdown_file();
    let mut wrong = Vec::new();
    for (path, text) in &docs {
        if is_record(path) {
            continue;
        }
        let lower = text.to_lowercase();
        let notes = text.matches("정정").count()
            + dated_audits(text)
            + lower.matches("used to say").count()
            + lower.matches("was wrong").count();
        let ceiling = CEILINGS
            .iter()
            .find(|(p, _)| p == path)
            .map_or(0, |(_, c)| *c);
        if notes > ceiling {
            wrong.push(format!(
                "{path} has {notes} correction notes, over its ceiling {ceiling}: rewrite the \
                 wrong sentence in place and put what was wrong, and when, in the session record \
                 (docs/history/9-N.md)"
            ));
        } else if notes < ceiling {
            wrong.push(format!(
                "{path} has {notes} correction notes, under its ceiling {ceiling}: lower its \
                 ceiling to {notes} here so it cannot grow back"
            ));
        }
    }
    for (p, _) in CEILINGS {
        if !docs.iter().any(|(path, _)| path == p) {
            wrong.push(format!(
                "{p} has a ceiling here but no longer exists — a renamed document carries its \
                 ceiling to the new name"
            ));
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

/// **Every guard's docstring is at most six lines: what it keeps true, why, and the PR.**
///
/// Why: the story beside each guard was the template the next one copied — 31 of 53 narrated
/// their incident, at a median of 16 lines. The story lives in the PR and the session. (#319)
#[test]
fn every_guard_docstring_is_short() {
    const LINES: usize = 6;
    let mut files = vec!["hosts/rust-oracle/tests/doc_claims.rs".to_string()];
    let dir = repo_root().join("hosts/rust-oracle/tests/doc_claims");
    for entry in std::fs::read_dir(&dir).expect("hosts/rust-oracle/tests/doc_claims/") {
        let name = entry
            .expect("a directory entry")
            .file_name()
            .to_string_lossy()
            .into_owned();
        files.push(format!("hosts/rust-oracle/tests/doc_claims/{name}"));
    }
    let mut guards = 0usize;
    let mut long = Vec::new();
    for path in &files {
        let text = read(path).replace('\r', "");
        let lines: Vec<&str> = text.lines().collect();
        for (i, line) in lines.iter().enumerate() {
            let attrs = lines[..i].iter().rev().take_while(|l| l.starts_with("#["));
            let is_test = line.starts_with("fn ") && attrs.clone().any(|l| *l == "#[test]");
            let is_row = line.starts_with("const ") && line.contains(": Stale = ");
            if !(is_test || is_row) {
                continue;
            }
            guards += 1;
            let doc: Vec<&&str> = lines[..i]
                .iter()
                .rev()
                .skip_while(|l| l.starts_with("#["))
                .take_while(|l| l.starts_with("///"))
                .collect();
            if doc.len() > LINES {
                long.push(format!(
                    "{path}:{} has {} doc lines, over {LINES}",
                    i + 1,
                    doc.len()
                ));
            }
            // The PR is where the story went; a docstring without one has dropped it.
            let cites_pr = doc.iter().any(|l| {
                l.match_indices("(#")
                    .any(|(at, _)| l[at + 2..].starts_with(|c: char| c.is_ascii_digit()))
            });
            if !cites_pr {
                long.push(format!(
                    "{path}:{} names no PR — end the docstring with (#N), the PR that holds its story",
                    i + 1
                ));
            }
        }
    }
    assert!(
        guards >= 50,
        "found only {guards} guards in the suite — this walk is reading less than it holds"
    );
    assert!(
        long.is_empty(),
        "{}\nKeep a guard's docstring to what it keeps true, why in a line or two, and the PR: the \
         story belongs in the PR and the session record, where it does not become the next \
         guard's template",
        long.join("\n")
    );
}

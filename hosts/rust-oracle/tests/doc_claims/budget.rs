//! Size and shape of what a session reads: each document, and each file of this suite, opens
//! in one Read.

use super::*;

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

/// **The "여기서 시작한다" block has to be somewhere you can start.**
///
/// The heading is a claim: this is where a session picks up work, and `CLAUDE.md` sends every
/// new session to it by name. It had grown to **759 lines across 45 dated entries**, 40 of
/// them summaries of a `§9-N` whose body is in `HISTORY.md` — the same narrative written
/// twice, in the file whose third line says a session reads it first. A block that long is a
/// log, and a log falsifies the heading above it.
///
/// So this is not a style rule about length; it is the heading's own claim, checked. The
/// budget is 200 lines because that is the number the global instructions use for a document
/// loaded every session, and because it is the only number in play that was not invented
/// here. After the move the block sat at 93, so the room left is real rather than nominal.
///
/// **What to do when it goes red** is in the message and matters more than the limit: move
/// the oldest DATED ENTRY verbatim into a new `docs/history/start-block-NNN.md` (the next
/// number) and add a dated row to the index, `docs/history/start-block-index.md`. `#265` and
/// the 2026-09-23 move did the same into `HISTORY.md` and verified the moved text
/// byte-for-byte against `git show` afterwards; on 2026-09-25 `HISTORY.md` became an index and
/// the records became one file each. The index itself lived inside the block until 2026-09-25,
/// when `status_fits_in_one_read` moved it out; the block keeps its `그 앞의 항목들` heading as
/// the pointer to it.
///
/// Two wrong answers, one of which was made and caught. Trimming by deleting is the first.
/// The second is moving more than the dated entries: the first cut of the 2026-09-23 move
/// swallowed the standing N·D·X·R·A queue that follows them, and `X1`–`X3`, `R1` and `R3` are
/// not closed — live work went into the history file. Grok's review caught it. The queue is
/// not a dated record and does not move.
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

    // The standing queue stays in this file. Moving it was the mistake the docstring records,
    // and it is silent: the rows read the same in either file, so only their absence here shows.
    // Which rows are still open is the rows' own ✅ marks, not a list here — this message named
    // R1 and R3 as open after both had closed (R1 on 2026-09-23, R3 on 2026-09-24).
    for row in ["**X1**", "**X2**", "**X3**", "**R1**", "**R3**"] {
        assert!(
            status.contains(row),
            "docs/STATUS.md no longer carries the standing queue row {row}. The queue is a \
             list a session reads for what to do next, not a dated record, so its rows belong \
             here and not in docs/HISTORY.md — open or closed. If one has closed, mark it ✅ \
             here rather than moving it: other documents cite the rows by number."
        );
    }
}

/// **`STATUS.md` is the file every session reads first, so it has to fit in one read.**
///
/// Measured 2026-09-25 with the agent's own Read tool: it returned *"lines 1-443 of 1944
/// total (93243 tokens, cap 25000)"* for this file, and the ▶ block a session is sent to sat
/// on the fourth page, at line 1713. The first page was 61% closed slice records under a
/// heading that says it is not the place to start. Nothing bounded the file — the ▶ block's
/// budget covers its live entry and nothing around it — so it grew back after each move
/// (+87 lines in the two days after the 2026-09-23 one).
///
/// The budget is in bytes because bytes are what a test can count; tokens are what the tool
/// counts. The file measured 2.06 LF bytes per token, so the 25,000-token cap is about 51.5 KB,
/// and 45,000 leaves room for that ratio to move with the mix of prose and code. `\r` is not
/// counted: Windows checks the file out with CRLF and ubuntu with LF, and a budget that
/// differs by platform is two budgets.
///
/// **When it goes red, move — do not delete.** A closed item keeps one line here and its
/// narrative moves byte-identical to `docs/history/status-<section>.md`; a dated record moves
/// whole and leaves its heading behind as a one-line stub, because other files cite
/// `STATUS §<n>` (`every_status_citation_resolves`).
#[test]
fn status_fits_in_one_read() {
    const BUDGET: usize = 45_000;
    let len = read("docs/STATUS.md").replace('\r', "").len();
    assert!(
        len <= BUDGET,
        "docs/STATUS.md is {len} bytes (LF), over the {BUDGET}-byte budget. The budget sits \
         below the Read tool's 25,000-token cap (about 51.5 KB for this file) so the ratio of \
         prose to code can move; past that cap the tool returns the file in pages and a \
         session sees only the first. Do not trim by \
         deleting: collapse CLOSED items to one line that links their narrative, and move the \
         narrative byte-identical to docs/history/status-<section>.md. A dated record moves \
         whole and leaves its heading as a one-line stub, so `STATUS §<n>` citations resolve. \
         Open items and current facts stay in full."
    );
}

/// **Every file under `docs/history/` fits in one read too, and `STATUS.md` points at real ones.**
///
/// The archive exists so that `STATUS.md` can stay small, and trading one file no session can
/// open whole for another would be no trade — that is what `docs/HISTORY.md` became on the
/// day it was created (264 KB against the Read tool's 256 KB cap). 40,000 bytes keeps a file
/// inside one read with room left; a section that outgrows it splits into two files.
///
/// The link half is the floor. An archive nothing points at is invisible, and a pointer to a
/// file that is not there is the silent loss the move exists to rule out.
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

/// **`HISTORY.md` is the index of the session log, and an index has to open in one read.**
///
/// Measured 2026-09-25: the Read tool refused the file outright — 378 KB against its 256 KB cap
/// — and `CLAUDE.md`'s reading order put it second, right after `STATUS.md`. The split turned
/// it into one-line `### 9-N.` stubs, each linking `docs/history/9-N.md`, so that the `§9-N`
/// citations spread across the tree still land on a heading here. Same budget and same `\r`
/// rule as `status_fits_in_one_read`.
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

/// **Every stub in `HISTORY.md` has its file, every file has its stub, and the ▶ records are
/// numbered without gaps.**
///
/// The split's two promises, checked from both ends. A stub whose file is missing strands the
/// `§9-N` citations that land on it; a file without a stub is an entry the index hides. The
/// file's first line must be the same `### 9-N.` heading, so a file renamed or overwritten with
/// another entry does not pass as the right one.
///
/// ▶ block records are `start-block-NNN.md`, numbered from 001 (the oldest) and never edited
/// once written: a session moves its oldest ▶ entry into the next number. Contiguous numbering
/// is what makes "the next number" unambiguous.
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

/// **The documents a new session reads first each fit in one read, and together stay under
/// 100 KB.**
///
/// Per-file caps did not bound the start of a session. Measured 2026-09-25, after STATUS and
/// HISTORY had both been cut to one read: the four documents in `STATUS.md` §9 step 1 weighed
/// 110.7 KB together, and one of them — `docs/slices/README.md`, 36.5 KB and growing about
/// 1.2 KB a day — had no guard at all. Grok's review named the gap: a guard per file is not a
/// guard on the whole.
///
/// The set is DERIVED from the step-1 line — `STATUS.md` plus every backticked `.md` on it — so
/// adding a document to the reading order puts it under the budget; a list written here would
/// be the defect that left the slice index out. 45 KB each keeps a file inside one read (the
/// tool pages at 25,000 tokens, about 51.5 KB for this prose); 100 KB for the four together is
/// the user's decision, about a quarter of a 200k-token window. `CLAUDE.md` is loaded into every
/// session regardless, and the user's global rule caps it at 200 lines.
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
/// `docs/slices/README.md` is read at the start of every session, and it grew the way
/// `STATUS.md` had: closed rows kept their narrative. Measured 2026-09-25 — the twelve oldest
/// rows were 133–361 bytes, the twenty newest 502–2,422, and the file had gone from 4.4 KB to
/// 36.5 KB in 27 days. The file cap catches that only after a month of it; a row cap stops it
/// in the first PR that writes a paragraph into a cell. The narrative belongs in the slice's
/// `§9-N` record; the rows collapsed that day are in `docs/history/slices-index.md`.
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
/// Why: the suite was one 205 KB file (2026-09-25), so no session could read the guards it was
/// editing next to. A file under `tests/doc_claims/` that the root does not declare compiles
/// nothing — its guards would vanish without a red.
#[test]
fn the_guard_suite_fits_in_one_read_per_file() {
    const BUDGET: usize = 45_000;
    let root_path = "hosts/rust-oracle/tests/doc_claims.rs";
    let root = read(root_path).replace('\r', "");
    let root_lines: Vec<&str> = root.lines().collect();
    let mut files = vec![(root_path.to_string(), root.clone())];
    if let Ok(entries) = std::fs::read_dir(repo_root().join("hosts/rust-oracle/tests/doc_claims")) {
        for entry in entries {
            let path = entry.expect("a directory entry").path();
            let name = path
                .file_name()
                .expect("a file name")
                .to_string_lossy()
                .into_owned();
            let stem = name.strip_suffix(".rs").unwrap_or_else(|| {
                panic!(
                    "tests/doc_claims/{name}: only this suite's modules live here, as <theme>.rs"
                )
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
    }
    for (path, text) in &files {
        assert!(
            text.len() <= BUDGET,
            "{path} is {} bytes, over {BUDGET}: it no longer opens in one Read. Move a theme's \
             guards into tests/doc_claims/<theme>.rs and declare `mod <theme>;` in {root_path}",
            text.len()
        );
    }
}

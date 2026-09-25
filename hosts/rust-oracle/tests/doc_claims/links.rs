//! Addresses: every citation, PR number, map entry and pointer lands on something that exists.

use super::*;

/// **Every PR number the slice index cites is a PR that exists.**
///
/// Why: a row pointed at a PR that never existed, and the number spread to three documents.
/// The real set comes from `git log`, compared as a set, not a range. (#273)
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

/// **The glossary defines the terms the documents lean on most.**
///
/// Why: it defined nine terms and none of the most used (`DP-` 585 times, 슬라이스 392, …). An
/// explicit list is right here: it is the vocabulary, not a sample of scope. (#272)
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

/// **Every `§9-N` cited anywhere resolves to a heading in the HISTORY index.**
///
/// Why: 46 files cite `§9-N` 150 times, six of them product source comments, and a dropped,
/// renumbered or mangled entry would strand them silently. (#265)
#[test]
fn every_cited_history_entry_has_a_heading() {
    // Headings live on the index pages: `HISTORY.md` and, once it has been rolled,
    // `docs/history/index-NNN.md`. Since the split they are one-line stubs that link
    // `docs/history/9-N.md`; `the_history_archive_is_indexed_and_numbered` checks each has its file.
    let mut headings: Vec<u32> = Vec::new();
    for (_, text, _) in history_index_pages() {
        for line in text.lines() {
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
    }
    // Each number once across every page: a duplicate must not stand in for a lost entry
    // (Grok, 2026-09-25 — the floor below once counted repeats), and a number on two pages is a
    // citation that could bind to either (Grok, on the rollover design).
    let total = headings.len();
    headings.sort_unstable();
    headings.dedup();
    assert_eq!(
        headings.len(),
        total,
        "a `### 9-N.` heading appears twice across HISTORY.md and its rolled pages — each \
         number must have exactly one heading, or a `§9-N` citation can land on the wrong one"
    );
    assert!(
        headings.len() >= 62,
        "docs/HISTORY.md carries {} distinct `### 9-N.` headings, fewer than the 62 that existed \
         in docs/STATUS.md before the move. Entries were lost, not moved",
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

/// **Both READMEs' document map lists every `docs/*.md`.**
///
/// Why: `CLAUDE.md` points at the map instead of carrying a reading order of its own, so a
/// document missing from the map is invisible to every session that follows it. (#285)
#[test]
fn both_readmes_map_every_document() {
    let mut docs: Vec<String> = std::fs::read_dir(repo_root().join("docs"))
        .expect("docs/ is readable")
        .filter_map(|e| e.ok())
        .filter_map(|e| e.file_name().into_string().ok())
        .filter(|n| n.ends_with(".md"))
        .collect();
    docs.sort();
    assert!(
        docs.len() >= 8,
        "read_dir(docs/) returned {docs:?} — the map covers more than that, so this walk is \
         reading the wrong directory and would pass by finding nothing to require"
    );

    for readme in ["README.md", "README.ko.md"] {
        let text = read(readme);
        for doc in &docs {
            let link = format!("](docs/{doc})");
            assert!(
                text.contains(&link),
                "{readme}'s document map does not link docs/{doc}. CLAUDE.md sends every new \
                 session to this map instead of carrying its own reading order, so a document \
                 missing here is a document nobody is told to read — which is how the copy \
                 that map replaced lost docs/STATUS.md."
            );
        }
    }
}

/// **A document that names where the current state lives names `docs/STATUS.md`.**
///
/// Why: `CLAUDE.md` pointed at a document that redirects to STATUS — a hop that goes stale where
/// nobody looks. (#288)
#[test]
fn every_pointer_to_the_current_state_names_status() {
    const CANONICAL: &str = "STATUS.md";
    let mut found = 0usize;

    for (path, text) in every_markdown_file() {
        for (no, line) in text.lines().enumerate() {
            let flat = flatten_prose(line);
            for lead in ["현재 상태는 ", "현재 상태의 정본은 ", "최신 상태의 정본은 "]
            {
                // Every occurrence, not the first — a line may carry the lead twice, and the
                // first one being right must not answer for a later one. Third time this
                // file has needed `match_indices` over `find`; Grok found all three.
                for (at, _) in flat.match_indices(lead) {
                    // The FIRST document named after the lead is the one it points at. Taking
                    // the sentence up to a separator and asking whether STATUS.md appears in
                    // it anywhere is too loose: `…정본은 docs/STATUS.md다. phase의 정의는
                    // docs/ROADMAP.md` has no separator between the two names (an ASCII `.`
                    // is not one), so the whole run would pass on the first name while a
                    // reversed version would pass on the second. Grok raised exactly that.
                    let tail = &flat[at + lead.len()..];
                    let Some(end) = tail.find(".md") else {
                        continue; // names no document: prose, not a pointer
                    };
                    let named: String = tail[..end + ".md".len()]
                        .chars()
                        .rev()
                        .take_while(|c| !c.is_whitespace() && !matches!(c, '(' | '·' | ','))
                        .collect::<Vec<_>>()
                        .into_iter()
                        .rev()
                        .collect();
                    found += 1;
                    assert!(
                        named.ends_with(CANONICAL),
                        "{path}:{} says the current state lives in '{named}', not {CANONICAL}. \
                         docs/STATUS.md is where a new session is told to look first, and the \
                         documents named here send it back there in their own opening lines — \
                         so this pointer costs a hop and goes stale where nobody reads it.",
                        no + 1
                    );
                }
            }
        }
    }

    // Floor: the sentence still exists somewhere. A needle that matches nothing asserts
    // nothing, and this one is prose rather than a derived value, so nothing else would notice.
    assert!(
        found >= 3,
        "found {found} document pointers to the current state; the tree had four when this \
         guard was written, so the sentence has been reworded and the needles no longer see it"
    );
}

/// **Every STATUS §-citation resolves on `STATUS.md` or its closed registry.**
///
/// Why: 150 prose citations fail silently when stranded. A label resolves to a numbered heading
/// or an item of one (`4-2`, `5-5.7`, `9-A`), each page judged on its own. (#310, #318)
#[test]
fn every_status_citation_resolves() {
    let pages = status_pages();
    let page_lines: Vec<Vec<&str>> = pages
        .iter()
        .map(|(_, text)| text.lines().collect())
        .collect();

    // (level, text after the hashes) for an unquoted heading line.
    let heading = |line: &str| -> Option<(usize, String)> {
        let level = line.chars().take_while(|&c| c == '#').count();
        (level >= 2).then(|| (level, line[level..].trim_start().to_string()))
    };
    let resolves_in = |lines: &[&str], label: &str| -> bool {
        // `{label}. ` with the space: without it `5-5.` would also claim a `5-5.7…` heading
        // (Grok). Every numbered heading in STATUS.md is written `N. title`.
        if lines
            .iter()
            .filter_map(|l| heading(l))
            .any(|(_, text)| text.starts_with(&format!("{label}. ")))
        {
            return true;
        }
        let Some((section, item)) = label.rsplit_once('.').or_else(|| label.rsplit_once('-'))
        else {
            return false;
        };
        let Some((start, level)) = lines.iter().enumerate().find_map(|(i, l)| {
            heading(l)
                .filter(|(_, text)| text.starts_with(&format!("{section}. ")))
                .map(|(level, _)| (i, level))
        }) else {
            return false;
        };
        let body = &lines[start + 1..];
        // A numbered item belongs to the section's OWN list, which ends at its first
        // sub-heading. Scanning past it would let `5-7` resolve to item `7.` of `### 5-5.`.
        let own_item = body
            .iter()
            .take_while(|l| heading(l).is_none())
            .any(|l| l.starts_with(&format!("{item}. ")));
        // A lettered sub-heading (`#### A.` for `9-A`) may sit anywhere inside the section.
        let sub_heading = body
            .iter()
            .take_while(|l| heading(l).is_none_or(|(inner, _)| inner > level))
            .any(|l| heading(l).is_some_and(|(_, text)| text.starts_with(&format!("{item}. "))));
        own_item || sub_heading
    };
    let resolves = |label: &str| page_lines.iter().any(|lines| resolves_in(lines, label));

    let mut cited: Vec<(String, String)> = Vec::new();
    for (path, text) in every_file_ending_in(&[".md", ".rs", ".c", ".h", ".dpr", ".mls", ".yml"]) {
        for (at, _) in text.match_indices("STATUS") {
            let rest = &text[at + "STATUS".len()..];
            let rest = rest.strip_prefix(".md").unwrap_or(rest);
            let rest = rest.strip_prefix('`').unwrap_or(rest);
            let rest = rest.strip_prefix(' ').unwrap_or(rest);
            let Some(rest) = rest.strip_prefix('§') else {
                continue;
            };
            // `<digits><optional letter>` then optionally `-<digits or capitals>` then
            // optionally `.<digits>`: 1 · 3a · 3a-9 · 4-12 · 5-5.7 · 9-A.
            let mut label: String = rest.chars().take_while(char::is_ascii_digit).collect();
            if label.is_empty() {
                continue;
            }
            let mut tail = &rest[label.len()..];
            if let Some(c) = tail.chars().next().filter(char::is_ascii_lowercase) {
                label.push(c);
                tail = &tail[1..];
            }
            if let Some(after) = tail.strip_prefix('-') {
                let part: String = after
                    .chars()
                    .take_while(|c| c.is_ascii_digit() || c.is_ascii_uppercase())
                    .collect();
                if !part.is_empty() {
                    label = format!("{label}-{part}");
                    tail = &after[part.len()..];
                }
            }
            if let Some(after) = tail.strip_prefix('.') {
                let part: String = after.chars().take_while(char::is_ascii_digit).collect();
                if !part.is_empty() {
                    label = format!("{label}.{part}");
                }
            }
            if label.starts_with("9-") && label[2..].starts_with(|c: char| c.is_ascii_digit()) {
                continue;
            }
            cited.push((path.clone(), label));
        }
    }
    assert!(
        cited.len() >= 100,
        "found only {} `STATUS §<n>` citations outside the session log; 2026-09-25 measured \
         150, so this scan is reading less than it did",
        cited.len()
    );

    for (path, label) in &cited {
        assert!(
            resolves(label),
            "{path} cites STATUS §{label}, but neither docs/STATUS.md nor its closed registry \
             (docs/history/status-closed-NNN.md) has a heading or numbered item for it. A \
             closed item moves to the registry under the same heading and number — the \
             citation is the address, and it is spread across the tree"
        );
    }
}

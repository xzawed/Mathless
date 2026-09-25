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
//!
//! This file holds the shared helpers; the guards live in `doc_claims/<theme>.rs`, one theme
//! per file, each small enough to open in one Read (`the_guard_suite_fits_in_one_read_per_file`).

use std::path::{Path, PathBuf};

// A crate root looks for `mod x;` next to itself, so each theme names its file.
#[path = "doc_claims/agreement.rs"]
mod agreement;
#[path = "doc_claims/budget.rs"]
mod budget;
#[path = "doc_claims/hygiene.rs"]
mod hygiene;
#[path = "doc_claims/lifecycle.rs"]
mod lifecycle;
#[path = "doc_claims/links.rs"]
mod links;
#[path = "doc_claims/presence.rs"]
mod presence;
#[path = "doc_claims/stale.rs"]
mod stale;

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

/// Whether `text` names `name` as a whole identifier, not as part of a longer one.
/// `MATHLESS_GATE_D` begins `MATHLESS_GATE_DELPHI`, `MATHLESS_GATE_FPC` begins
/// `MATHLESS_GATE_FPC_HOST`, and `len` ends `byte_len`, so a bare `contains` passed on documents
/// that named none of the three (plants, 2026-09-25).
fn names_identifier(text: &str, name: &str) -> bool {
    let ident = |c: char| c.is_ascii_alphanumeric() || c == '_';
    text.match_indices(name).any(|(at, _)| {
        !text[..at].chars().next_back().is_some_and(ident)
            && !text[at + name.len()..].chars().next().is_some_and(ident)
    })
}

/// Every place `needle` starts in `hay`. `windows` tries each start, the last one included —
/// the four scan loops this replaced ran `0..len - n` and never tried a needle that ends the
/// text (Grok found it reviewing the guards; `stale.rs` has the plants).
fn starts<'a>(hay: &'a [char], needle: &'a [char]) -> impl Iterator<Item = usize> + 'a {
    hay.windows(needle.len())
        .enumerate()
        .filter(move |(_, w)| *w == needle)
        .map(|(i, _)| i)
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

/// **Dated records, which present-tense guards skip.** `docs/HISTORY.md` is the index of the
/// session log, and `docs/history/` holds the entries (`9-N.md`), the ▶ block records
/// (`start-block-NNN.md`) and the STATUS sections moved out on 2026-09-25. Each says what was
/// true on its day; rewriting a record to satisfy today's check is the wrong fix.
///
/// One helper rather than six copies of the condition: when the log was split out of
/// `HISTORY.md`, the five guards that exempted that one path by equality began reading the
/// entries under their new names. Measured: `no_document_says_c_is_the_only_gated_host` went red
/// on `history/9-21.md` and `no_document_publishes_a_baseline_that_requires_only_some_gates`
/// on `history/9-26.md` — both written before the Pascal gates existed.
fn is_dated_record(path: &str) -> bool {
    path == "docs/HISTORY.md" || path.starts_with("docs/history/")
}

/// **The pages of the session-log index**, newest stubs first: `docs/HISTORY.md`, then any
/// rolled pages `docs/history/index-NNN.md` from the newest roll down to 001. Each item is
/// `(path, text without '\r', the prefix a stub on that page uses to link `9-N.md`)`.
///
/// Rolling is the procedure for when `HISTORY.md` itself outgrows one read: its OLDEST stubs
/// move, unchanged except the link prefix, into the next `index-NNN.md`, and `HISTORY.md` keeps
/// a link to that page. Nothing has been rolled yet (2026-09-25, 13.5 KB of 45 KB) — this is
/// here so that the day the budget fires, the guards already know where the headings went and
/// `§9-N` citations keep landing on one.
fn history_index_pages() -> Vec<(String, String, &'static str)> {
    let mut rolled: Vec<(u32, String, String)> = Vec::new();
    for (path, text) in every_markdown_file() {
        let Some(stem) = path
            .strip_prefix("docs/history/index-")
            .and_then(|rest| rest.strip_suffix(".md"))
        else {
            continue;
        };
        assert!(
            stem.len() == 3 && stem.bytes().all(|b| b.is_ascii_digit()),
            "{path}: a rolled page of the HISTORY index is named index-NNN.md with three digits"
        );
        rolled.push((
            stem.parse().expect("three digits"),
            path,
            text.replace('\r', ""),
        ));
    }
    rolled.sort_by_key(|page| std::cmp::Reverse(page.0));
    for (i, (n, path, _)) in rolled.iter().rev().enumerate() {
        assert_eq!(
            *n,
            i as u32 + 1,
            "{path}: rolled index pages are numbered 001, 002, … without gaps"
        );
    }
    let mut pages = vec![(
        "docs/HISTORY.md".to_string(),
        read("docs/HISTORY.md").replace('\r', ""),
        "history/",
    )];
    pages.extend(rolled.into_iter().map(|(_, path, text)| (path, text, "")));
    pages
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

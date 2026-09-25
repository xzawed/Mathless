//! Open and closed: slices, SPECs, questions and debts say the state their own index says.

use super::*;

/// **A shipped SPEC does not still ask for the confirmation it already got.**
///
/// Why: six SPECs said 확정 · 구현 완료 above a §4 heading asking for 사용자 확인, which sends an
/// agent following rule 8 to ask again. The DP tables are records and are left alone. (#276)
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

/// **`CLAUDE.md` does not name a closed slice.**
///
/// Why: it listed `배열 반환` as a candidate after it shipped — the fourth time a closed slice
/// stayed a candidate, this time in the file every session loads. (#260)
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

/// **A SPEC the index calls closed says so in its own `상태:` header.**
///
/// Why: eleven SPECs the index marked ✅ had headers saying 구현 대기 or 구현 중. The body is a
/// dated record; the header reads as the current state. (#261)
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

/// **A slice cannot be both closed and a candidate in the slice index.**
///
/// Why: closing a slice adds an index row, and nothing removed it from the candidate list —
/// three times before this guard. (#250)
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

/// **No document calls a question `OPEN_QUESTIONS.md` has closed still open.**
///
/// Why: rule 8 sends the next session to the open questions, so calling a closed one open
/// reopens a decision made with the user. The closed set comes from the entries. (#280)
#[test]
fn no_document_says_a_closed_question_is_open() {
    let oq = read("docs/OPEN_QUESTIONS.md");
    let (mut closed, mut open): (Vec<u32>, Vec<u32>) = (Vec::new(), Vec::new());
    for line in oq.lines() {
        let flat = flatten_prose(line);
        // The entry that OWNS the question. `flatten_prose` has already dropped the `-` or
        // `###` marker, so an entry starts with the number and a period. The period is what
        // separates `Q12.` from `Q1~Q5는 …`, a line that merely talks about questions.
        let Some(rest) = flat.strip_prefix('Q') else {
            continue;
        };
        let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
        if digits.is_empty() || !rest[digits.len()..].starts_with('.') {
            continue;
        }
        let n: u32 = digits.parse().expect("digits");
        if flat.contains("닫힘") || flat.contains("닫혔다") {
            closed.push(n);
        } else {
            open.push(n);
        }
    }

    // Floors: the parse found both kinds, said nothing twice, and left no gaps. A walk that
    // silently stops matching — a bullet style changes, a section moves — otherwise reports
    // an empty `closed` set and this test passes by reading nothing.
    assert!(
        !closed.is_empty() && !open.is_empty(),
        "parsed {} closed and {} open questions from docs/OPEN_QUESTIONS.md; both must be \
         non-empty or the entry parse no longer matches that file's shape",
        closed.len(),
        open.len()
    );
    let highest = closed
        .iter()
        .chain(open.iter())
        .copied()
        .max()
        .expect("max");
    for n in 1..=highest {
        let seen =
            closed.iter().filter(|c| **c == n).count() + open.iter().filter(|o| **o == n).count();
        assert_eq!(
            seen, 1,
            "Q{n} is owned by {seen} entries in docs/OPEN_QUESTIONS.md, not 1 — every number \
             up to Q{highest} must have exactly one entry, or this guard is classifying some \
             question by a line that does not own it"
        );
    }

    for (path, text) in every_markdown_file() {
        for (no, line) in text.lines().enumerate() {
            let flat = flatten_prose(line);
            for n in &closed {
                for needle in [
                    format!("열려 있는 Q{n}"),
                    format!("Q{n}는 열려"),
                    format!("Q{n}은 열려"),
                    format!("Q{n}이 열려"),
                    format!("Q{n}가 열려"),
                    format!("Q{n}를 먼저 닫아야"),
                    format!("Q{n}을 먼저 닫아야"),
                ] {
                    // Every occurrence, not the first. `열려 있는 Q1` is a prefix of
                    // `열려 있는 Q14`, so where the number ends the needle the next character
                    // decides which question was named — and a line may hold both. Grok
                    // raised this verifying the test: `find` returns one index, so a line
                    // reading `열려 있는 Q11과 … 열려 있는 Q1` skipped the Q11 hit and never
                    // looked further, passing a sentence that calls closed Q1 open. Planted
                    // and confirmed green before the fix.
                    let real = flat.match_indices(&needle).any(|(at, _)| {
                        !(needle.ends_with(char::is_numeric)
                            && flat[at + needle.len()..].starts_with(|c: char| c.is_ascii_digit()))
                    });
                    if !real {
                        continue;
                    }
                    panic!(
                        "{path}:{} says '{needle}', but docs/OPEN_QUESTIONS.md closed Q{n}. \
                         Rule 8 sends the next session to the open questions, so a closed one \
                         named as open is an instruction to reopen a decision the user already \
                         made. If the sentence records why a slice stopped there on the day it \
                         was written, put it in the past tense — 당시 열려 있던 — which is what \
                         makes it a record.",
                        no + 1
                    );
                }
            }
        }
    }
}

/// **The §5-6 record quotes the exact shape `codegen.rs` emits for `i32 /` and `%`.**
///
/// Why: the first version pinned four fragments, and the slice that changed the shape kept all
/// four, so it stayed green. It pins the whole template now. (#281)
#[test]
fn the_recorded_division_debt_still_matches_what_codegen_emits() {
    // The emitter writes this as a `format!` template, so the source carries doubled braces.
    // Undoubling them turns the template back into the text it produces, which is the thing
    // §5-6 quotes — matching the template's own spelling instead would pin an escaping
    // detail rather than the shape.
    let codegen = read("compiler/src/codegen.rs")
        .replace("{{", "{")
        .replace("}}", "}");

    // The template with its placeholders still in, exactly as §5-6 prints it. `{}` is what
    // `format!` leaves for the operands; the document writes `<lhs>`/`<rhs>` in their place,
    // so the two are compared after the same substitution.
    let emitted = codegen
        .lines()
        .map(str::trim)
        .find(|l| l.contains("if __d == 0 { 0i32 } else {"))
        .map(|l| l.trim_matches(|c| c == '"' || c == ',').to_string())
        .expect(
            "compiler/src/codegen.rs no longer emits an `if __d == 0 { 0i32 } else { … }` \
             guard for i32 division. docs/STATUS.md §5-6 quotes that shape; if it is gone, \
             that section is describing code that is gone",
        );
    assert!(
        emitted.contains("let __l =") && emitted.find("let __l =") < emitted.find("let __d ="),
        "the i32 division guard no longer binds the dividend before the divisor. That \
         ordering is what SPEC-division-guard-operands closed §5-6 with, and \
         compiler/tests/division_guard.rs is where the reason lives:\n  {emitted}"
    );

    // Three placeholders, in emission order: dividend, divisor, method. The document writes
    // the operands as `<lhs>`/`<rhs>` and the method out in full, so the comparison is made
    // after the same substitution rather than by loosening either side.
    let shape = emitted
        .replacen("{}", "<lhs>", 1)
        .replacen("{}", "<rhs>", 1)
        .replacen("{}", "wrapping_div", 1);
    // §5-6 is paid, so its record lives in the closed registry now; read it wherever it is.
    let record: String = status_pages()
        .into_iter()
        .map(|(_, text)| text)
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        record.contains("5-6."),
        "neither docs/STATUS.md nor its closed registry carries §5-6. The record of what this \
         shape is for does not go away when the debt is paid — it moves to the registry"
    );
    assert!(
        record.contains(&shape),
        "the §5-6 record (docs/STATUS.md or docs/history/status-closed-NNN.md) does not quote \
         the shape codegen.rs emits.\n  emitted: {shape}\nUpdate the code block there in the \
         same commit that changes the emitter — that is what this guard is for, and the \
         version of it that pinned fragments instead of the whole template missed exactly this \
         change."
    );
}

/// **A SPEC may not call a STATUS §5 debt open after the register marks it paid.**
///
/// Why: four SPECs still called §5-1 open after #200 paid it. The needles pick claims about the
/// debt, not about the slice (measured 4 of 4 real); SPECs only. (#282)
#[test]
fn no_spec_calls_a_paid_debt_open() {
    // STATUS holds the open debts and its closed registry the paid ones, under the same
    // headings, so every page is read (`status_pages`).
    let pages = status_pages();
    // Both halves of the register: §5's own numbered list, cited as `§5-N`, and the
    // sub-register §5-5 ("미추적 부채"), cited as `§5-5.N`. The needles find nothing in the
    // second half today; it is read anyway because which half a debt lands in is not
    // something this guard should have an opinion about.
    let mut paid: Vec<String> = Vec::new();
    let mut unpaid: Vec<String> = Vec::new();
    for (open_at, prefix) in [("## 5. ", "5-"), ("### 5-5. ", "5-5.")] {
        let before = paid.len() + unpaid.len();
        for (_, status) in &pages {
            let mut inside = false;
            for line in status.lines() {
                if line.starts_with(open_at) {
                    inside = true;
                    continue;
                }
                if inside && (line.starts_with("## ") || line.starts_with("### ")) {
                    break;
                }
                if !inside {
                    continue;
                }
                let t = line.trim_start();
                let digits: String = t.chars().take_while(char::is_ascii_digit).collect();
                if digits.is_empty() || !t[digits.len()..].starts_with(". ") {
                    continue;
                }
                let key = format!("{prefix}{digits}");
                if t.contains('✅') {
                    paid.push(key);
                } else {
                    unpaid.push(key);
                }
            }
        }

        // Floor, per register rather than over the union. The first version asserted only
        // that `paid` and `unpaid` were non-empty overall, and a plant showed what that
        // costs: renaming one heading dropped that whole register and the other one kept the
        // totals non-zero, so the guard went on scanning for half the debts and passed.
        assert!(
            paid.len() + unpaid.len() > before,
            "neither docs/STATUS.md nor its closed registry has numbered items under a heading \
             starting `{open_at}` — the debt register moved or was renamed, and this guard is \
             now reading nothing there"
        );
    }

    // And both verdicts exist somewhere, so a register that lost its ✅ marks is visible too.
    assert!(
        !paid.is_empty() && !unpaid.is_empty(),
        "parsed {} paid and {} unpaid debts from docs/STATUS.md and its closed registry; both \
         must be non-empty or the register no longer distinguishes them",
        paid.len(),
        unpaid.len()
    );

    for (path, text) in every_markdown_file() {
        if !path.starts_with("docs/slices/SPEC-") {
            continue;
        }
        for (no, line) in text.lines().enumerate() {
            let flat = flatten_prose(line);
            // The repository's own correction forms. Same line, so one debt, one verdict.
            if line.contains("~~")
                || flat.contains('✅')
                || flat.contains("갚혔다")
                || flat.contains("닫혔다")
            {
                continue;
            }
            for key in &paid {
                // Every occurrence, not the first. `§5-5` is a prefix of `§5-5.4` and `§5-1`
                // of `§5-10`, and those are different debts — so a citation is only real when
                // the key is not continued by a digit or a sub-number. Taking `find`'s single
                // index meant a line that continued the first match abandoned the key, and a
                // later real citation on the same line went unread. This is the second time
                // the same `find`-first hole appeared in this file; Grok found both, and the
                // first (#280) is where the shape is described.
                let cite = format!("§{key}");
                let real = flat.match_indices(&cite).any(|(at, _)| {
                    !flat[at + cite.len()..].starts_with(|c: char| c == '.' || c.is_ascii_digit())
                });
                if !real {
                    continue;
                }
                for needle in [
                    "열린 채",
                    "그대로다",
                    "여전히 없다",
                    "관측 수단이 없다",
                    // Added 2026-09-23 after §5-6 was paid: two SPECs described it as
                    // 미해결 and this guard did not know the word. Measured when added —
                    // zero hits either way, so it exists for the NEXT copy, not this one.
                    "미해결",
                    "아직 열려",
                    "미상환",
                ] {
                    assert!(
                        !flat.contains(needle),
                        "{path}:{} cites §{key} and says '{needle}', but docs/STATUS.md marks \
                         that debt ✅ 갚음. Saying a slice does not pay a debt stays true \
                         forever; saying the debt is still open expires the day it is paid, \
                         and a SPEC's debt list carries no date to read it by. Strike the line \
                         through and add what paid it.",
                        no + 1
                    );
                }
            }
        }
    }
}

/// **`STATUS.md` holds only what is open; its closed registry holds only what is closed.**
///
/// Why: closed ✅ lines made STATUS grow with the project's age (14.6 of 40.5 KB). A closed
/// item moves to `docs/history/status-closed-NNN.md` under the same number; quoted lines (the ▶
/// block) are skipped. (#318)
#[test]
fn status_holds_no_closed_item() {
    let numbered = |t: &str| {
        t.split_once(". ")
            .is_some_and(|(n, _)| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
    };
    // A table data row: not the `| # |` header, and not a separator, which is only pipes,
    // dashes, colons and spaces however it is spaced (`|---|` and `| --- |` alike — Grok).
    let row = |t: &str| {
        t.starts_with('|')
            && !t.starts_with("| # ")
            && !t.chars().all(|c| matches!(c, '|' | '-' | ':' | ' '))
    };
    let mut wrong = Vec::new();
    for (i, (path, text)) in status_pages().iter().enumerate() {
        for (no, line) in text.lines().enumerate() {
            if line.starts_with('>') {
                continue;
            }
            let t = line.trim_start();
            let item = numbered(t) || row(t);
            let closed = line.contains('✅');
            if i == 0 && closed && (item || t.starts_with('#') || t.starts_with("- ")) {
                wrong.push(format!(
                    "{path}:{} is closed (✅) but still in STATUS. Move it byte-identical to \
                     docs/history/status-closed-NNN.md under the same heading — its §N address \
                     keeps resolving there",
                    no + 1
                ));
            }
            if i > 0 && item && !closed {
                wrong.push(format!(
                    "{path}:{} is not marked ✅, but it is in the closed registry. Open work \
                     belongs in docs/STATUS.md",
                    no + 1
                ));
            }
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

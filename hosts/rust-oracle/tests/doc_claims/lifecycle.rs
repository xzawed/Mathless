//! Open and closed: slices, SPECs, questions and debts say the state their own index says.

use super::*;

/// **A shipped SPEC does not still ask for the confirmation it already got.**
///
/// Six SPECs carried a §4 heading reading *미확정, 사용자 확인 필요* while their own line 3
/// said 확정 · 구현 완료. That is not a cosmetic mismatch: `CLAUDE.md` rule 8 makes "find the
/// DP items awaiting user confirmation" a procedure, and an agent following it reads these
/// headings and goes asking for a confirmation that was given — in one case over three weeks
/// earlier, for a feature that has been in the compiler since.
///
/// Wholly derivable from inside each file: the status line says whether the slice shipped, and
/// the DP heading says whether its decisions are open. Those two cannot both be true.
///
/// The DP TABLES are left alone. They are the record of what was decided and why, and this
/// guard says nothing about them — only about a heading that describes them as pending.
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

/// **`CLAUDE.md` does not name a closed slice.** It is the file every session loads.
///
/// `docs/slices/README.md` has corrected "a closed slice is still listed as a candidate"
/// three times, and next to the third it writes the cause: closing a slice adds a row to the
/// index — *`CLAUDE.md` puts that in the procedure* — and **nothing removes it from the
/// candidate list**. The file it names by name was the fourth occurrence. Rule 3 listed
/// `배열 반환` as a future candidate while the index row read ✅ 구현 완료 and the compiler had
/// been emitting `RetAbi::ArrayOut` since #206; rule 2, one line above it, forbids reopening a
/// closed slice.
///
/// The sibling guard below bounds its scan to a named section. **This one scans the whole
/// file, and the reason is measured, not stylistic**: of the 29 closed index titles, exactly
/// one appeared anywhere in `CLAUDE.md`, and it was the defect. With no legitimate mention to
/// carve out there is no sentence boundary to parse — and a whole-file scan cannot quietly
/// stop failing the way a literal section marker does when somebody renames a heading.
///
/// `docs/STATUS.md` and `docs/phase1/WBS.md` are deliberately NOT here. The same measurement
/// found 10 and 7 titles in them, and every one is a closure record — a `✅ 닫힘` section
/// heading or a completed WBS row. Pointing this guard at those files would make it a
/// false-positive machine, which is how a guard gets deleted.
///
/// If a rule ever genuinely needs to name a closed slice, this test failing is the right
/// outcome: it makes the author say so here, with a reason, instead of leaving a candidate.
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

/// **A SPEC the index calls closed says so in its own header.**
///
/// The index guard above opens `docs/slices/README.md` and `read_dir`s the SPEC names. It
/// never opens a SPEC. So the row and the file it links could disagree for as long as nobody
/// happened to read both, and they did: eleven SPECs the index marked ✅ 구현 완료 carried a
/// header saying 구현 대기 / 구현 중 / 구현 착수, one said *"구현은 시작하지 않았다"* about a
/// feature wired through every stage of the compiler, and three named no implementation state
/// at all.
///
/// `docs/slices/README.md` disclaims that each SPEC is "a design record at the time of
/// writing", and for the BODY that is right — a §5 that says "BLOCKED" is a fact about that
/// day and rewriting it would be rewriting history. **The `상태:` header is not body.** It is
/// the first thing in the file, it has no date attached to the implementation clause, and it
/// reads as the current state. Grok was asked whether the disclaimer covers it and answered
/// NOT-COVERED, for the reason the disclaimer itself gives: it immunises then-facts and sends
/// live status to `docs/STATUS.md`, never recasting the header as historical.
///
/// **Allowlist, not denylist**, and the difference is which way it breaks. A denylist of
/// 구현 대기 / 구현 중 / 구현 착수 would pass the three headers that name no state at all, and
/// would pass the next spelling somebody invents. Requiring the done-token means a SPEC that
/// invents a phrasing goes red and someone has to look. That is the direction this repository
/// has already chosen for every other guard it kept.
///
/// Bounded to the first 30 lines because that is where every header is today (30 files put it
/// on line 3; `SPEC-iface-hash.md` on line 24), and because the bound is what makes a DELETED
/// header fail. Searching the whole file would let a stray later 상태: line answer for a
/// header that is gone.
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

/// **A slice cannot be both closed and a candidate**, in the file that says so about itself.
///
/// `docs/slices/README.md` carries two lists: an index of closed slices at the top and
/// "next slices (no SPEC yet)" at the bottom. Closing a slice adds a row to the first —
/// `CLAUDE.md` puts that in the procedure. **Nothing removes it from the second**, and that
/// file has corrected the resulting lie three times: 문자열 연결 (2026-09-03), 배열 입력
/// (2026-09-11), 배열 반환 (2026-09-15). The third one had been wrong for three days while the
/// row twelve lines above it read ✅ 구현 완료.
///
/// It is the shape `STATUS.md` §7-1 records as this repository's most repeated failure — a
/// document calling something open that the same document calls closed — and a human reading
/// the candidate list to pick the next slice is exactly who it misleads.
///
/// Matching is on the index row's OWN title text, stripped of `**`. That is what a copy into
/// the candidate list looks like, and it is what happened all three times.
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

/// **No document says a question `docs/OPEN_QUESTIONS.md` has closed is still open.**
///
/// Rule 8 in `CLAUDE.md` tells the next agent to find the open questions and get user
/// confirmation before touching them. A document that calls a *closed* question open is
/// therefore not a stale sentence but an instruction: it sends the next session to reopen a
/// decision this repository already made with the user. That is the same failure `#276` fixed
/// for SPEC headings, one layer up — there the file asked for confirmation it already had,
/// here it names the question that confirmation closed.
///
/// **The set is derived, never listed.** `docs/OPEN_QUESTIONS.md` owns one entry per
/// question — `- Q12.` in the deferred list, `### Q1.` in the closed section — and an entry
/// is closed when its own entry line says 닫힘 / 닫혔다. Deriving it is not decoration: a
/// hand-written list drifts the moment a question closes, which is the defect this guard is
/// about. The first cut of this derivation scanned every line rather than the owning entry
/// and marked Q6 and Q11 closed, because the summary line that names `Q6~Q11` as *remaining*
/// also carries 닫혔다 about Q12–Q15. A guard that believes an open question is closed goes
/// quiet exactly where it is needed.
///
/// **Present tense only.** The needles are the live spellings — 열려 있는 Qn, Qn는 열려 있다,
/// Qn를 먼저 닫아야 한다. A decision table recording why a slice stopped at a boundary that
/// existed *that day* is a record, and the repository keeps those; writing it in the past
/// tense (당시 열려 있던) is what makes it a record rather than a claim, and the past tense
/// evades these needles without an allowlist. An allowlist would have been the other option
/// and a worse one: "the line also says 닫혔다 somewhere" passes a line that says both about
/// two different questions.
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

/// **`STATUS.md` §5-6 quotes an emitted shape; this fails when that shape moves.**
///
/// §5-6 recorded the defect: `i32 /` and `%` put their LEFT operand inside the `else` of the
/// totality guard `#76` added, so a zero divisor skipped it and an out-of-range index came
/// back as status 0. `SPEC-division-guard-operands` closed it by binding the dividend first
/// and outside the `else`, and §5-6 now quotes the shape that replaced it.
///
/// **The first version of this guard did not do what its own docstring promised**, and the
/// slice that changed the shape is what proved it. It pinned four fragments — `let __d =`,
/// `if __d == 0 { 0i32 } else {`, `wrapping_div`, `wrapping_rem` — and the new emission
/// contains all four, so the shape moved and the guard stayed green. The plant that had
/// "proved" it worked inverted the condition, which is not the change that happened.
///
/// It now pins the WHOLE template, both halves of it, against the document that quotes it:
/// the emitted text and §5-6's code block have to be the same string. That is the only form
/// where "the shape moved" and "this test fails" are the same statement — a fragment list is
/// a guess about which part will move.
///
/// Deliberately not a behavioural test: `division_guard.rs` and `division_guard_operands.rs`
/// own the behaviour. This owns only the agreement between the source and the document that
/// describes it.
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

/// **A SPEC may not call a `STATUS.md` §5 debt open after §5 marks it paid.**
///
/// §5 is the debt register — "조용히 사라지면 안 되는 것" — and its numbered items carry ✅
/// 갚음 once paid. Twelve slices cite §5-1 while explaining what they do and do not pay, which
/// is exactly what the register is for. Four of them said the debt itself was still open:
/// 열린 채다 / 그대로다 / 여전히 없다 / 관측 수단이 없다. #200 paid it on 2026-09-13.
///
/// **The distinction the needles draw is real and was measured.** *"이 슬라이스는 §5-1을 갚지
/// 않는다"* is a statement about the slice and stays true forever; *"§5-1은 열린 채다"* is a
/// statement about the debt and expires the day it is paid. Scanning for the citation alone
/// found 29 lines, of which 25 were the first kind. The needles below select the second kind:
/// measured 4 hits, 4 real, 0 false.
///
/// **Scope is `docs/slices/SPEC-*.md`, and the reason is the same one #261 used.** Entries in
/// `STATUS.md` §9 and `HISTORY.md` are dated records of what was true that day, and three of
/// them say §5-1 was open because it was. A SPEC's §5 debt list carries no date and reads as
/// current. Including the records would have added 4 false positives for no true one.
///
/// A line that corrects itself in place — struck through, or carrying ✅ / 갚혔다 / 닫혔다 —
/// is the repository's own correction form and passes. That is an allowlist and it is narrow
/// on purpose: it is scoped to one line that already names one debt, so it cannot pass a line
/// that says both things about two different debts.
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

/// **`docs/STATUS.md` holds only what is open; its closed registry holds only what is closed.**
///
/// Why: closed items stayed in STATUS as ✅ lines so their `§N` addresses kept resolving, which
/// made STATUS grow with the project's age rather than its open work (14.6 KB of 40.5 KB on
/// 2026-09-25). A closed item now moves to `docs/history/status-closed-NNN.md` under the same
/// heading and number; `every_status_citation_resolves` reads both. Quoted lines are skipped —
/// the ▶ block is a dated entry that rotates out on its own.
#[test]
fn status_holds_no_closed_item() {
    let numbered = |t: &str| {
        t.split_once(". ")
            .is_some_and(|(n, _)| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
    };
    let row = |t: &str| t.starts_with("| ") && !t.starts_with("| # ") && !t.starts_with("|--");
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

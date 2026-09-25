//! Wherever a document states a code-derived fact, it states the current value.

use super::*;

/// **A message a host README quotes is a message that host can print.**
///
/// `hosts/c-host-link/README.md` quoted the refusal as `refuse: interface …`. The host prints
/// `refuse discount: interface …` — the module name is in there, and it is in there for a
/// reason: that host links TWO modules and the whole point of #215 was that it can now name
/// which one drifted. The README was quoting the message from before that slice.
///
/// A phantom transcript is worse than a stale sentence. A reader who greps the source for the
/// line they were shown finds nothing and concludes the check is not there.
///
/// Scope and needle are both measured. Spans are taken from backticks and filtered to the ones
/// that look like program output — `refuse…`, `GATE_…`, `ok …` — and checked against the
/// host's own source AND its harness, because the gate banners are printed by the test, not by
/// the host. Checking the host source alone reported six false misses; adding the harness left
/// exactly one, and that one was the defect.
///
/// The probe stops at the first `…`, so a README may elide the varying half of a format
/// string without defeating the check.
#[test]
fn every_message_a_host_readme_quotes_exists_in_that_host() {
    let harness_dir = repo_root().join("hosts").join("rust-oracle").join("tests");
    for (dir, sources) in [
        ("c-host", vec!["hosts/c-host/host.c", "c_host.rs"]),
        (
            "c-host-link",
            vec!["hosts/c-host-link/host.c", "c_host.rs", "link_host.rs"],
        ),
        (
            "delphi-host",
            vec![
                "hosts/delphi-host/host.dpr",
                "delphi_host.rs",
                "fpc_units.rs",
            ],
        ),
    ] {
        // `.github/workflows/ci.yml` is in every blob because a host README legitimately
        // quotes the gate setting that drives it (`MATHLESS_GATE_FPC_HOST: require`), and
        // ci.yml is where CLAUDE.md says that list is canonical. It adds no risk of a false
        // pass: the one real defect below is a `refuse …` string, which appears in no yml.
        let mut blob = read(".github/workflows/ci.yml");
        for s in &sources {
            let p = if s.contains('/') {
                repo_root().join(s)
            } else {
                harness_dir.join(s)
            };
            if let Ok(t) = std::fs::read_to_string(&p) {
                blob.push_str(&t);
            }
        }
        assert!(
            blob.len() > 1000,
            "hosts/{dir}: read almost nothing from {sources:?}, so this guard would pass by \
             having no text to search"
        );

        let readme = read(&format!("hosts/{dir}/README.md"));
        let mut checked = 0usize;
        for span in readme.split('`').skip(1).step_by(2) {
            if span.contains('\n') {
                continue;
            }
            let looks_printed =
                span.starts_with("refuse") || span.contains("GATE_") || span.starts_with("ok ");
            if !looks_printed {
                continue;
            }
            let probe = span.split('…').next().unwrap_or(span).trim();
            if probe.is_empty() {
                continue;
            }
            checked += 1;
            assert!(
                blob.contains(probe),
                "hosts/{dir}/README.md quotes `{span}`, but no such text appears in {sources:?}. \
                 A reader who greps for the line they were shown finds nothing and concludes \
                 the check is absent"
            );
        }
        assert!(
            checked >= 2,
            "hosts/{dir}/README.md: only {checked} quoted messages matched the filter, fewer \
             than the two known to be there — the filter stopped seeing them"
        );
    }
}

/// **"Run what that job runs" runs what that job runs.**
///
/// `CONTRIBUTING.md` tells a contributor to reproduce CI locally and then gives a command that
/// sets ONE gate. CI requires three. On a machine without Free Pascal the other two **skip**,
/// the suite prints green and exits 0, the contributor pushes, and CI goes red — after review
/// time has already been spent.
///
/// The block itself is the only thing that made this invisible: a skipped gate is loud on
/// stdout and silent in the exit code, which is the exact failure mode the paragraph
/// underneath that block warns about for pipes. It warned about the wrapper and not about
/// itself.
///
/// Derived from `ci.yml`, so it cannot drift the way a hand-copied list does: every
/// `MATHLESS_GATE_*: require` there must appear in the code fence under the "run what that job
/// runs" heading. Adding a fourth required gate to CI without telling contributors turns this
/// red.
///
/// `MATHLESS_GATE_DELPHI` is deliberately NOT implied by this: it is not `require` in ci.yml,
/// because the runner has no Delphi. The guard asks only for parity with what CI actually
/// enforces.
#[test]
fn the_local_command_block_runs_every_gate_ci_requires() {
    let ci = read(".github/workflows/ci.yml");
    let mut required: Vec<String> = Vec::new();
    for line in ci.lines() {
        let t = line.trim();
        let Some((name, value)) = t.split_once(':') else {
            continue;
        };
        if name.starts_with("MATHLESS_GATE_") && value.trim() == "require" {
            required.push(name.to_string());
        }
    }
    required.sort();
    required.dedup();
    assert!(
        required.len() >= 3,
        "only {} required gates parsed out of ci.yml ({required:?}) — fewer than the three \
         known to be required, so this guard would demand almost nothing",
        required.len()
    );

    let contributing = read("CONTRIBUTING.md");
    let fences: Vec<&str> = contributing.split("```").skip(1).step_by(2).collect();
    let blocks: Vec<&&str> = fences
        .iter()
        .filter(|f| f.contains("cargo test --workspace"))
        .collect();
    assert!(
        !blocks.is_empty(),
        "CONTRIBUTING.md has no fenced block running `cargo test --workspace` — the guard can \
         no longer find the command it checks"
    );

    for block in &blocks {
        for gate in &required {
            assert!(
                block.contains(gate.as_str()),
                "a CONTRIBUTING.md command block runs `cargo test --workspace` without setting \
                 {gate}, which .github/workflows/ci.yml requires. On a machine missing that \
                 toolchain the gate SKIPS, the suite exits 0, and CI is the one that says no"
            );
        }
    }
}

/// **No document states a Rust version that is not the pinned one.**
///
/// The global rule puts dependency versions in the build file and nowhere else. This
/// repository copies the pin into five documents instead — `CONTRIBUTING.md`, both READMEs,
/// `docs/STATUS.md`, `docs/phase1/WBS.md` — and nothing compared any of them to
/// `rust-toolchain.toml`. All five are right today, which is the whole problem: raising the
/// pin makes five documents false at once and a person is the only thing that would notice.
///
/// Same shape as the abi-constant guard: the source states the value, the documents may
/// repeat it, and repeating it wrongly fails. That is the form the global rule allows — "put
/// the number in a machine-checkable form or do not put it in a document".
///
/// Matches any `1.<minor>.<patch>` so a document cannot escape by being stale in a way the
/// needle does not expect; versions that are not Rust releases live in other shapes here
/// (`ml-iface/1`, SDK `10.0.20348`) and do not match.
#[test]
fn no_document_states_a_rust_version_other_than_the_pin() {
    let toml = read("rust-toolchain.toml");
    let pin = toml
        .lines()
        .find_map(|l| {
            let t = l.trim();
            let rest = t.strip_prefix("channel")?.trim_start().strip_prefix('=')?;
            Some(rest.trim().trim_matches('"').to_string())
        })
        .expect("rust-toolchain.toml states a channel");
    assert!(
        pin.starts_with("1."),
        "rust-toolchain.toml's channel is `{pin}`, which is not a numbered release — this \
         guard compares documents against a version number and has nothing to compare"
    );

    let mut seen = 0usize;
    for (path, text) in every_markdown_file() {
        if is_dated_record(&path) {
            continue;
        }
        let bytes: Vec<char> = text.chars().collect();
        for (idx, _) in text.match_indices("1.") {
            let tail: String = text[idx..].chars().take(12).collect();
            let mut parts = tail.splitn(3, '.');
            let (Some(_), Some(minor), Some(rest)) = (parts.next(), parts.next(), parts.next())
            else {
                continue;
            };
            let patch: String = rest.chars().take_while(char::is_ascii_digit).collect();
            if minor.len() < 2 || !minor.chars().all(|c| c.is_ascii_digit()) || patch.is_empty() {
                continue;
            }
            // A digit immediately before means this is the tail of a longer number.
            let before = text[..idx].chars().next_back();
            if before.is_some_and(|c| c.is_ascii_digit() || c == '.') {
                continue;
            }
            let found = format!("1.{minor}.{patch}");
            if !(found.starts_with("1.8") || found.starts_with("1.9")) {
                continue;
            }
            seen += 1;
            assert_eq!(
                found, pin,
                "{path} states Rust {found}, but rust-toolchain.toml pins {pin}. The pin is \
                 the source; a document may repeat it, and repeating it wrongly is what this \
                 catches"
            );
        }
        let _ = &bytes;
    }
    assert!(
        seen >= 5,
        "only {seen} Rust version mentions were found across the documents, fewer than the \
         five known to exist — the scan stopped seeing them and would pass by checking nothing"
    );
}

/// **No document states a value for an `abi.rs` constant that `abi.rs` does not state.**
///
/// This file's own header says the repository watched a measured number drift three times
/// and that until it existed nothing checked the numbers. It still did not check THESE
/// numbers, and the hole was measured rather than argued: lowering `ML_MAX_MODULE_NAME`
/// from 64 to 48 left the whole workspace — 516 tests, four gates required — green, while
/// three documents went on saying 64 in the present tense, one of them in the very table
/// cell that reads "**문서에 숫자를 적지 않는다**".
///
/// The rule the global instructions give is "put the number in a machine-checkable form or
/// do not put it in a document". This is that form: the document may carry the number, and
/// the source decides whether it is still true.
///
/// **A line that also states the true value passes.** A decision table that shows a rejected
/// alternative beside the chosen one (`SPEC-string-return` §T6 weighs `-122` against `-1`)
/// is not drift — it is the record of a choice, and erasing it would cost more than it saves.
#[test]
fn no_document_states_a_stale_value_for_an_abi_constant() {
    let constants = abi_constants();
    assert!(
        constants.len() >= 4,
        "compiler/src/abi.rs declared {} integer constants — it had four, so either the \
         parse broke or the constants moved, and in both cases this guard is now checking \
         nothing",
        constants.len()
    );

    let mut stale = Vec::new();
    for (path, text) in every_markdown_file() {
        for (n, line) in text.lines().enumerate() {
            for (name, value) in &constants {
                let found = assigned_on_line(line, name);
                // The line states the truth somewhere on it; anything else beside it is a
                // rejected alternative, not a stale claim.
                if found.is_empty() || found.contains(value) {
                    continue;
                }
                for wrong in found {
                    stale.push(format!(
                        "{path}:{} says `{name} = {wrong}` — compiler/src/abi.rs says {value}",
                        n + 1
                    ));
                }
            }
        }
    }

    assert!(
        stale.is_empty(),
        "a document states a value the source no longer holds. The source is \
         compiler/src/abi.rs; fix the document, or drop the number and let the source say \
         it:\n{}",
        stale.join("\n")
    );
}

/// **A document that enumerates the export set as complete must name all of it.**
///
/// The set moved 2 → 3 on 2026-09-02 when the fingerprint slice added a second reserved
/// symbol. Two guards already check that number — this file checks the two READMEs, and
/// `protection.rs` checks `SECURITY.md` and `STATUS.md` because reading it needs a built
/// module. Both carry a hand-written list of documents, and twelve lines in ten other files
/// still say the export table is `mlx_<fn>` + `ml_module_abi_version` and nothing else.
///
/// That is the failure the README guard already wrote down about itself: *"a guard's SCOPE is
/// a claim too"*, after `STATUS.md` sat outside it saying 로드하는 모듈 15개 while `host.c`
/// loaded 18 — the two documents inside the scope were corrected by the guard and the one
/// outside simply kept lying. So this one has no list: it walks every markdown file.
///
/// **Detector, then requirement.** A line is enumerating the export table when it names both
/// fixed halves — the reserved `ml_module_abi_version` and D18's user prefix `mlx_` — and
/// claims the enumeration is complete (정확히 / 만 / 둘뿐 / 2개). Only then must it also name
/// the third. Both halves of the detector are asserted to be in what `protection.rs` pins, so
/// a rename moves the detector instead of silently emptying it.
///
/// **The completeness marker is what makes this safe on frozen records.** A slice's §3 wrote
/// its acceptance criterion as an exact set on the day it shipped, and the correction is one
/// clause naming what joined — after which the line names all three and passes. Lines that
/// merely mention the two names without claiming the set is complete are untouched: measured,
/// that is `DECISIONS.md` D18 (which says how the ABI version is exposed, not what else is)
/// and `SPEC-calls.md:78`.
#[test]
fn no_document_enumerates_the_export_set_without_the_fingerprint() {
    let protection = read("hosts/rust-oracle/tests/protection.rs");
    let pinned: Vec<String> = string_literals_in_vec_after(&protection, "        exports,")
        .iter()
        .map(|s| placeholder_to_module(s))
        .collect();
    assert!(
        pinned.len() >= 3,
        "recovered {pinned:?} from protection.rs's export assertion — has its shape changed?"
    );

    // The two fixed halves of the detector, and the third name it then requires. Everything
    // here comes out of `pinned`; nothing is spelled twice.
    let reserved_version = "ml_module_abi_version";
    let user_prefix = "mlx_";
    let fingerprint: String = pinned
        .iter()
        .find(|n| n.starts_with("ml_iface_hash"))
        .map(|n| {
            n.split('<')
                .next()
                .unwrap_or(n)
                .trim_end_matches('_')
                .to_string()
        })
        .expect("protection.rs no longer pins a fingerprint export");
    for part in [reserved_version, user_prefix] {
        assert!(
            pinned.iter().any(|n| n.starts_with(part)),
            "protection.rs pins {pinned:?}, which contains no name starting `{part}` — the \
             detector below would then match nothing and this guard would pass by reading \
             nothing"
        );
    }

    for (path, text) in every_markdown_file() {
        for (no, line) in text.lines().enumerate() {
            let flat = flatten_prose(line);
            if !flat.contains(reserved_version) || !flat.contains(user_prefix) {
                continue;
            }
            if flat.contains(&fingerprint) {
                continue;
            }
            for marker in ["정확히", "만 노출", "둘뿐", "뿐이다", "2개", "만;", "만."]
            {
                // `2개` needs a digit boundary. Grok raised it verifying this guard and the
                // plant confirmed it: a line reading `재시도 32개` beside the two names failed
                // as though it had claimed the export set was two.
                let claimed = flat.match_indices(marker).any(|(at, _)| {
                    !marker.starts_with(|c: char| c.is_ascii_digit())
                        || !flat[..at].ends_with(|c: char| c.is_ascii_digit())
                });
                assert!(
                    !claimed,
                    "{path}:{} enumerates the export table as complete ('{marker}') but names \
                     only {reserved_version} and {user_prefix}*. Every module also exports \
                     {fingerprint}_<module>, which joined on 2026-09-02, and protection.rs \
                     pins the set as {pinned:?}. If the line records a measurement from \
                     before that, say so in the line and name what joined — a reader applying \
                     this criterion today judges a correct module as violating it.",
                    no + 1
                );
            }
        }
    }
}

/// **A documented baseline that requires one gate must require all of them.**
///
/// `#277` fixed this in `CONTRIBUTING.md` and put the guard there by name. Two lines outside
/// that name still published `MATHLESS_GATE_D=require cargo test --workspace --locked` as the
/// baseline — including step 2 of `STATUS.md` §9, which is the first command a new session
/// runs. A skipped gate is not a passed gate: the suite is green with `MATHLESS_GATE_FPC` and
/// `MATHLESS_GATE_FPC_HOST` unset, so a session following that step verifies the C host and
/// nothing on the Pascal side.
///
/// Same lesson as the export guard above, one file over: a guard that names the document it
/// checks has made a claim about scope, and the copies outside it keep their own counsel.
///
/// **The detector is what keeps this quiet on prose and records.** A line only has to name
/// every gate once it requires at least one — a line that mentions the command without
/// setting a gate is prose, and `CONTRIBUTING.md`'s blocks put the variables on their own
/// lines, which `#277`'s guard reads as a block and this one leaves alone. `docs/HISTORY.md`
/// is exempt for #261's reason: its entries are dated records of runs that really did set one
/// gate, and rewriting them would be rewriting what happened.
#[test]
fn no_document_publishes_a_baseline_that_requires_only_some_gates() {
    let ci = read(".github/workflows/ci.yml");
    let required: Vec<String> = ci
        .lines()
        .filter_map(|l| {
            let t = l.trim();
            let name = t.strip_suffix(": require")?;
            name.starts_with("MATHLESS_GATE_").then(|| name.to_string())
        })
        .collect();
    let mut required: Vec<String> = required.into_iter().collect();
    required.sort();
    required.dedup();
    assert!(
        required.len() >= 2,
        "recovered {required:?} from .github/workflows/ci.yml — CI requires at least the C \
         host and one Pascal gate, so a shorter list means this parse no longer reads that file"
    );

    for (path, text) in every_markdown_file() {
        if is_dated_record(&path) {
            continue;
        }
        for (no, line) in text.lines().enumerate() {
            if !line.contains("cargo test --workspace") {
                continue;
            }
            // `require`, not a word that merely starts with it. Grok raised this verifying
            // the guard and the plant confirmed it: a line spelling `MATHLESS_GATE_FPC=required`
            // counted as setting that gate, so a typo would have been read as coverage.
            let set = |g: &String| {
                line.match_indices(&format!("{g}=require")).any(|(at, m)| {
                    !line[at + m.len()..].starts_with(|c: char| c.is_alphanumeric() || c == '_')
                })
            };
            // Requires at least one gate → it is publishing the gated baseline, not prose.
            if !required.iter().any(set) {
                continue;
            }
            let missing: Vec<&String> = required.iter().filter(|g| !set(g)).collect();
            assert!(
                missing.is_empty(),
                "{path}:{} publishes a baseline that requires some gates but not {missing:?}. \
                 CI requires {required:?}, and a skipped gate is not a passed gate — the suite \
                 goes green with the others unset, so whoever follows this line verifies less \
                 than they think.",
                no + 1
            );
        }
    }
}

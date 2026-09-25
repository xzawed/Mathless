//! Wherever a document states a code-derived fact, it states the current value.

use super::*;

/// **A message a host README quotes is one that host, or its test harness, can print.**
///
/// Why: `c-host-link/README.md` quoted a refusal from before #215, and a reader who greps for
/// a phantom transcript concludes the check is not there. (#274)
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

/// **"Run what that job runs" sets every gate `ci.yml` requires.**
///
/// Why: the CONTRIBUTING block set one gate of three, so the others skipped, the suite exited 0
/// and CI went red after review. Derived from `ci.yml`, not a hand-copied list. (#277)
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
                names_identifier(block, gate),
                "a CONTRIBUTING.md command block runs `cargo test --workspace` without setting \
                 {gate}, which .github/workflows/ci.yml requires. On a machine missing that \
                 toolchain the gate SKIPS, the suite exits 0, and CI is the one that says no"
            );
        }
    }
}

/// **No document states a Rust version that is not the pinned one.**
///
/// Why: five documents copy the `rust-toolchain.toml` pin, and raising it would make all five
/// false at once with nothing to notice. A stated version must be the pin. (#277)
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
        if is_record(&path) {
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
/// Why: lowering `ML_MAX_MODULE_NAME` from 64 to 48 left every test green while three documents
/// kept saying 64. A line that also states the true value passes. (#226)
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

/// **A document that enumerates the export set as complete names all of it.**
///
/// Why: after the set moved 2 → 3 (#105), twelve lines in ten files still listed `mlx_*` and
/// `ml_module_abi_version` alone. This walks every Markdown file instead of a list. (#283)
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

/// **A documented baseline that requires one gate requires all of them.**
///
/// Why: two lines outside the guarded file — one of them the first command a new session runs —
/// published a one-gate baseline, and a skipped gate is not a passed gate. (#284)
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
        if is_record(&path) {
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

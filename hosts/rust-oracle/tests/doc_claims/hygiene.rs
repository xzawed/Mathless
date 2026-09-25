//! Source and artifact checks that are not about prose: ASCII sources, symbol names, the
//! header a user receives, and how tests are gated.

use super::*;

/// The one piece of prose that travels WITH the artifact.
///
/// Every generated header carries a note saying what has and has not been verified. For a
/// whole slice after link-time binding was verified with a measured run, that note still
/// told each user "Not verified: … link-time binding via an import library". A stale
/// document is bad; a stale sentence compiled into the product's own output is worse,
/// because it reaches people who never open this repository.
///
/// Conditional on the evidence existing, the same shape as the ABI-refusal guard: if the
/// link host is deleted, this stops demanding the claim.
#[test]
fn the_generated_header_does_not_deny_a_verification_that_exists() {
    let link_host = repo_root().join("hosts").join("c-host-link").join("host.c");
    if !link_host.exists() {
        return;
    }

    // The EMITTED note, not the emitter's source. Matching source text was the first shape
    // of this test and of its neighbour above; both were brittle in the same way — a
    // reworded or reflowed literal breaks the match without changing the artifact, and a
    // literal left behind unused satisfies it without reaching the artifact. What a user
    // reads is the output, so that is what is asserted.
    let ir = mlc::compile_to_ir("export fn f(x: f64) -> f64 { return x }\n")
        .expect("the probe module must compile");
    let h = mlc::header::emit_c_header(&ir, "widget");

    assert!(
        !h.contains("link-time binding via an"),
        "the generated header still tells its reader 'Not verified: … link-time binding via \
         an import library', but hosts/c-host-link/host.c links against the packaged .lib \
         and acceptance D runs it:\n{h}"
    );
    assert!(
        h.contains("hosts/c-host-link"),
        "the generated header's verification note does not mention the link host. Dropping \
         the denial is not enough — the note is what a user reads to know which consumption \
         paths were actually proved:\n{h}"
    );
}

/// The error constant carries its module, and the emitter cannot quietly drop it again.
///
/// `SPEC-error-prefix` §3-F. The unprefixed `ML_ERR_<NAME>` survived four slices while the
/// fingerprint constant beside it was given a prefix *because this debt was recorded* — the
/// rule was decided and simply not applied here. This is the assertion that keeps it applied.
///
/// It reads the emitter, not the output: a golden can be re-blessed, and a re-bless that
/// silently accepted a bare `ML_ERR_` is exactly the failure mode this file exists for.
#[test]
fn the_emitter_prefixes_error_constants_with_the_module() {
    // Run the emitter and read WHAT IT WROTE. The first version of this test matched source
    // text in header.rs instead — including a `format!` call character for character — and
    // Grok showed it had a hole big enough to drive the regression through: leave
    // `error_macro` defined but stop calling it, and every source-text assertion still
    // passed while the emitted constants lost their prefix. It also failed on harmless
    // changes (rustfmt wrapping the call, renaming the helper).
    //
    // Text generation needs no build, so this stays off `cfg(windows)` and the ubuntu job
    // runs it. The module name is deliberately NOT one of the corpus names: the rule is
    // being checked, not one fixture.
    let ir = mlc::compile_to_ir(
        "error E_NEG = 3\n\
         error E_LATE = 4\n\
         export fn take(x: f64) -> f64! {\n\
         \x20 if x < 0.0 { fail E_NEG }\n\
         \x20 return x\n\
         }\n",
    )
    .expect("the probe module must compile");

    let h = mlc::header::emit_c_header(&ir, "widget");
    let pas = mlc::header::emit_delphi_unit(&ir, "widget");

    for (binding, text, expected) in [
        ("the C header", &h, "#define ML_WIDGET_ERR_E_NEG 3"),
        ("the Delphi unit", &pas, "ML_WIDGET_ERR_E_NEG = 3;"),
    ] {
        assert!(
            text.contains(expected),
            "{binding} does not carry the module in the error constant's name (expected \
             '{expected}'). Two modules that both declare a common error name would collide \
             again — measured as C4005 under /W4 /WX before Q14 was closed:\n{text}"
        );
        assert!(
            !text.contains("ML_ERR_"),
            "{binding} still emits a bare 'ML_ERR_', which is the collision itself:\n{text}"
        );
    }

    // The guard that must NOT be added (DP-Q3): it would let the first header win and turn
    // that loud C4005 into a silent wrong meaning.
    assert!(
        !h.contains("#ifndef ML_WIDGET_ERR"),
        "an #ifndef guard around a per-module error constant makes a genuine conflict silent"
    );
}

/// `runtime/ml_abi.h` declares the reserved half of the ABI by hand, and nothing compiles
/// it, so nothing noticed when it fell behind: every module has exported `ml_iface_hash`
/// since #105 and the header that exists to list the reserved symbols did not mention it.
///
/// This derives the list from the emitter instead of trusting the file. Any reserved `ml_*`
/// declaration `header.rs` writes into a generated header must also be here — with or
/// without parameters — and the truncation status must carry the same value in both places.
///
/// It also forbids a module-specific `mlx_*` declaration. The file used to declare
/// `mlx_discount` from one example — the kind of detail that goes stale in a file nobody
/// compiles, and the reason D4 was open at all.
///
/// **One of the two is a PATTERN, not a declaration.** Since `SPEC-qualified-iface-hash` the
/// fingerprint export is `ml_iface_hash_<module>`, so the emitter's literal carries a format
/// placeholder and `runtime/ml_abi.h` cannot declare the symbol at all — the name is not
/// fixed. It documents the shape instead, and [`placeholder_to_module`] is the single
/// translation between the two spellings. That is a real narrowing of what this can prove:
/// for that symbol it now checks that the file DESCRIBES the export, not that it declares
/// it. Both were only ever textual — nothing compiles `ml_abi.h` — so what is lost is the
/// declaration's shape, and what is kept is the name and the signature.
#[test]
fn the_hand_written_abi_header_declares_every_reserved_symbol_the_compiler_emits() {
    let header_rs = read("compiler/src/header.rs");
    let abi_h = read("runtime/ml_abi.h");

    // Declarations the emitter writes, recovered from its string literals. Every literal on
    // a line is a candidate; one that names an `ml_` symbol and ends a declaration is a
    // reserved export. Deliberately not limited to `(void);` — a reserved export that takes
    // an argument would otherwise slip past exactly the way `ml_iface_hash` did (Grok raised
    // this while verifying the narrower first version).
    let mut reserved: Vec<&str> = Vec::new();
    for line in header_rs.lines() {
        for (i, part) in line.split('"').enumerate() {
            if i % 2 == 1 && part.contains(" ml_") && part.ends_with(");") && !part.contains('\\') {
                reserved.push(part);
            }
        }
    }
    reserved.sort_unstable();
    reserved.dedup();
    assert!(
        reserved.len() >= 2,
        "expected header.rs to emit at least the two reserved symbols, recovered {reserved:?} \
         — has the emitter's shape changed?"
    );

    for decl in &reserved {
        let want = placeholder_to_module(decl);
        assert!(
            abi_h.contains(&want),
            "compiler/src/header.rs emits '{decl}' into every generated header, but \
             runtime/ml_abi.h does not carry '{want}'. That file is the hand-written list of \
             reserved symbols; nothing compiles it, so only this test can notice"
        );
    }

    for line in abi_h.lines() {
        assert!(
            !(line.contains("mlx_") && line.trim_end().ends_with(");")),
            "runtime/ml_abi.h declares a module function ('{}'). Module exports belong in \
             the module's generated header — a specific mlx_ name here goes stale unnoticed",
            line.trim()
        );
    }

    // It also TEACHES a naming rule, and that rule has to be the one the emitter follows.
    // It did not: after Q14 renamed error constants, this file still told a third-party host
    // author they would find `ML_ERR_<NAME>` in a generated header — while, four dozen lines
    // lower, correctly describing `ML_<MODULE>_IFACE_HASH`. One hand-written contract
    // teaching two naming rules is the exact state Q14 existed to remove.
    //
    // Derived from the emitter, not asserted as a literal: build a header and read the shape
    // back out of it.
    let ir = mlc::compile_to_ir(
        "error E_NEG = 3\nexport fn take(x: f64) -> f64! { if x < 0.0 { fail E_NEG } return x }\n",
    )
    .expect("the probe module must compile");
    let probe = mlc::header::emit_c_header(&ir, "widget");
    // Asserted, NOT used as an `if` gate. Behind a gate, a change to the emitted shape would
    // skip the two checks below instead of failing them — the same silent-skip pattern this
    // file keeps finding elsewhere. Grok pointed it out one review after the gate was
    // written; if the shape moves, this line is where it stops.
    assert!(
        probe.contains("ML_WIDGET_ERR_E_NEG"),
        "the emitter no longer produces ML_<MODULE>_ERR_<NAME>; the contract this test holds \
         ml_abi.h to is derived from that shape:\n{probe}"
    );
    assert!(
        abi_h.contains("ML_<MODULE>_ERR_<NAME>"),
        "runtime/ml_abi.h does not describe the error-constant shape the emitter produces. A \
         generated header defines ML_<MODULE>_ERR_<NAME>; this file is what a third-party \
         host author reads to learn that"
    );
    assert!(
        !abi_h.contains("ML_ERR_<NAME>"),
        "runtime/ml_abi.h still teaches the pre-Q14 shape ML_ERR_<NAME>, which no generated \
         header has used since 2026-09-03"
    );

    // The one negative status that exists is defined in both places, and the values must
    // agree: a translation unit can see both, and the `#ifndef` guard means a mismatch is
    // NOT a redefinition error — it silently resolves to whichever was seen first.
    //
    // Asserted as two positives, not as `assert_eq!` of two `contains` flags. That earlier
    // shape passed when the definition was missing from BOTH files, which is a vacuous
    // success of exactly the kind section 7-1 warns about (Grok caught it in review).
    //
    // The first half used to grep `header.rs` for the literal `#define
    // ML_ST_INSUFFICIENT_BUFFER (-1)`. That broke the day the value moved into
    // `abi::ML_ST_INSUFFICIENT_BUFFER` and the emitter started interpolating it: the guard
    // failed while the emitted header was unchanged and MORE correct than before. Section 7
    // says it plainly -- guard the artifact, not the code that writes it. So this calls the
    // emitter and reads what comes out.
    let define = format!(
        "#define ML_ST_INSUFFICIENT_BUFFER ({})",
        mlc::abi::ML_ST_INSUFFICIENT_BUFFER
    );
    let define = define.as_str();
    let emitted = {
        let ir = mlc::compile_to_ir("export fn f(s: string) -> string! { return s }")
            .expect("a string return is what makes the header emit the guard");
        mlc::header::emit_c_header(&ir, "probe")
    };
    assert!(
        emitted.contains(define),
        "a generated header no longer defines '{define}'. The truncation status is the one \
         negative both bindings must agree on, so if it changed value, runtime/ml_abi.h has \
         to change with it"
    );
    assert!(
        abi_h.contains(define),
        "runtime/ml_abi.h no longer declares '{define}' while header.rs still emits it. \
         Both are `#ifndef`-guarded, so a divergence is silent: the translation unit keeps \
         whichever definition it saw first"
    );
}

/// Acceptance D is the only thing in this repository that compiles a generated `.h` as C.
/// An example whose header is not included there has a surface no C compiler has ever
/// read: the Rust oracle checks the module's behaviour, not whether the header it ships
/// beside it is valid C11 under `/W4 /WX`.
///
/// This was measured as a real hole — 14 of 18 — and the worst of the four outside it was
/// `shapes`, the file written to collect "export shapes where a mis-written C ABI adapter
/// would compile and return a plausible wrong value" (`STATUS.md` N1).
#[test]
fn every_example_header_is_compiled_by_the_c_host() {
    // Comments stripped: a commented-out `#include "shapes.h"` would satisfy the check below
    // while the C compiler never sees that header — the coverage hole this test exists to
    // close, reopened in a way that reads as closed. None exists today (measured).
    let host_c = strip_c_comments(&read("hosts/c-host/host.c"));
    let dir = repo_root().join("examples");

    let mut examples: Vec<String> = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("{}: {e}", dir.display()))
        .filter_map(|entry| {
            let path = entry.expect("dir entry").path();
            if path.extension().and_then(|e| e.to_str()) != Some("mls") {
                return None;
            }
            Some(path.file_stem()?.to_str()?.to_string())
        })
        .collect();
    examples.sort();

    assert!(
        examples.len() >= 18,
        "found only {} example(s) under examples/ — the corpus does not shrink, so this is \
         more likely a path problem than a deletion",
        examples.len()
    );

    let missing: Vec<&String> = examples
        .iter()
        .filter(|stem| !host_c.contains(&format!("#include \"{stem}.h\"")))
        .collect();

    assert!(
        missing.is_empty(),
        "hosts/c-host/host.c does not include the generated header for {} of {} examples: \
         {missing:?}. Their headers are never compiled as C, so an invalid one ships \
         unnoticed. Emit them in c_host.rs and include them here",
        missing.len(),
        examples.len()
    );
}

/// `host.c` is compiled by MSVC with `/W4 /WX`, and a non-ASCII byte in it is warning
/// C4819 ("cannot be represented in the current code page") which `/WX` turns into an
/// error. That gate is real but it costs a full acceptance-D build and only runs where
/// MSVC exists; this costs nothing and runs on the ubuntu job too.
///
/// Written after exactly that: an em dash in a comment failed the C build.
#[test]
fn the_c_sources_are_ascii_only() {
    // DISCOVERED, not listed. The earlier version named the two hosts by hand, so a third C
    // host would have been added outside the check — and this is a rule about the language
    // the compiler reads, which applies to every C source, not to two chosen ones.
    // `runtime/ml_abi.h` is compiled by acceptance D, so it is covered there.
    let hosts = repo_root().join("hosts");
    let mut sources: Vec<PathBuf> = Vec::new();
    for entry in std::fs::read_dir(&hosts).expect("hosts/") {
        let dir = entry.expect("dir entry").path();
        if !dir.is_dir() {
            continue;
        }
        for f in std::fs::read_dir(&dir).expect("host dir") {
            let p = f.expect("dir entry").path();
            if p.extension().and_then(|e| e.to_str()) == Some("c") {
                sources.push(p);
            }
        }
    }
    sources.sort();
    assert!(
        sources.len() >= 2,
        "expected at least the two C hosts under hosts/, found {sources:?}"
    );

    for src in &sources {
        let rel = src
            .strip_prefix(repo_root())
            .expect("under the repo root")
            .to_string_lossy()
            .replace('\\', "/");
        assert_ascii(&rel);
    }
}

/// The link host's whole claim is that it never looks a symbol up by name.
///
/// `SPEC-linkable-bindings` §3-B says the host binds through the import library "without
/// ever calling GetProcAddress". A test that only checks the program's OUTPUT cannot tell
/// the difference — a host that quietly fell back to dynamic loading would print the same
/// `LINK_GATE_OK`. So the claim is pinned where it can be checked: in the source.
#[test]
fn the_link_host_never_resolves_a_symbol_by_name() {
    // CODE, not prose. The first version of this test failed on the file's own comment
    // saying "there is deliberately no GetProcAddress here" — a guard that forbids naming
    // the thing you are not doing punishes the explanation, so the comments come out first.
    let host = strip_c_comments(&read("hosts/c-host-link/host.c"));
    for dynamic in ["GetProcAddress", "LoadLibrary", "FreeLibrary", "HMODULE"] {
        assert!(
            !host.contains(dynamic),
            "hosts/c-host-link/host.c mentions '{dynamic}'. That host exists to prove \
             LINK-time binding; if it resolves anything dynamically it is proving the thing \
             hosts/c-host already proves"
        );
    }
    // And it really does call the module and the gate, rather than being an empty shell
    // that trivially satisfies the check above.
    // Both fingerprints, not one. This host links two modules, and until
    // SPEC-qualified-iface-hash they exported the same `ml_iface_hash`, so "the host calls
    // the fingerprint" was satisfied while `schedule` was called with no interface check at
    // all — the linker chose which module the single call reached (STATUS §7-4). Naming both
    // is what makes this guard say "every linked module is gated" rather than "a gate exists".
    for expected in [
        "mlx_discount(",
        "ml_iface_hash_discount()",
        "ml_iface_hash_schedule()",
        "ml_module_abi_version()",
    ] {
        assert!(
            host.contains(expected),
            "hosts/c-host-link/host.c does not call '{expected}' — it would pass the \
             no-dynamic-loading check by doing nothing"
        );
    }
}

/// The hand-written C and C++ sources must be pure ASCII, because MSVC reads them in the
/// machine's ANSI code page and `/W4 /WX` turns "not representable there" into an error.
///
/// Found by writing an em-dash into a `host.c` comment. On this development machine (code
/// page 949) the acceptance-D gate failed immediately:
///
/// ```text
/// host.c(1): warning C4819: <character not representable in code page 949>
/// host.c(1): error C2220: warning treated as error
/// ```
///
/// The reason this is a test and not just "the build caught it" is that the build would NOT
/// have caught it everywhere. An em-dash *is* representable in CP1252, so on a runner with
/// that code page the same file compiles clean — the failure is a property of the reader, not
/// of the file, which is exactly the shape that reaches one developer and not the next. It is
/// the same argument the generated headers already make for identifiers (`reserved.rs`), and
/// the hand-written hosts had no equivalent.
///
/// Text only, so the ubuntu insurance job runs it too — which is the point: it needs no MSVC.
#[test]
fn the_hand_written_c_sources_are_pure_ascii() {
    let root = repo_root();
    // Discovered, not listed: a hand-written host added next year is covered without anyone
    // remembering this file. Generated artifacts are not here — they live in temp dirs, and
    // `compiler/tests/emit.rs` already asserts the header is ASCII.
    let mut sources: Vec<(String, PathBuf)> = Vec::new();
    for dir in ["hosts/c-host", "hosts/c-host-link", "runtime"] {
        let Ok(entries) = std::fs::read_dir(root.join(dir)) else {
            continue;
        };
        for e in entries.flatten() {
            let p = e.path();
            let ext = p.extension().and_then(|x| x.to_str()).unwrap_or("");
            if matches!(ext, "c" | "h" | "cpp" | "hpp") {
                let name = p.file_name().and_then(|x| x.to_str()).unwrap_or("?");
                sources.push((format!("{dir}/{name}"), p));
            }
        }
    }
    let mut checked = 0;
    for (rel, path) in &sources {
        let bytes = std::fs::read(path).unwrap_or_else(|e| panic!("read {rel}: {e}"));
        checked += 1;
        if let Some(i) = bytes.iter().position(|b| !b.is_ascii()) {
            let line = 1 + bytes[..i].iter().filter(|b| **b == b'\n').count();
            panic!(
                "{rel}:{line} holds the non-ASCII byte 0x{:02X}. MSVC reads this file in the \
                 machine's ANSI code page, so under `/W4 /WX` it is C4819 -> C2220 on any code \
                 page that cannot represent it — a build that fails for one developer and not \
                 the next. Write it in ASCII.",
                bytes[i]
            );
        }
    }
    assert!(
        checked >= 3,
        "expected at least the two C hosts and the ABI header; found {checked} ({:?}). A guard \
         that scans an empty set passes forever — if these moved, point it at where they went",
        sources.iter().map(|(r, _)| r).collect::<Vec<_>>()
    );
}

/// **The reference dynamic host must be able to gate every module name the compiler accepts.**
///
/// Two numbers in two languages that have to agree: `ML_MAX_MODULE_NAME` in
/// `compiler/src/abi.rs`, and the `#define` of the same name in `hosts/c-host/host.c` that
/// sizes the buffer `gate()` builds `ml_iface_hash_<module>` into. C cannot read the Rust
/// constant, so the host carries a copy — and a copy that nothing checks is how the two drift.
///
/// They HAD drifted, in the only direction that matters: the host's buffer was an
/// independently chosen `80`, which gated 65 characters and refused at 66, while the compiler
/// had no bound at all and `mlc build` produced a 70-character module at exit 0. The
/// compiler shipped modules its own reference host could not gate
/// (`SPEC-module-name-length` §0.1).
///
/// This reads both sources rather than running anything, so unlike the acceptance-D gates it
/// needs no MSVC and runs on **both** CI jobs — which is the point, because the Windows job
/// is the only one that would otherwise notice, and only if a long name were in the corpus.
#[test]
fn the_c_host_can_gate_every_module_name_the_compiler_accepts() {
    let abi = read("compiler/src/abi.rs");
    let compiler_bound: usize =
        numbers_between(&abi, "pub const ML_MAX_MODULE_NAME: usize = ", ";")
            .first()
            .copied()
            .unwrap_or_else(|| {
                panic!(
                    "compiler/src/abi.rs no longer declares ML_MAX_MODULE_NAME — either put it \
                 back or drop this guard, but do not leave the host's copy unchecked"
                )
            });

    let host = read("hosts/c-host/host.c");
    // Empty suffix on purpose: the number ends the line, and this working tree checks C
    // sources out with CRLF, so a `"\n"` suffix matched nothing and the guard failed for a
    // reason that had nothing to do with the bound.
    let host_bound: usize = numbers_between(&host, "#define ML_MAX_MODULE_NAME ", "")
        .first()
        .copied()
        .unwrap_or_else(|| {
            panic!(
                "hosts/c-host/host.c no longer defines ML_MAX_MODULE_NAME — gate() sizes its \
                 symbol buffer from it"
            )
        });

    assert!(
        host_bound >= compiler_bound,
        "the C host's ML_MAX_MODULE_NAME is {host_bound} but the compiler accepts names up \
         to {compiler_bound}. A module with a name between the two compiles, links and \
         loads, and is then REFUSED by the reference dynamic host with `module name too \
         long to form its fingerprint symbol` — far from the cause. The compiler sets the \
         bound; the host follows it"
    );

    // And the buffer really is derived from that define rather than a number beside it.
    assert!(
        host.contains("char symbol[sizeof \"ml_iface_hash_\" + ML_MAX_MODULE_NAME]"),
        "hosts/c-host/host.c must size gate()'s buffer from ML_MAX_MODULE_NAME and the \
         prefix, or this guard checks a constant nothing uses"
    );
}

/// **No test creates a temp directory by hand — `common::TempOut` is the only way.**
///
/// This has recurred twice. `STATUS.md` §5-5.7 measured the first version (clear at the
/// START, never at the end: 1,720 trees / 976 MB on one development machine), and #219
/// answered it with `TempOut`, whose Drop also runs when a test panics and whose removal
/// retries past the Windows DLL-lock race. Then a NEW file copied the older helper it had
/// replaced, and eight trees were left behind in a single afternoon (§9-44.3).
///
/// Twice means the helper existing is not enough, because nothing made a new file use it.
/// This is what makes it stick. Measured before it was written: every leftover tree in the
/// working directory — `mlc_w6` ×7, `mlc_gco_*` ×6, `mlc_arr_*`, `mlc_calls`, `mlc_str_*`,
/// `mlc_amb` — belonged to a file still building its path by hand, and none to a converted one.
///
/// The needle is `create_dir_all`, not `temp_dir()`: a test may legitimately NAME a temp path
/// it never creates (`diagnostics.rs` builds one only to watch the compiler refuse before
/// anything is written). Creating is what leaves something behind.
#[test]
fn no_test_creates_a_temp_directory_by_hand() {
    // The needles are ASSEMBLED, not written. Spelled literally they appear in this file — in
    // the search itself — and the guard reported its own source as an offender. Exempting
    // this file by name would have worked and would have been worse: an exemption hides a
    // real offender the day someone adds one here. Same problem the gated-module guard above
    // records ("it cannot tell a CLAIM from a QUOTATION"), answered the same way, by making
    // the text not match rather than by carving out a hole.
    let creates = format!("fs::{}", "create_dir_all");
    let names_temp = format!("env::{}()", "temp_dir");
    // Assembled for the same reason, and it bit the same way: written literally, this file
    // counted as a THIRD definition of the helper.
    let defines_helper = format!("pub struct {}", "TempOut");
    let drops_it = format!("impl {} for", "Drop");
    let removes = format!("remove_{}", "dir_all");

    let root = repo_root();
    let mut offenders = Vec::new();
    // RECURSIVE, and that is not incidental: `common/` is itself a subdirectory, so a
    // top-level-only walk would miss a leaky helper copied into exactly the place a helper
    // most plausibly goes. (Review named this; it was a hole, not a boundary.)
    // `src/bin` is in scope, and that was measured the hard way: `mlprobe` landed in the same
    // session as this guard, created its scratch tree by hand, and left 27 of them — in the
    // one directory a `tests/`-only walk does not reach. A developer-run binary is the same
    // kind of thing as a test for this purpose. Library code stages its own builds with its
    // own documented cleanup (`emit.rs`), which is a different question this does not answer.
    let mut stack: Vec<PathBuf> = [
        "hosts/rust-oracle/tests",
        "compiler/tests",
        "compiler/src/bin",
    ]
    .iter()
    .map(|d| root.join(d))
    .collect();
    let mut seen_files = 0usize;
    let mut helpers = 0usize;
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            panic!(
                "{} is missing — this guard would pass by checking nothing",
                dir.display()
            );
        };
        for entry in entries {
            let path = entry.expect("a directory entry").path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            if path.extension().and_then(|e| e.to_str()) != Some("rs") {
                continue;
            }
            seen_files += 1;
            let text = std::fs::read_to_string(&path).expect("read a test file");
            // The file that DEFINES the helper is the one place a create belongs. Recognised
            // by that definition rather than by its path, so the exemption is a property and
            // not a name: a file stops being exempt the moment it stops being the helper.
            if text.contains(&defines_helper) {
                helpers += 1;
                continue;
            }
            // A file that removes its own tree in a `Drop` has answered the question. Checked
            // as a property, like the helper exemption above: a binary cannot import the test
            // helper, so it has to carry its own, and what matters is that cleanup survives an
            // unwind — not which type provides it.
            // …and the Drop must actually REMOVE something. `impl Drop for` alone is too
            // loose: a file with an unrelated Drop would be exempt without cleaning anything
            // up. Review named that; it costs one more substring to close.
            if text.contains(&drops_it) && text.contains(&removes) {
                continue;
            }
            for (n, line) in text.lines().enumerate() {
                // A create on a path the same file derived from the system temp directory.
                if line.contains(&creates) && text.contains(&names_temp) {
                    let rel = path
                        .strip_prefix(&root)
                        .expect("a path under the root")
                        .to_string_lossy()
                        .replace('\\', "/");
                    offenders.push(format!("{rel}:{}", n + 1));
                }
            }
        }
    }
    assert!(
        seen_files >= 30,
        "only {seen_files} test files were read — the walk is checking almost nothing"
    );
    assert_eq!(
        helpers, 2,
        "expected exactly the two byte-identical copies of tests/common/mod.rs to define \
         `TempOut`, found {helpers} — a third definition would be a third exemption, and \
         nothing else asked for one"
    );
    assert!(
        offenders.is_empty(),
        "these build a temp directory by hand instead of holding `common::TempOut`, so the \
         tree survives whenever the test panics — which is exactly when you are iterating \
         (STATUS §5-5.7, §9-44.3):\n  {}\n\nUse `common::TempOut::new(\"<tag>\")` and drop the \
         manual cleanup; bind it as `_out` if nothing reads it, never `_`, which would delete \
         the tree while the module is still loaded.",
        offenders.join("\n  ")
    );
}

/// **A test that builds a module carries a Windows gate.**
///
/// `codegen::build_cdylib` looks for `target/release/<name>.dll`. On Linux cargo writes
/// `lib<name>.so`, so the build succeeds and the artifact lookup fails — the unstarted D22
/// gap, documented on `compiler/tests/generated_crate_output.rs`, whose own success-path test
/// is `#[cfg(windows)]` for exactly this reason. **Every test that calls `emit_artifacts` or
/// `build_cdylib` and expects an artifact is Windows-only whether or not it says so.**
///
/// Thirty-seven test files call one of those two entry points and thirty-six carried a gate.
/// The one that did not was written the day this guard was: a test *about* temp trees, whose
/// only case that creates one is a successful build. The ubuntu job caught it in 24 seconds,
/// which is what that job is for — but a red CI on a PR is still a claim made and retracted,
/// and this is a `grep` that costs nothing.
///
/// **It is a floor, not a proof, and the difference is worth stating.** It requires the file
/// to contain a windows gate *somewhere*; it cannot tell that the gate is on the call. Two
/// files (`diagnostics.rs`, `emit_robustness.rs`) gate per-test rather than file-wide and are
/// green on ubuntu, so demanding `#![cfg(windows)]` at the top would be wrong. What this
/// catches is the case that actually happened: no gate at all.
#[test]
fn every_module_building_test_is_windows_gated() {
    let mut checked = 0usize;
    for (path, text) in every_file_ending_in(&[".rs"]) {
        if !path.contains("/tests/") {
            continue;
        }
        // A call that expects SUCCESS. `diagnostics.rs` calls `emit_artifacts` and takes
        // `unwrap_err()` — a rejection never reaches the cdylib build, so it is correctly
        // cross-platform and an unconditional rule would have forced a wrong gate onto it.
        // Measured: 37 files expect success, and before this guard exactly one had no gate.
        let expects_success = ["emit_artifacts(", "build_cdylib("].iter().any(|entry| {
            text.match_indices(entry).any(|(at, _)| {
                let stmt = &text[at..];
                let end = stmt.find(';').unwrap_or(stmt.len().min(300));
                let stmt = &stmt[..end];
                !stmt.contains("unwrap_err") && !stmt.contains("is_err")
            })
        });
        // The other way a test builds a module: spawning the CLI. `cli_temp_trees.rs` does
        // that and calls neither entry point, so the library needles alone would have missed
        // the very mistake this guard was written for — the plant caught that.
        //
        // `diagnostics.rs` spawns `mlc build` too and is correctly cross-platform, because it
        // only ever asserts the build FAILED. The discriminator is that: take the text before
        // each `.success()` on its line and see whether any of them lacks a `!`. Measured
        // across the three files that spawn the CLI — two need a gate and have one, one does
        // not and has none, no false positives either way.
        let cli_expects_success = text.contains("CARGO_BIN_EXE_mlc")
            && text.contains("\"build\"")
            && text.lines().any(|l| {
                l.find(".success()").is_some_and(|at| {
                    // The `!` immediately before the RECEIVER, not anywhere in the line.
                    // `assert!(!out.status.success())` is negated; `assert!(out.status
                    // .success())` is not — and both contain a `!`, from `assert!`. The first
                    // version looked for one anywhere and so measured "is it inside an
                    // assert", which classified every line the same way. A plant caught it.
                    let head = l[..at]
                        .trim_end_matches(|c: char| c.is_alphanumeric() || c == '_' || c == '.');
                    !head.ends_with('!')
                })
            });

        if !expects_success && !cli_expects_success {
            continue;
        }
        checked += 1;
        assert!(
            text.contains("cfg(windows)"),
            "{path} builds a module (`emit_artifacts` or `build_cdylib`) with no Windows gate \
             anywhere in the file. `build_cdylib` looks for `target/release/<name>.dll` and \
             Linux cargo writes `lib<name>.so`, so this passes here and fails on the ubuntu \
             job. Add `#![cfg(windows)]` at the top, or `#[cfg(windows)]` on the tests that \
             build — and if the property you want is cross-platform, assert it in the front \
             end instead, which is both portable and stronger."
        );
    }
    assert!(
        checked >= 20,
        "found only {checked} test files calling emit_artifacts/build_cdylib; the tree had 37 \
         when this guard was written, so the walk or the entry-point names have moved and this \
         is passing by checking almost nothing"
    );
}

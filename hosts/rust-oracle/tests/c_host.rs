//! **Acceptance D** (`docs/phase1/SPEC.md` §3-D) — the gate that was BLOCKED for all of
//! Phase 1: load the same `mlc`-produced `.dll` from a *real C host*, not from our own Rust
//! oracle, and call it over the plain C ABI.
//!
//! The host is `hosts/c-host/host.c`, compiled here with MSVC `cl` (D22 target, toolchain
//! chosen 2026-08-29). It includes the generated `.h` files and derives its function-pointer
//! types from their declarations, so a change in a generated signature breaks the build.
//!
//! **Skipping must never read as passing.** With no MSVC on the machine this test prints
//! `GATE_D_SKIPPED` and returns — but if `MATHLESS_GATE_D=require` is set (CI does set it)
//! a missing toolchain is a *failure*. Only a run that prints `GATE_D_OK` closes the gate.
#![cfg(windows)]

use std::path::{Path, PathBuf};
use std::process::Command;

use ml_oracle::pe;

mod common;
use mlc::emit::emit_artifacts;

/// `vcvars64.bat` for the newest installed MSVC, or `None` if MSVC isn't installed.
fn vcvars64() -> Option<PathBuf> {
    let program_files_x86 =
        std::env::var("ProgramFiles(x86)").unwrap_or_else(|_| r"C:\Program Files (x86)".into());
    let vswhere = Path::new(&program_files_x86)
        .join("Microsoft Visual Studio")
        .join("Installer")
        .join("vswhere.exe");
    if !vswhere.exists() {
        return None;
    }
    let out = Command::new(&vswhere)
        .args([
            "-latest",
            "-products",
            "*",
            "-requires",
            "Microsoft.VisualStudio.Component.VC.Tools.x86.x64",
            "-property",
            "installationPath",
        ])
        .output()
        .ok()?;
    let root = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if root.is_empty() {
        return None;
    }
    let bat = Path::new(&root)
        .join("VC")
        .join("Auxiliary")
        .join("Build")
        .join("vcvars64.bat");
    bat.exists().then_some(bat)
}

/// Run `body` inside a `vcvars64` environment. Everything MSVC needs (INCLUDE, LIB, PATH)
/// comes from that script, so we drive it through a one-shot batch file rather than trying
/// to reproduce the environment ourselves. `VSLANG=1033` keeps tool output English so the
/// parsing below doesn't depend on the machine's locale.
fn run_in_msvc_env(vcvars: &Path, workdir: &Path, body: &str) -> std::process::Output {
    let bat = workdir.join("run.bat");
    std::fs::write(
        &bat,
        format!(
            "@echo off\r\nset VSLANG=1033\r\ncall \"{}\" >nul 2>&1\r\nif errorlevel 1 exit /b 90\r\ncd /d \"{}\"\r\n{body}\r\n",
            vcvars.display(),
            workdir.display()
        ),
    )
    .expect("write run.bat");
    Command::new("cmd")
        .args(["/c".as_ref(), bat.as_os_str()])
        .output()
        .expect("spawn cmd")
}

/// Export names from `dumpbin /exports`, parsed without depending on the header wording:
/// an export row is `<ordinal> <hint> <rva> <name>`.
fn dumpbin_exports(vcvars: &Path, workdir: &Path, dll: &Path) -> Vec<String> {
    let out = run_in_msvc_env(
        vcvars,
        workdir,
        &format!("dumpbin /nologo /exports \"{}\"", dll.display()),
    );
    assert!(
        out.status.success(),
        "dumpbin failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let text = String::from_utf8_lossy(&out.stdout);
    let mut names: Vec<String> = text
        .lines()
        .filter_map(|line| {
            let f: Vec<&str> = line.split_whitespace().collect();
            if f.len() == 4 && f[0].parse::<u32>().is_ok() && u32::from_str_radix(f[2], 16).is_ok()
            {
                Some(f[3].to_string())
            } else {
                None
            }
        })
        .collect();
    names.sort();
    names
}

/// The same second opinion for the import table, in the `"dll!function"` form our reader uses.
///
/// `dumpbin /imports` prints one indented DLL name, then its functions as `<hint> <name>`
/// pairs (or `Ordinal <n>` when there is no name). The `Summary` section at the end also has
/// two-field lines (`1000 .rdata`), and its first field parses as hex — so parsing stops there
/// rather than inventing imports named after sections.
fn dumpbin_imports(vcvars: &Path, workdir: &Path, dll: &Path) -> Vec<String> {
    let out = run_in_msvc_env(
        vcvars,
        workdir,
        &format!("dumpbin /nologo /imports \"{}\"", dll.display()),
    );
    assert!(
        out.status.success(),
        "dumpbin /imports failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let text = String::from_utf8_lossy(&out.stdout);
    let mut names: Vec<String> = Vec::new();
    let mut current: Option<String> = None;
    for line in text.lines() {
        let t = line.trim();
        if t == "Summary" {
            break;
        }
        if t.to_ascii_lowercase().ends_with(".dll") && !t.contains(char::is_whitespace) {
            current = Some(t.to_ascii_lowercase());
            continue;
        }
        let Some(dll_name) = current.as_deref() else {
            continue;
        };
        let f: Vec<&str> = t.split_whitespace().collect();
        if f.len() != 2 {
            continue;
        }
        if f[0] == "Ordinal" {
            if let Ok(n) = f[1].parse::<u32>() {
                names.push(format!("{dll_name}!#{n}"));
            }
        } else if u32::from_str_radix(f[0], 16).is_ok() {
            names.push(format!("{dll_name}!{}", f[1]));
        }
    }
    names.sort();
    names
}

#[test]
fn a_real_c_host_loads_and_calls_the_module() {
    let Some(vcvars) = vcvars64() else {
        if std::env::var("MATHLESS_GATE_D").as_deref() == Ok("require") {
            panic!(
                "MATHLESS_GATE_D=require but MSVC was not found — acceptance D cannot be \
                 verified. Install the VS Build Tools with the C++ workload, or unset the var."
            );
        }
        // Deliberately loud: a skipped gate is NOT a passed gate.
        println!(
            "GATE_D_SKIPPED: no MSVC toolchain found (vswhere/vcvars64 absent). \
             Acceptance D is NOT verified by this run."
        );
        return;
    };

    // A guard, not a bare path: this test builds 19 DLLs and runs a child host process that
    // loads them, so its tree is the biggest one and the most likely to lose the unlock race
    // (measured — it leaked despite removing at the end). `TempOut` retries.
    let work = common::TempOut::new("gate_d");

    // The very same artifacts `mlc build` gives a user.
    let discount = emit_artifacts(
        include_str!("../../../examples/discount.mls"),
        "discount",
        &work,
    )
    .expect("emit discount");
    let safe_div = emit_artifacts(
        include_str!("../../../examples/safe_div.mls"),
        "safe_div",
        &work,
    )
    .expect("emit safe_div");
    let sum_to = emit_artifacts(
        include_str!("../../../examples/sum_to.mls"),
        "sum_to",
        &work,
    )
    .expect("emit sum_to");
    let negate_if = emit_artifacts(
        include_str!("../../../examples/negate_if.mls"),
        "negate_if",
        &work,
    )
    .expect("emit negate_if");
    let count_bounded = emit_artifacts(
        include_str!("../../../examples/count_bounded.mls"),
        "count_bounded",
        &work,
    )
    .expect("emit count_bounded");
    let discount4 = emit_artifacts(
        include_str!("../../../examples/discount4.mls"),
        "discount4",
        &work,
    )
    .expect("emit discount4");
    let pack = emit_artifacts(include_str!("../../../examples/pack.mls"), "pack", &work)
        .expect("emit pack");
    let commission = emit_artifacts(
        include_str!("../../../examples/commission.mls"),
        "commission",
        &work,
    )
    .expect("emit commission");
    let deduction = emit_artifacts(
        include_str!("../../../examples/deduction.mls"),
        "deduction",
        &work,
    )
    .expect("emit deduction");
    let line_total = emit_artifacts(
        include_str!("../../../examples/line_total.mls"),
        "line_total",
        &work,
    )
    .expect("emit line_total");

    let vat =
        emit_artifacts(include_str!("../../../examples/vat.mls"), "vat", &work).expect("emit vat");

    let carrier = emit_artifacts(
        include_str!("../../../examples/carrier.mls"),
        "carrier",
        &work,
    )
    .expect("emit carrier");

    let quote = emit_artifacts(include_str!("../../../examples/quote.mls"), "quote", &work)
        .expect("emit quote");

    let receipt = emit_artifacts(
        include_str!("../../../examples/receipt.mls"),
        "receipt",
        &work,
    )
    .expect("emit receipt");

    // N1 (`STATUS.md` section 9): four examples sat outside this gate, so their generated
    // headers had never been read by a C compiler. `shapes` was the sharpest of them — it
    // exists to collect the export shapes where a mis-written C ABI adapter would compile
    // and return a plausible wrong value, and its header was the one nobody compiled.
    //
    // Emitted for the HEADER: `host.c` includes all four and `cl /W4 /WX` has to accept
    // them. Behaviour stays where it already is, with the Rust oracle. `doc_claims.rs`
    // fails if a future example is added without landing here.
    for (src, name) in [
        (include_str!("../../../examples/add.mls"), "add"),
        (include_str!("../../../examples/discount2.mls"), "discount2"),
        (include_str!("../../../examples/discount3.mls"), "discount3"),
        (include_str!("../../../examples/shapes.mls"), "shapes"),
        (include_str!("../../../examples/refund.mls"), "refund"),
    ] {
        emit_artifacts(src, name, &work).unwrap_or_else(|e| panic!("emit {name}: {e}"));
    }

    // A drifted `pack`: the two parameters of `boxes` are swapped and nothing else changes.
    // Both versions are `int32_t mlx_boxes(int32_t, int32_t)` in C, so this is the drift the
    // ABI cannot see and the host's `_Static_assert` cannot catch — the measured case where
    // `boxes(100, 3)` quietly returned 0 instead of 33. The host must refuse it (WH6).
    let drifted_pack = "\
export fn boxes(per_box: i32, qty: i32) -> i32 { return qty / per_box }
export fn loose(qty: i32, per_box: i32) -> i32 { return qty % per_box }
error E_EMPTY_BOX = 1
export fn boxes_checked(qty: i32, per_box: i32) -> i32! {
  if per_box == 0 { fail E_EMPTY_BOX }
  return qty / per_box
}
";
    let drift = emit_artifacts(drifted_pack, "pack_drift", &work).expect("emit drifted pack");
    assert!(drift.dll.exists());

    let host_c = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("c-host")
        .join("host.c");
    assert!(host_c.exists(), "missing {}", host_c.display());

    // `runtime/ml_abi.h` is the hand-written half of the ABI, and nothing includes it — so
    // until now nothing compiled it either, and a text-only guard cannot know its text is
    // valid C (section 7-1, cause 4: match the assertion to the risk). It is compiled here
    // standalone, under the same flags as the host, and included TWICE to exercise the
    // include guard.
    //
    // This does not couple a host to it. The file stays uncoupled, which is the property
    // three other files cite it for; what is checked is only that it would compile if
    // someone did include it.
    let runtime_dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("runtime");
    let abi_probe = work.join("ml_abi_probe.c");
    std::fs::write(
        &abi_probe,
        "#include \"ml_abi.h\"\n#include \"ml_abi.h\"\n\
         int ml_abi_probe(void) { return ML_ST_OK + ML_ST_INSUFFICIENT_BUFFER; }\n",
    )
    .expect("write ml_abi probe");
    let abi_compile = run_in_msvc_env(
        &vcvars,
        &work,
        &format!(
            "cl /nologo /W4 /WX /std:c11 /c /I\"{}\" \"{}\" /Fo:ml_abi_probe.obj",
            runtime_dir.display(),
            abi_probe.display()
        ),
    );
    assert!(
        abi_compile.status.success(),
        "runtime/ml_abi.h does not compile as C11 under /W4 /WX:\n{}\n{}",
        String::from_utf8_lossy(&abi_compile.stdout),
        String::from_utf8_lossy(&abi_compile.stderr)
    );

    // `SPEC-linkable-bindings` §3-A: the import library is real, checked with the linker's
    // own tool rather than by trusting that a non-empty file is an archive. This is the same
    // discipline as the export set — measured with `dumpbin`, not only with our PE reader.
    //
    // `/exports` and not `/linkermember`, and that is measured rather than assumed: on
    // `discount.lib` from a real `mlc build`, `dumpbin /nologo /exports` exits 0 and prints
    // exactly `ml_iface_hash`, `ml_module_abi_version`, `mlx_discount`. (`/linkermember:1`
    // also works, listing `mlx_discount` and `__imp_mlx_discount`, but it reports archive
    // members rather than the export set this asserts.) Grok flagged the mode as a risk
    // during review; the run above is what settled it.
    let implib_dump = run_in_msvc_env(
        &vcvars,
        &work,
        &format!(
            "dumpbin /nologo /exports \"{}\"",
            discount.import_lib.display()
        ),
    );
    assert!(
        implib_dump.status.success(),
        "dumpbin could not read the packaged import library: {}",
        String::from_utf8_lossy(&implib_dump.stderr)
    );
    let implib_text = String::from_utf8_lossy(&implib_dump.stdout);
    for symbol in ["mlx_discount", "ml_module_abi_version", "ml_iface_hash"] {
        assert!(
            implib_text.contains(symbol),
            "the import library does not offer '{symbol}' to the linker, so a host that \
             includes the header and links would fail to resolve it:\n{implib_text}"
        );
    }

    // `SPEC-linkable-bindings` §3-C: the generated headers compile as C++ too.
    //
    // Every header emits `#ifdef __cplusplus extern "C" {`, and until now nothing in the tree
    // had ever run a C++ compiler over one — `HOST_ABI.md` puts C and C++ jointly first, and
    // half of joint first place was unproven. Like N1 this is a coverage extension rather
    // than a fix: measured beforehand, all eighteen already compile.
    //
    // ONE translation unit per header, so each is checked for being self-contained, and all
    // of them in a single `cl` invocation so it costs one process. `/TP` compiles a `.cpp`
    // as C++ regardless of extension; the flags otherwise match the C gate.
    let mut header_stems: Vec<String> = std::fs::read_dir(&*work)
        .expect("read artifact dir")
        .filter_map(|e| {
            let p = e.expect("dir entry").path();
            if p.extension().and_then(|x| x.to_str()) != Some("h") {
                return None;
            }
            Some(p.file_stem()?.to_str()?.to_string())
        })
        .collect();
    header_stems.sort();
    // DERIVED from the corpus, not a literal. It was `>= 18` — written when there were 18
    // examples, so the floor had one header of slack; adding `refund.mls` silently widened
    // that to two, and a hardcoded floor only ever gets looser as the corpus grows. Counting
    // examples/ makes the check say what it means: every example's header is here, plus the
    // drifted fixture the gate builds on top.
    let example_count = std::fs::read_dir(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("examples"),
    )
    .expect("examples/")
    .filter(|e| {
        e.as_ref()
            .map(|e| e.path().extension().and_then(|x| x.to_str()) == Some("mls"))
            .unwrap_or(false)
    })
    .count();
    assert!(
        header_stems.len() >= example_count,
        "expected every example's header in the artifact dir: {} examples but {} headers \
         ({header_stems:?})",
        example_count,
        header_stems.len()
    );

    let mut cpp_units: Vec<String> = Vec::new();
    for stem in &header_stems {
        let name = format!("cpp_{stem}.cpp");
        std::fs::write(
            work.join(&name),
            format!("#include \"{stem}.h\"\nint cpp_probe_{stem}(void) {{ return 0; }}\n"),
        )
        .expect("write C++ probe");
        cpp_units.push(name);
    }
    let cpp = run_in_msvc_env(
        &vcvars,
        &work,
        &format!(
            "cl /nologo /TP /W4 /WX /c /I\"{}\" {}",
            work.display(),
            cpp_units.join(" ")
        ),
    );
    assert!(
        cpp.status.success(),
        "a generated header is not valid C++ under /W4 /WX (the failing file is named \
         below):\n{}\n{}",
        String::from_utf8_lossy(&cpp.stdout),
        String::from_utf8_lossy(&cpp.stderr)
    );

    // Compile the C host against the GENERATED headers (`/I` the artifact dir).
    let compile = run_in_msvc_env(
        &vcvars,
        &work,
        &format!(
            "cl /nologo /W4 /WX /std:c11 /I\"{}\" \"{}\" /Fe:host.exe /Fo:host.obj",
            work.display(),
            host_c.display()
        ),
    );
    assert!(
        compile.status.success(),
        "cl failed to build the C host against the generated headers:\n{}\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr)
    );

    // Run it: LoadLibrary + GetProcAddress + call, in a process that is not ours. Invoke by
    // absolute path — `NoDefaultCurrentDirectoryInExePath` is set on some machines (it is on
    // this one), so cmd will not find `host.exe` in the working directory.
    let run = run_in_msvc_env(
        &vcvars,
        &work,
        &format!(
            "\"{}\" \"{}\" {} pack_drift.dll",
            work.join("host.exe").display(),
            work.display(),
            mlc::ML_MODULE_ABI_VERSION
        ),
    );
    let stdout = String::from_utf8_lossy(&run.stdout);
    // Print it: this transcript IS the acceptance-D evidence (`cargo test -- --nocapture`).
    println!("{stdout}");
    assert!(
        run.status.success() && stdout.contains("GATE_D_OK"),
        "the C host did not pass:\n{stdout}\n{}",
        String::from_utf8_lossy(&run.stderr)
    );
    // GATE_D_OK alone does not say the refusal ran. The drift block is behind `argc >= 4`,
    // and a run that never reached it printed exactly the same OK line (STATUS §9-A A8), so
    // the one check whose whole point is that the gate REFUSES could stop running without
    // anything here noticing. The host now marks which branch it took; this asserts on it.
    assert!(
        stdout.contains("GATE_D_DRIFT_CHECKED"),
        "the C host never exercised the drift refusal — it printed GATE_D_DRIFT_SKIPPED, so \
         the drifted module was not passed on the command line:\n{stdout}"
    );

    // Cross-check our own PE reader against Microsoft's dumpbin on the same file: until now
    // the export measurement (acceptance C) had exactly one implementation — ours.
    for dll in [
        &discount.dll,
        &safe_div.dll,
        &sum_to.dll,
        &negate_if.dll,
        &count_bounded.dll,
        &discount4.dll,
        &line_total.dll,
        &pack.dll,
        &commission.dll,
        &deduction.dll,
        &vat.dll,
        &carrier.dll,
        &quote.dll,
        &receipt.dll,
    ] {
        let mut ours = pe::read_exports(dll).expect("our PE reader");
        ours.sort();
        let theirs = dumpbin_exports(&vcvars, &work, dll);
        assert_eq!(
            ours,
            theirs,
            "our PE reader and dumpbin disagree about {}",
            dll.display()
        );
        // Agreement is not measurement. Two readers that both returned nothing agree
        // perfectly, and the import check below has had this guard since it was written
        // while the export check — the older and more load-bearing of the two, since
        // acceptance C's whole claim is "exactly these three symbols" — did not (STATUS
        // §9-A A4). Assert the shape every module must have, not merely non-emptiness:
        // both reserved symbols plus at least one `mlx_` entry point.
        assert!(
            ours.iter().any(|s| s == "ml_module_abi_version")
                && ours.iter().any(|s| s == "ml_iface_hash")
                && ours.iter().any(|s| s.starts_with("mlx_")),
            "the export set read for {} is {ours:?}; every module must export \
             ml_module_abi_version, ml_iface_hash and at least one mlx_ function. An empty \
             read, or one that lost a reserved symbol, would otherwise match dumpbin's and \
             prove nothing. (It does NOT catch a read that drops one of several mlx_ names \
             while keeping the shape — section_invariants.rs pins the exact count against \
             the module's declarations; this is the floor, not the ceiling.)",
            dll.display()
        );

        // The IMPORT reader is new (SPEC-string-input section 3-C measures "the import set is
        // unchanged"), so give it the same second opinion the export reader gets. Without this
        // a reader that silently returned an empty set would make every import assertion pass.
        let ours = pe::read_imports(dll).expect("our PE import reader");
        let theirs = dumpbin_imports(&vcvars, &work, dll);
        assert_eq!(
            ours,
            theirs,
            "our PE reader and dumpbin disagree about the imports of {}",
            dll.display()
        );
        assert!(!ours.is_empty(), "an empty import set would be suspicious");
    }
}

/// `SPEC-linkable-bindings` §3-B and §3-D — the OTHER way to consume a module.
///
/// Acceptance D proves the dynamic path: `LoadLibrary`, `GetProcAddress`, a function pointer
/// per export. That was the only consumption path this repository had ever proved, and a C
/// programmer handed a `.h` and a `.dll` usually does the other thing — include the header,
/// link the import library, call the function. Until the `.lib` shipped, that did not work.
///
/// So this builds `hosts/c-host-link/host.c`, which contains no `LoadLibrary` and no
/// `GetProcAddress` at all, against the packaged import library, and runs it twice:
///
///   1. beside the module it was built for  -> the gate passes and the values are right;
///   2. beside a DRIFTED module             -> the gate refuses, with a distinct exit code.
///
/// The second run is the one that matters. A linked host resolves its symbols either way —
/// the drifted module exports the same names with the same C types — so nothing but the
/// fingerprint comparison stands between it and a plausible wrong answer. That is the point
/// `SPEC-iface-hash` §5.1 makes about hosts that skip the check, and a linked host is more
/// exposed to it, not less.
#[test]
fn a_c_host_that_links_against_the_import_library() {
    let Some(vcvars) = vcvars64() else {
        if std::env::var("MATHLESS_GATE_D").as_deref() == Ok("require") {
            panic!(
                "MATHLESS_GATE_D=require but MSVC was not found — the link-binding gate \
                 cannot be verified. Install the VS Build Tools with the C++ workload."
            );
        }
        println!(
            "GATE_LINK_SKIPPED: no MSVC toolchain found. Link-time binding is NOT verified \
             by this run."
        );
        return;
    };

    let work = common::TempOut::new("gate_link");
    let arts = emit_artifacts(
        include_str!("../../../examples/discount.mls"),
        "discount",
        &work,
    )
    .expect("emit discount");

    let host_c = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("c-host-link")
        .join("host.c");
    assert!(host_c.exists(), "missing {}", host_c.display());

    // The whole point: the import library is on the link line, and the header supplies the
    // declarations. No adapter, no cast, no function-pointer typedef to get wrong.
    let compile = run_in_msvc_env(
        &vcvars,
        &work,
        &format!(
            "cl /nologo /W4 /WX /std:c11 /I\"{}\" \"{}\" /Fe:host_link.exe /Fo:host_link.obj \
             /link \"{}\"",
            work.display(),
            host_c.display(),
            arts.import_lib.display()
        ),
    );
    assert!(
        compile.status.success(),
        "cl could not build a host that links against the import library:\n{}\n{}",
        String::from_utf8_lossy(&compile.stdout),
        String::from_utf8_lossy(&compile.stderr)
    );

    let exe = work.join("host_link.exe");
    let abi = mlc::ML_MODULE_ABI_VERSION;

    // 1. Beside the module it was built for.
    let ok = run_in_msvc_env(&vcvars, &work, &format!("\"{}\" {abi}", exe.display()));
    let ok_out = String::from_utf8_lossy(&ok.stdout);
    assert!(
        ok.status.success() && ok_out.contains("LINK_GATE_OK"),
        "the linked host did not run clean: status={:?}\n{ok_out}",
        ok.status.code()
    );
    println!("{}", ok_out.trim_end());

    // 2. Beside a drifted module. Same export names and the same C signature — only a
    //    parameter NAME and the rate change — so every symbol still resolves and a host that
    //    skipped the gate would quietly return 50.0 where it expects 90.0. DP-H1 puts
    //    parameter names in the fingerprint precisely for this.
    let drift_src = work.join("driftsrc");
    let drifted = emit_artifacts(
        "export fn discount(price: f64, member: bool) -> f64 {\n\
         \x20 if member { return price * 0.5 }\n\
         \x20 return price\n\
         }\n",
        "discount",
        &drift_src,
    )
    .expect("emit drifted discount");

    let drift_run = work.join("driftrun");
    std::fs::create_dir_all(&drift_run).expect("create drift run dir");
    std::fs::copy(&exe, drift_run.join("host_link.exe")).expect("copy exe");
    std::fs::copy(&drifted.dll, drift_run.join("discount.dll")).expect("copy drifted dll");

    let refused = run_in_msvc_env(
        &vcvars,
        &drift_run,
        &format!("\"{}\" {abi}", drift_run.join("host_link.exe").display()),
    );
    let refused_out = String::from_utf8_lossy(&refused.stdout);
    assert_eq!(
        refused.status.code(),
        Some(3),
        "the linked host must refuse a drifted module with the fingerprint exit code, \
         got {:?}\n{refused_out}",
        refused.status.code()
    );
    assert!(
        refused_out.contains("refuse: interface"),
        "the refusal must say which check failed:\n{refused_out}"
    );
    println!("{}", refused_out.trim_end());
    println!("GATE_LINK_OK: link-time binding verified, and a drifted module refused.");
}

/// A generated header must compile where a real host actually reads it — **after** the
/// platform headers that host includes.
///
/// `hosts/c-host/host.c` includes `<windows.h>` before every generated header. The C++ gate
/// above and the header gate in the same file compile the generated headers *alone*, so
/// nothing had ever read one in that context. Measured, with `mlc build` reporting **exit 0**
/// and writing all four artifacts each time:
///
/// | parameter name | emitted declaration | `cl /W4 /WX /std:c11` after `<windows.h>` |
/// |---|---|---|
/// | `TRUE`     | `double mlx_f(double TRUE);`     | C2059, C2143 — the preprocessor made it `double 1` |
/// | `FALSE`    | `double mlx_f(double FALSE);`    | C2059, C2143 |
/// | `VOID`     | `double mlx_f(double VOID);`     | C2632 — `double void` |
/// | `small`    | `double mlx_f(double small);`    | C2632 — `double char` |
/// | `WINAPI`   | `double mlx_f(double WINAPI);`   | C2220 — `double __stdcall` |
/// | `CALLBACK` | `double mlx_f(double CALLBACK);` | C2220 |
/// | `ERROR`    | `double mlx_f(double ERROR);`    | C2059, C2143 — `double 0` |
///
/// `IN`, `OUT`, `CONST` and `interface` were refused — but **by accident**, as Pascal reserved
/// words, exactly the accident that hid `class` until #159.
///
/// This is the third time a name rule has been written for one reader and met another: C
/// keywords missed C++ keywords, keywords missed `<stdint.h>` macros, and the header alone
/// missed the headers a host includes before it. The set of macros in headers this project
/// does not own is unbounded, so the fix is not a fourth list — the names go where the
/// preprocessor cannot reach them (a comment is removed in translation phase 3, before macro
/// expansion in phase 4). This test is what keeps that true.
#[test]
fn a_generated_header_compiles_after_the_platform_headers_a_host_includes() {
    let Some(vcvars) = vcvars64() else {
        if std::env::var("MATHLESS_GATE_D").as_deref() == Ok("require") {
            panic!(
                "MATHLESS_GATE_D=require but MSVC was not found — this gate cannot be verified."
            );
        }
        println!("GATE_D_SKIPPED: no MSVC toolchain; the platform-header gate is NOT verified.");
        return;
    };
    let work = common::TempOut::new("hdr_after_platform");

    // Every one of these is a macro `<windows.h>` (or a header it pulls in) defines, and every
    // one is a name a person could reasonably give a business parameter. They are deliberately
    // NOT in any reserved list — the point is that no list is needed.
    let hostile = [
        "TRUE",
        "FALSE",
        "VOID",
        "WINAPI",
        "CALLBACK",
        "ERROR",
        "small",
        "IN",
        "OUT",
        "CONST",
        "INT32_MAX",
        "SIZE_MAX",
    ];
    let mut units: Vec<String> = Vec::new();
    let mut built: Vec<&str> = Vec::new();
    for name in hostile {
        let src = format!("export fn probe({name}: f64) -> f64 {{ return {name} }}\n");
        let stem = format!("hp_{}", name.to_ascii_lowercase());
        // Some of these are refused by the frontend for other reasons (Pascal words, stdint
        // macros). That is fine and not what this test is about — it only compiles the headers
        // that were actually produced.
        if emit_artifacts(&src, &stem, &work).is_err() {
            continue;
        }
        built.push(name);
        let unit = format!("u_{stem}.c");
        std::fs::write(
            work.join(&unit),
            format!(
                "#include <windows.h>\n#include \"{stem}.h\"\nint probe_{stem}(void) {{ return 0; }}\n"
            ),
        )
        .expect("write probe TU");
        units.push(unit);
    }
    assert!(
        built.len() >= 6,
        "the frontend now refuses too many of these for this gate to mean anything: built \
         {built:?}. If a name became reserved for a good reason, replace it here with another \
         macro from a header a host includes — do not let the corpus shrink to nothing"
    );

    let out = run_in_msvc_env(
        &vcvars,
        &work,
        &format!(
            "cl /nologo /W4 /WX /std:c11 /c /I\"{}\" {}",
            work.display(),
            units.join(" ")
        ),
    );
    assert!(
        out.status.success(),
        "a generated header does not compile after <windows.h>, which is exactly how \
         hosts/c-host/host.c reads it. Names built into this run: {built:?}\n{}\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );

    // ...and as C++, where the same declarations also meet the C++ keywords.
    let cpp_units: Vec<String> = units
        .iter()
        .map(|u| {
            let c = u.replace(".c", ".cpp");
            std::fs::copy(work.join(u), work.join(&c)).expect("copy TU");
            c
        })
        .collect();
    let cpp = run_in_msvc_env(
        &vcvars,
        &work,
        &format!(
            "cl /nologo /TP /W4 /WX /c /I\"{}\" {}",
            work.display(),
            cpp_units.join(" ")
        ),
    );
    assert!(
        cpp.status.success(),
        "a generated header does not compile after <windows.h> as C++:\n{}\n{}",
        String::from_utf8_lossy(&cpp.stdout),
        String::from_utf8_lossy(&cpp.stderr)
    );
    println!(
        "GATE_PLATFORM_HEADERS_OK: {} headers compiled after <windows.h> as C and C++, \
              with parameter names that are macros there: {built:?}",
        built.len()
    );
}

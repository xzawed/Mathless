//! WH1 (SPEC-iface-hash §3-A..E) — the interface fingerprint, at the compiler layer.
//!
//! The centre of gravity is what the fingerprint must NOT notice. Criteria D and E are
//! the safety catch: a reordered export list and a changed function *body* both keep the
//! fingerprint, so "swap the module file, do not rebuild the host" survives for the edits
//! §3a-5 measured. Criterion B is the one that pays for the slice — it is the case a
//! type-only fingerprint cannot see, because the two C declarations are identical.

use mlc::{compile_to_ir, iface};

fn hash(src: &str) -> u64 {
    iface::fingerprint(&compile_to_ir(src).expect("compile"))
}

fn manifest(src: &str) -> String {
    iface::manifest(&compile_to_ir(src).expect("compile"))
}

// ---------------------------------------------------------------- §3-A determinism

#[test]
fn same_source_hashes_the_same_every_time() {
    let src = "export fn discount(price: f64, vip: bool) -> f64 { if vip { return price * 0.9 } return price }";
    assert_eq!(hash(src), hash(src), "fingerprint must be deterministic");
}

#[test]
fn the_manifest_is_ascii_only() {
    // Generated artifacts stay ASCII (MSVC C4819 breaks the /WX build otherwise), and the
    // manifest is what the hash is computed over, so it is held to the same rule.
    let m = manifest("export fn f(a: f64) -> f64 { return a }");
    assert!(m.is_ascii(), "manifest must be ASCII: {m:?}");
}

#[test]
fn the_manifest_has_the_documented_shape() {
    let m = manifest("export fn discount(price: f64, vip: bool) -> f64 { return price }");
    assert_eq!(
        m, "ml-iface/1\nabi=1\nfn discount(price:f64, vip:bool) -> f64\n",
        "manifest shape is a contract (SPEC §2.1) — hosts never see it, but the hash is\n\
         only reproducible if this text is"
    );
}

// ------------------------------------------------- §3-B parameter NAMES are covered

#[test]
fn reordering_parameters_changes_the_fingerprint() {
    // The measured silent-wrong-value drift. Both compile to the SAME C declaration:
    //   int32_t mlx_boxes(int32_t, int32_t)
    // so a fingerprint over C types alone would call these two modules interchangeable.
    let v1 = "export fn boxes(items: i32, per: i32) -> i32 { return items / per }";
    let v2 = "export fn boxes(per: i32, items: i32) -> i32 { return items / per }";
    assert_ne!(
        hash(v1),
        hash(v2),
        "swapped parameters must not share a fingerprint (DP-H1)"
    );
}

#[test]
fn renaming_a_parameter_changes_the_fingerprint() {
    // The acknowledged cost of DP-H1: a pure rename is a false rejection. Pinned so it is
    // a decision on record, not an accident (SPEC §5.3).
    let a = "export fn f(amount: f64) -> f64 { return amount }";
    let b = "export fn f(sales: f64) -> f64 { return sales }";
    assert_ne!(hash(a), hash(b));
}

// -------------------------------------------------------- §3-C types are covered

#[test]
fn changing_a_parameter_type_changes_the_fingerprint() {
    // The measured crash drift: the host passes an int, v2 dereferences it as a pointer.
    let v1 = "export fn rate(code: i32) -> f64 { if code == 1 { return 0.1 } return 0.0 }";
    let v2 = "export fn rate(code: string) -> f64 { if code == \"KR\" { return 0.1 } return 0.0 }";
    assert_ne!(hash(v1), hash(v2));
}

#[test]
fn changing_the_return_type_changes_the_fingerprint() {
    let a = "export fn f(x: f64) -> f64 { return x }";
    let b = "export fn f(x: f64) -> i32 { return x as i32 }";
    assert_ne!(hash(a), hash(b));
}

#[test]
fn making_a_function_fallible_changes_the_fingerprint() {
    // `-> f64!` widens the C declaration to status + out-param. A host built against the
    // non-fallible form would read the return value as the answer.
    let a = "export fn f(x: f64) -> f64 { return x }";
    let b = "error E = 1\nexport fn f(x: f64) -> f64! { if x < 0.0 { fail E } return x }";
    assert_ne!(hash(a), hash(b));
}

#[test]
fn an_out_parameter_is_distinguished_from_a_value_parameter() {
    // #80's own failure mode: `out_tier: i32` declared by value compiled and did nothing.
    let a = "export fn f(x: f64, t: i32) -> f64 { return x }";
    let b = "export fn f(x: f64, out t: i32) -> f64 { t = 1  return x }";
    assert_ne!(hash(a), hash(b));
    assert!(
        manifest(b).contains("out t:i32"),
        "out-ness must be spelled in the manifest: {}",
        manifest(b)
    );
}

// ------------------------------------------ §3-D / §3-E what must NOT change the hash

#[test]
fn reordering_the_export_list_keeps_the_fingerprint() {
    // Hosts resolve by name; declaration order reaches no host. Sorting the manifest
    // (DP-H4) is what stops a harmless edit from becoming a false rejection.
    let a = "export fn a(x: f64) -> f64 { return x }\nexport fn b(y: i32) -> i32 { return y }";
    let b = "export fn b(y: i32) -> i32 { return y }\nexport fn a(x: f64) -> f64 { return x }";
    assert_eq!(hash(a), hash(b), "export order must not matter (DP-H4)");
}

#[test]
fn changing_only_a_function_body_keeps_the_fingerprint() {
    // THE criterion. ARCHITECTURE.md:68 promises a module swap needs no host rebuild, and
    // §3a-5 measured that threshold edits are exactly that case. If this test ever fails,
    // the slice has broken the project's central promise, not merely a test.
    let a = "export fn discount(price: f64, vip: bool) -> f64 { if vip { return price * 0.9 } return price }";
    let b = "export fn discount(price: f64, vip: bool) -> f64 { if vip { return price * 0.8 } return price }";
    assert_eq!(
        hash(a),
        hash(b),
        "a body-only edit must keep the fingerprint"
    );
}

#[test]
fn adding_an_internal_function_keeps_the_fingerprint() {
    // Internal `fn` is not exported (#101), so it is not part of the host contract.
    let a = "export fn f(x: f64) -> f64 { return x * 2.0 }";
    let b = "fn double(x: f64) -> f64 { return x * 2.0 }\nexport fn f(x: f64) -> f64 { return double(x) }";
    assert_eq!(
        hash(a),
        hash(b),
        "internal functions are not part of the contract"
    );
}

#[test]
fn renaming_a_local_variable_keeps_the_fingerprint() {
    let a = "export fn f(x: f64) -> f64 { let t = x * 2.0  return t }";
    let b = "export fn f(x: f64) -> f64 { let u = x * 2.0  return u }";
    assert_eq!(hash(a), hash(b));
}

// ---------------------------------------------------------------- §2.1 error table

#[test]
fn renumbering_an_error_changes_the_fingerprint() {
    // ML_ERR_* is compiled INTO the host from the generated header. Renumbering it while
    // the host keeps the old value is the same silent misreading as a swapped parameter.
    let a = "error E_BAD = 1\nexport fn f(x: f64) -> f64! { if x < 0.0 { fail E_BAD } return x }";
    let b = "error E_BAD = 7\nexport fn f(x: f64) -> f64! { if x < 0.0 { fail E_BAD } return x }";
    assert_ne!(
        hash(a),
        hash(b),
        "error codes are part of the contract (DP-H6)"
    );
}

#[test]
fn declaring_errors_in_a_different_order_keeps_the_fingerprint() {
    let a =
        "error A = 1\nerror B = 2\nexport fn f(x: f64) -> f64! { if x < 0.0 { fail A } return x }";
    let b =
        "error B = 2\nerror A = 1\nexport fn f(x: f64) -> f64! { if x < 0.0 { fail A } return x }";
    assert_eq!(
        hash(a),
        hash(b),
        "error declaration order must not matter (DP-H4)"
    );
}

// ------------------------------------------ WH3/WH4 the value reaches the artifacts

/// The one number that matters: the module's exported value and the constant burned into
/// the host's header must be the SAME. They are produced by different files (`codegen.rs`
/// and `header.rs`), which is exactly the split #101 measured as unable to prove itself —
/// so it is pinned here rather than assumed.
#[test]
fn codegen_and_header_agree_on_the_value() {
    let src = "export fn discount(price: f64, vip: bool) -> f64 { if vip { return price * 0.9 } return price }";
    let ir = compile_to_ir(src).expect("compile");
    let expected = iface::fingerprint(&ir);

    let rust = mlc::codegen::emit(&ir, "discount").expect("codegen");
    assert!(
        rust.contains(&format!(
            "pub extern \"C\" fn ml_iface_hash_discount() -> u64 {{ 0x{expected:016X} }}"
        )),
        "generated Rust must export the fingerprint:\n{rust}"
    );

    let h = mlc::header::emit_c_header(&ir, "discount");
    assert!(
        h.contains(&format!(
            "#define ML_DISCOUNT_IFACE_HASH 0x{expected:016X}ULL"
        )),
        "header must pin the same value:\n{h}"
    );
    assert!(
        h.contains("uint64_t ml_iface_hash_discount(void);"),
        "header must declare the export:\n{h}"
    );

    let p = mlc::header::emit_delphi_unit(&ir, "discount");
    assert!(
        p.contains(&format!(
            "ML_DISCOUNT_IFACE_HASH: UInt64 = UInt64(${expected:016X});"
        )),
        "Delphi unit must pin the same value:\n{p}"
    );
    assert!(
        p.contains("function ml_iface_hash_discount: UInt64; cdecl; external ML_MODULE;"),
        "Delphi unit must declare the export:\n{p}"
    );
}

/// The Delphi constant must not depend on how the compiler types a hex literal.
///
/// Measured with Free Pascal 3.2.2 in `-Mdelphi` mode, the only Pascal compiler that has ever
/// read these units: `H: UInt64 = $8120E9C099B13F94;` is accepted, holds the RIGHT value, and
/// warns — "range check error while evaluating constants (-9142050230140584044 must be
/// between 0 and 18446744073709551615)". The literal is typed as a signed Int64 first, and
/// every fingerprint with the top bit set trips it: **11 of the 19 example modules**. Nothing
/// is wrong with the value; a consumer building with warnings-as-errors simply cannot build.
/// That is the same class as the C header's `/W4 /WX`, which this repo already holds itself to.
///
/// `UInt64($...)` compiles clean and carries the same value (measured, all three spellings
/// print 9304693843568967572). This test pins the CAST rather than the whole line, so it fails
/// for the reason it exists rather than on unrelated formatting.
#[test]
fn the_delphi_fingerprint_does_not_rely_on_literal_type_inference() {
    // A module whose fingerprint has the top bit set is what triggers it. Rather than pick one
    // and hope it keeps that property, search the corpus for one and say so if there is none.
    let sources = [
        "export fn f(x: f64) -> f64 { return x }",
        "export fn g(a: i32, b: i32) -> i32 { return a + b }",
        "export fn h(s: string) -> i32! { return 1 }",
        "export fn k(x: i32, out t: i32) -> i32 { t = x return x }",
    ];
    let mut checked = 0;
    for src in sources {
        let ir = compile_to_ir(src).expect("compile");
        let hash = iface::fingerprint(&ir);
        if hash & (1u64 << 63) == 0 {
            continue; // not a case that can trip it
        }
        checked += 1;
        let p = mlc::header::emit_delphi_unit(&ir, "m");
        assert!(
            p.contains(&format!("UInt64(${hash:016X})")),
            "a fingerprint with the top bit set must be cast, not left to the compiler to              type — Free Pascal reads the bare literal as a negative Int64 and warns:
{p}"
        );
    }
    assert!(
        checked > 0,
        "no source in this test produced a top-bit-set fingerprint, so it checked nothing"
    );
}

/// A generated header must never open a `/*` while one is already open.
///
/// This is not hypothetical: the first draft of the fingerprint note contained a nested
/// `/* ... */` in its usage example. C closes a comment at the FIRST `*/`, so the rest of
/// the block became code — MSVC C4138 and a broken `/W4 /WX` build. Every `contains()`
/// assertion still passed, because the text really was in the file. That is the failure
/// §7 records as "a test that pins text does not know whether the text is valid".
/// `Err(byte offset)` at the first `/*` opened inside an already-open comment, or at the
/// end if a comment is left unterminated.
fn first_comment_nesting_fault(src: &str) -> Result<(), usize> {
    let b = src.as_bytes();
    let (mut open, mut i) = (false, 0usize);
    while i + 1 < b.len() {
        match (&b[i..i + 2], open) {
            (b"/*", false) => {
                open = true;
                i += 2;
            }
            (b"/*", true) => return Err(i),
            (b"*/", true) => {
                open = false;
                i += 2;
            }
            _ => i += 1,
        }
    }
    if open {
        return Err(b.len());
    }
    Ok(())
}

#[test]
fn the_nesting_scanner_actually_flags_a_bad_comment() {
    // Without this, the guard below could pass by never detecting anything. The sample is
    // the exact shape the first draft emitted.
    let bad = "/* note\n *   if (x) {\n *       /* do not call */\n *   }\n * more text */\n";
    assert!(
        first_comment_nesting_fault(bad).is_err(),
        "the scanner must reject a nested comment, or it proves nothing"
    );
    assert!(first_comment_nesting_fault("/* fine */ int x; /* also fine */").is_ok());
    assert!(first_comment_nesting_fault("/* never closed").is_err());
}

#[test]
fn generated_headers_never_nest_a_comment() {
    for src in [
        "export fn f(x: f64) -> f64 { return x }",
        "error E = 1\nexport fn g(s: string, out t: i32) -> string! { t = 1  return \"x\" }",
    ] {
        let h = mlc::header::emit_c_header(&compile_to_ir(src).expect("compile"), "m");
        if let Err(at) = first_comment_nesting_fault(&h) {
            panic!(
                "comment nesting fault at byte {at}; C closes a comment at the FIRST `*/`, \
                 so everything after it compiles as code:\n{h}"
            );
        }
    }
}

#[test]
fn the_header_constant_is_module_prefixed() {
    // DP-H10. Unprefixed `ML_ERR_*` already collides when one host includes two generated
    // headers (STATUS §5-5.4); the new constant must not repeat that.
    let ir = compile_to_ir("export fn f(x: f64) -> f64 { return x }").expect("compile");
    let a = mlc::header::emit_c_header(&ir, "alpha");
    let b = mlc::header::emit_c_header(&ir, "beta");
    assert!(a.contains("#define ML_ALPHA_IFACE_HASH "), "{a}");
    assert!(b.contains("#define ML_BETA_IFACE_HASH "), "{b}");
}

#[test]
fn a_body_only_edit_leaves_the_header_byte_identical() {
    // The host-facing half of §3-E: not only does the fingerprint survive a threshold
    // change, the whole generated header does — so nothing about the host needs to move.
    let a = compile_to_ir("export fn f(x: f64) -> f64 { return x * 0.9 }").expect("compile");
    let b = compile_to_ir("export fn f(x: f64) -> f64 { return x * 0.8 }").expect("compile");
    assert_eq!(
        mlc::header::emit_c_header(&a, "m"),
        mlc::header::emit_c_header(&b, "m")
    );
}

// ---------------------------------------------------------------- FNV-1a itself

#[test]
fn fnv1a64_matches_the_published_vectors() {
    // E1: the algorithm is pinned against vectors published with FNV, so a refactor of our
    // 10-line implementation cannot silently change every module's fingerprint.
    assert_eq!(iface::fnv1a64(b""), 0xcbf2_9ce4_8422_2325);
    assert_eq!(iface::fnv1a64(b"a"), 0xaf63_dc4c_8601_ec8c);
    assert_eq!(iface::fnv1a64(b"foobar"), 0x8594_4171_f739_67e8);
}

/// SPEC §2.1 / DP-H4: both top-level lists are sorted **by name**, byte-ascending. The doc
/// comment on `manifest` says the same thing, twice.
///
/// The code sorted the RENDERED LINES instead, which is a different order whenever one name
/// is a prefix of another and the next character sorts below `=` (0x3D) — every digit does.
/// With `error E_1 = 1` and `error E_10 = 2` the manifest came out
///
/// ```text
/// err E_10=2
/// err E_1=1
/// ```
///
/// because `'0'` (0x30) is below `'='`. Nothing misbehaves today — the module and the header
/// both get their value from `fingerprint`, so they agree with each other — but the manifest
/// is the *contract* for anyone recomputing it (the D19 C-emit backend, a host-side checker),
/// and it was not the contract the spec and the comment describe.
#[test]
fn the_error_lines_are_sorted_by_name_not_by_rendered_line() {
    let m = manifest(
        "error E_10 = 2\n\
         error E_1 = 1\n\
         export fn f(x: f64) -> f64! { if x < 0.0 { fail E_1 }  return x }",
    );
    let errs: Vec<&str> = m.lines().filter(|l| l.starts_with("err ")).collect();
    assert_eq!(
        errs,
        vec!["err E_1=1", "err E_10=2"],
        "byte-ascending BY NAME, so E_1 precedes E_10:\n{m}"
    );
}

/// The same rule for `fn` lines, which the spec states first.
///
/// This one was already right, and for a reason worth writing down rather than rediscovering:
/// a `fn` line puts `(` (0x28) after the name, and `(` sorts below every character an
/// identifier can contain, so a shorter name always came first either way. Sorting by name is
/// now what the code says as well as what it did — and this test would catch a future
/// separator that does not have that property.
#[test]
fn the_fn_lines_are_sorted_by_name_including_the_prefix_case() {
    let m = manifest(
        "export fn f10(x: f64) -> f64 { return x }\n\
         export fn f1(x: f64) -> f64 { return x }\n\
         export fn f(x: f64) -> f64 { return x }",
    );
    let fns: Vec<&str> = m
        .lines()
        .filter(|l| l.starts_with("fn "))
        .map(|l| l.split('(').next().unwrap())
        .collect();
    assert_eq!(fns, vec!["fn f", "fn f1", "fn f10"], "{m}");
}

// ------------------------------------------------- the mutations nothing else covered

/// **Every remaining edit a host could be hurt by, and what the fingerprint does about it.**
///
/// The per-case tests above cover ten mutations, each with the reasoning that earned it. This
/// table covers the rest, found by listing what a host depends on and checking the list
/// against the file rather than the other way round (`STATUS.md` §9-59).
///
/// One row is why the table exists at all: `ir.rs`'s `Display for IrType` carries the comment
/// *"The manifest quotes this, so an array parameter changes the fingerprint and a changed
/// element type changes it again (SPEC-array-input §2.7)"* — and **nothing measured it**. A
/// claim in a comment about the fingerprint is the same shape as a claim in a document about
/// a constant, which `doc_claims.rs` exists to refuse.
///
/// The `same` rows matter as much as the `differs` ones: a fingerprint that changes for a
/// non-contract edit rejects a module the host could have used, and "swap the file, do not
/// rebuild the host" is the property §3-D/E exist to protect.
#[test]
fn the_remaining_edits_change_the_fingerprint_exactly_when_they_change_the_contract() {
    // (label, before, after, must the fingerprint differ?)
    let cases: &[(&str, &str, &str, bool)] = &[
        // A host resolves by symbol name, so a renamed export is a different symbol — every
        // host built against the old one fails at `GetProcAddress` rather than silently. The
        // fingerprint changing too is belt-and-braces, but a fingerprint that did NOT change
        // would mean the manifest was not quoting the name at all, and then a SWAP of two
        // functions' names would be invisible.
        (
            "function renamed",
            "export fn a(x: f64) -> f64 { return x }",
            "export fn b(x: f64) -> f64 { return x }",
            true,
        ),
        // The pair the row above is really about: two exports trade names. Both symbols still
        // resolve, and the host calls each through the other's declaration — a double where a
        // pointer is expected, which is the `0xC0000005` this slice's module doc measured.
        //
        // The first draft of this row gave both functions the SAME signature and swapped only
        // their bodies, then asserted the fingerprint must change. It must not: that is a body
        // edit, and §3-E exists to let body edits through. Two exports with identical
        // signatures trading behaviour is invisible here **by design** — the fingerprint
        // states the contract, and both spellings satisfy the same contract.
        (
            "two functions swap names",
            "export fn a(x: f64) -> f64 { return x }\n\
             export fn b(s: string) -> string! { return s }",
            "export fn b(x: f64) -> f64 { return x }\n\
             export fn a(s: string) -> string! { return s }",
            true,
        ),
        // Adding an export is a contract change in the direction that cannot hurt an old host
        // — it resolves nothing new — but the fingerprint is a two-way check: a host built
        // against the NEW module and handed the old one must be refused, and that is this row
        // read right to left.
        (
            "an export added",
            "export fn a(x: f64) -> f64 { return x }",
            "export fn a(x: f64) -> f64 { return x }\nexport fn b(x: f64) -> f64 { return x }",
            true,
        ),
        // `ir.rs`'s documented claim, measured at last.
        (
            "array element type",
            "export fn f(xs: [i32]) -> i32! { return xs[0] }",
            "export fn f(xs: [f64]) -> f64! { return xs[0] }",
            true,
        ),
        // Same name, same arity — and a completely different calling convention, because an
        // array parameter is TWO parameters at the boundary (pointer + length).
        (
            "scalar parameter becomes an array",
            "export fn f(xs: i32) -> i32 { return xs }",
            "export fn f(xs: [i32]) -> i32! { return xs[0] }",
            true,
        ),
        // Error NAMES are compiled into the host as `ML_<MODULE>_ERR_<NAME>`, so a rename is
        // the same class as a renumber (which §3 already covers): the host's macro no longer
        // exists, or worse, still exists with a stale value from a previous build.
        (
            "an error renamed",
            "error E_ONE = 1\nexport fn f(x: f64) -> f64! { if x < 0.0 { fail E_ONE } return x }",
            "error E_TWO = 1\nexport fn f(x: f64) -> f64! { if x < 0.0 { fail E_TWO } return x }",
            true,
        ),
        // A second error declaration changes the set of positive statuses a host may see.
        (
            "an error added",
            "error E_ONE = 1\nexport fn f(x: f64) -> f64! { if x < 0.0 { fail E_ONE } return x }",
            "error E_ONE = 1\nerror E_TWO = 2\nexport fn f(x: f64) -> f64! { if x < 0.0 { fail E_TWO } return x }",
            true,
        ),
        // NOT a contract change: which RESERVED negative statuses a body can produce. D17
        // makes negatives the ABI's, not the module's — a host must already treat any
        // negative as a failure — so this is a body edit like any other, and the header
        // naming `ML_ST_INDEX_OUT_OF_RANGE` (#238) is a convenience for a reader, not a new
        // obligation. Measured here so that reasoning is a test rather than a paragraph.
        (
            "a body gains the ability to report -2",
            "export fn f(s: string) -> string! { return s }",
            "export fn f(s: string) -> string! { return byte_slice(s, 0, 2) }",
            false,
        ),
        // The encoding of the bytes the module hands out. A contract change since 2026-09-15
        // (user-confirmed, `SPEC-iface-hash` §2.1): the signature is identical and so is the
        // unit of `ml_needed`, but the `.h` and `.pas` gain a UTF-8 notice, and a host built
        // against the ASCII form used to load the other one, pass the fingerprint check, and
        // render mojibake on a code page 949 Delphi host — silently, because nothing fails.
        //
        // The contrast that decided it is the row above: a reserved negative status is the
        // ABI's, so a host already handles every one. Encoding has no such rule (DP-S2 left
        // it undecided), so there is nothing a host can have been told in advance.
        (
            "a returned literal stops being ASCII",
            "export fn label() -> string! { return \"OK\" }",
            "export fn label() -> string! { return \"\u{c2b9}\u{c778}\" }",
            true,
        ),
        // NOT a contract change: an internal function's signature. Nothing exports it.
        (
            "an internal function's signature",
            "fn helper(x: f64) -> f64 { return x }\nexport fn f(x: f64) -> f64 { return helper(x) }",
            "fn helper(x: f64, unused: f64) -> f64 { return x }\nexport fn f(x: f64) -> f64 { return helper(x, 0.0) }",
            false,
        ),
    ];

    for (label, before, after, must_differ) in cases {
        let (a, b) = (hash(before), hash(after));
        assert_eq!(
            a != b,
            *must_differ,
            "{label}: the fingerprint {} have changed\n  before: {}  after: {}",
            if *must_differ { "must" } else { "must NOT" },
            manifest(before).replace('\n', " | "),
            manifest(after).replace('\n', " | ")
        );
    }
}

/// **An ASCII module carries no `utf8` line — which is why this change cost one fingerprint.**
///
/// `utf8=1` is emitted only when true, exactly like the `err` lines, and that is not a
/// micro-optimisation: it is what keeps every ASCII module's manifest byte-identical to what
/// it was before the field existed. Bumping `ml-iface/1` to `/2` would have been the other
/// way to signal a format change, and it would have moved EVERY module's fingerprint — a
/// false rejection for every module whose contract did not change, which is precisely what
/// §3-D and §3-E exist to prevent.
///
/// Measured when it landed: of the whole example corpus exactly one fingerprint moved
/// (`claim`, the only module that returns Korean labels), one line per golden file.
#[test]
fn the_encoding_line_appears_only_when_the_module_returns_non_ascii() {
    let ascii = manifest("export fn label() -> string! { return \"OK\" }");
    assert!(
        !ascii.contains("utf8"),
        "an ASCII module's manifest must be unchanged by this field: {ascii:?}"
    );

    let korean = manifest("export fn label() -> string! { return \"\u{c2b9}\u{c778}\" }");
    assert!(korean.contains("utf8=1\n"), "{korean:?}");

    // Position matters, because the manifest is a byte string and the SPEC fixes its shape:
    // module-level lines come before the per-function lines.
    assert!(
        korean.starts_with("ml-iface/1\nabi=1\nutf8=1\nfn "),
        "the encoding line is module-level and precedes the fn lines: {korean:?}"
    );

    // Still ASCII — the field says the module's OUTPUT is UTF-8, it does not make the
    // manifest so. `the_manifest_is_ascii_only` covers the general rule; this covers the one
    // module shape that could plausibly break it.
    assert!(korean.is_ascii(), "{korean:?}");

    // And the notice in both bindings agrees with the manifest, because all three now ask the
    // same walker (`ir::returns_non_ascii_bytes`) rather than each carrying a copy.
    let ir = mlc::compile_to_ir("export fn label() -> string! { return \"\u{c2b9}\u{c778}\" }")
        .expect("compile");
    assert!(mlc::header::emit_c_header(&ir, "m").contains("UTF-8"));
    assert!(mlc::header::emit_delphi_unit(&ir, "m").contains("UTF-8"));
}

/// **Counting a literal is not copying it** — the over-emission the `utf8=1` line exposed.
///
/// `returns_non_ascii_bytes` used to ask "is there a non-ASCII literal anywhere", and
/// `byte_len("승인")` answered yes. That function returns the integer 6; not one byte of UTF-8
/// crosses the boundary. While the answer only drove a comment in the header it was merely
/// wrong; once the manifest read the same walker it would have moved that module's
/// fingerprint for a contract that did not change — a false rejection, which is what §3-D and
/// §3-E exist to prevent.
///
/// The positions are the whole test: a literal's bytes leave through a `return`, through a
/// concatenation piece, and as the source of a returned span. They do not leave through
/// `byte_len`, through a comparison, or through `fixed`.
#[test]
fn only_a_literal_whose_bytes_leave_the_module_sets_the_encoding_line() {
    const KO: &str = "\u{c2b9}\u{c778}";

    // Out: the bytes reach `ml_buf`.
    for src in [
        format!("export fn f() -> string! {{ return \"{KO}\" }}"),
        format!("export fn f(s: string) -> string! {{ return s + \"{KO}\" }}"),
    ] {
        let m = manifest(&src);
        assert!(
            m.contains("utf8=1"),
            "these bytes leave the module: {m:?}\n{src}"
        );
    }

    // Not out: the literal is measured, not written. `byte_len` yields an i32.
    let counted = manifest(&format!(
        "export fn f() -> i32 {{ return byte_len(\"{KO}\") }}"
    ));
    assert!(
        !counted.contains("utf8"),
        "counting a literal writes none of it to the host: {counted:?}"
    );

    // …and the two bindings agree with the manifest, because all three ask one walker.
    let ir = compile_to_ir(&format!(
        "export fn f() -> i32 {{ return byte_len(\"{KO}\") }}"
    ))
    .expect("compile");
    assert!(
        !mlc::header::emit_c_header(&ir, "m").contains("UTF-8"),
        "the header claimed the module returns UTF-8 while returning an i32"
    );
    assert!(!mlc::header::emit_delphi_unit(&ir, "m").contains("UTF-8"));
}

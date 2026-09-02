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

    let rust = mlc::codegen::emit(&ir).expect("codegen");
    assert!(
        rust.contains(&format!(
            "pub extern \"C\" fn ml_iface_hash() -> u64 {{ 0x{expected:016X} }}"
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
        h.contains("uint64_t ml_iface_hash(void);"),
        "header must declare the export:\n{h}"
    );

    let p = mlc::header::emit_delphi_unit(&ir, "Mlx_Discount", "discount");
    assert!(
        p.contains(&format!(
            "ML_DISCOUNT_IFACE_HASH: UInt64 = ${expected:016X};"
        )),
        "Delphi unit must pin the same value:\n{p}"
    );
    assert!(
        p.contains("function ml_iface_hash: UInt64; cdecl; external ML_MODULE;"),
        "Delphi unit must declare the export:\n{p}"
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

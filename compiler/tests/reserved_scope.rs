//! STATUS §4-2 — a name is checked against the languages it can actually REACH.
//!
//! Measured 2026-09-02, which is what closed the question:
//!
//!   - an exported function's parameter names are written verbatim into the Delphi unit
//!     (`function mlx_price_of(quantity: Integer; discount_rate: Double)`), so they must
//!     avoid Pascal's reserved words;
//!   - an internal function's parameters and every local appear in **neither** the `.h` nor
//!     the `.pas` — zero occurrences of each — so checking them against Pascal has no reason.
//!
//! Pascal is dropped for those, and **C is kept**: D19 leaves a C-emit backend slot open, and
//! under that backend an internal name would become a C identifier. Delphi cannot be reached
//! by any planned backend — it is a binding target, never a codegen target.
//!
//! This was not hypothetical. Writing a real business rule with a parameter named `unit` was
//! rejected as a Pascal reserved word, in a position Pascal never sees.

use mlc::compile_to_ir;

fn err(src: &str) -> String {
    compile_to_ir(src)
        .expect_err("should not compile")
        .to_string()
}

fn ok(src: &str) {
    compile_to_ir(src).expect("should compile");
}

// ------------------------------------------------- names that DO reach a binding

#[test]
fn an_exported_parameter_still_avoids_pascal() {
    // `unit` opens a Delphi source file. It reaches the generated `.pas`, so it is refused.
    let e = err("export fn f(unit: i32) -> i32 { return unit }");
    assert!(e.contains("Pascal"), "{e}");
    assert!(e.contains("unit"), "{e}");
}

#[test]
fn an_exported_parameter_still_avoids_c_and_rust() {
    assert!(err("export fn f(int: i32) -> i32 { return int }").contains('C'));
    assert!(err("export fn f(match: i32) -> i32 { return match }").contains("Rust"));
}

#[test]
fn the_module_name_is_unchanged_by_this() {
    // The module name becomes the crate name, the header guard AND the unit name, so it keeps
    // being checked against everything. `emit.rs` owns that check; this is a reminder, not a
    // new rule.
    ok("export fn f(price: i32) -> i32 { return price }");
}

// ------------------------------------------- names that reach NOTHING but the module

#[test]
fn an_internal_parameter_may_use_a_pascal_word() {
    // Measured: `inner_param` appeared zero times in the generated `.h` and `.pas`. A Pascal
    // keyword in that position cannot collide with anything.
    ok("fn helper(unit: f64) -> f64 { return unit * 2.0 }\n\
        export fn outer(v: f64) -> f64 { return helper(v) }");
    ok("fn helper(record: f64) -> f64 { return record }\n\
        export fn outer(v: f64) -> f64 { return helper(v) }");
}

#[test]
fn a_local_may_use_a_pascal_word() {
    ok("export fn f(v: f64) -> f64 { let unit = v * 2.0  return unit }");
    ok("export fn f(v: f64) -> f64 { let mut set = v  set = set + 1.0  return set }");
}

#[test]
fn an_internal_parameter_still_avoids_rust_and_c() {
    // Rust: the generated module is Rust today. C: D19 keeps the C-emit slot open, and this
    // is the one place a future backend would put the name.
    let e = err("fn helper(match: f64) -> f64 { return match }\n\
                 export fn outer(v: f64) -> f64 { return helper(v) }");
    assert!(e.contains("Rust"), "{e}");

    let e = err("fn helper(int: f64) -> f64 { return int }\n\
                 export fn outer(v: f64) -> f64 { return helper(v) }");
    assert!(e.contains('C'), "{e}");
}

#[test]
fn a_local_still_avoids_rust_and_c() {
    assert!(err("export fn f(v: f64) -> f64 { let match = v  return match }").contains("Rust"));
    assert!(err("export fn f(v: f64) -> f64 { let int = v  return int }").contains('C'));
}

#[test]
fn the_generated_prefix_rule_is_untouched() {
    // Orthogonal and still absolute: `ml_`/`mlx_`/`__` share the emitted scope with codegen's
    // own bindings, and shadowing there is silent — that is the `__d` defect (#85).
    for src in [
        "export fn f(ml_cap: i32) -> i32 { return ml_cap }",
        "fn helper(__d: f64) -> f64 { return __d }\nexport fn o(v: f64) -> f64 { return helper(v) }",
        "export fn f(v: f64) -> f64 { let __t = v  return __t }",
    ] {
        let e = err(src);
        assert!(
            e.contains("compiler generates") || e.contains("reserved"),
            "{src} -> {e}"
        );
    }
}

// ------------------------------------------------------------------ the message

#[test]
fn the_message_names_only_the_targets_that_apply() {
    // `type` is reserved in Rust AND Pascal. In an internal position only Rust applies, and
    // the message must not mention a language the name cannot reach — that is the whole
    // defect being fixed.
    let e = err("fn helper(type: f64) -> f64 { return type }\n\
                 export fn outer(v: f64) -> f64 { return helper(v) }");
    assert!(e.contains("Rust"), "{e}");
    assert!(
        !e.contains("Pascal"),
        "an internal name never reaches Pascal, so the message must not say it does: {e}"
    );
}

// ---------------------------------------- what the artifact reads, not what C reads
//
// The four cases below were all measured to ship a broken artifact with `mlc build`
// reporting success (or, for `_`, to fail inside generated Rust). Each is the same mistake:
// the name was checked against ONE reader of the artifact, and the artifact has more.

/// The generated header carries `#ifdef __cplusplus extern "C" {`, its own preamble says it
/// compiles as C++, and `hosts/rust-oracle/tests/c_host.rs` runs `cl /TP` over every example's
/// header. But the reserved list holds C keywords only, so a C++-only keyword passes.
///
/// Measured: `export fn f(new: f64) -> f64 { return new }` built with exit 0 and emitted
/// `double mlx_f(double new);`. Compiling that header with `cl /nologo /TP /W4 /WX /c`:
///
///     p_cpp_new.h(35): error C2143 / C2059 — syntax error before 'new'
///
/// `template` behaves the same. `class` was already refused — but only by accident, because
/// it is also a *Pascal* reserved word.
#[test]
fn an_exported_parameter_avoids_cpp_keywords_because_the_header_is_also_cpp() {
    for kw in [
        "new",
        "template",
        "delete",
        "throw",
        "namespace",
        "this",
        "operator",
    ] {
        let e = err(&format!("export fn f({kw}: f64) -> f64 {{ return {kw} }}"));
        assert!(
            e.contains("C++"),
            "`{kw}` breaks the header under `cl /TP`, so it must be named as a C++ reserved \
             word: {e}"
        );
    }
    // Still not a global ban: an INTERNAL parameter never reaches the header.
    ok("fn g(new: f64) -> f64 { return new }\n\
        export fn f(x: f64) -> f64 { return g(x) }");
}

/// The generated header `#include <stdint.h>` itself, so every macro that header defines is
/// already in scope by the time the declarations are read — and a macro is *text
/// substitution*, not a name that can be shadowed.
///
/// Measured: `export fn f(INT32_MAX: f64) -> f64 { return INT32_MAX }` built with exit 0 and
/// emitted `double mlx_f(double INT32_MAX);`, which the preprocessor turns into
/// `double mlx_f(double 2147483647);`. `cl` as C **and** as C++:
///
///     p_stdint_macro.h(35): error C2143 / C2059
#[test]
fn an_exported_parameter_avoids_the_macros_the_header_includes() {
    for m in [
        "INT32_MAX",
        "INT64_MIN",
        "UINT8_MAX",
        "SIZE_MAX",
        "PTRDIFF_MAX",
        "INTMAX_MAX",
    ] {
        let e = err(&format!("export fn f({m}: f64) -> f64 {{ return {m} }}"));
        assert!(
            e.contains("stdint.h") || e.contains("macro"),
            "`{m}` is expanded by the preprocessor inside the generated header, so it must be \
             refused with a reason that says so: {e}"
        );
    }
    // A plain uppercase name is not a macro and must stay legal — the rule is the standard
    // header's macro set, not "shouting names are banned".
    ok("export fn f(MAX_QTY: f64) -> f64 { return MAX_QTY }");
    // `int32_t` is a typedef, not a macro: `double mlx_f(double int32_t);` was measured to
    // compile as C and as C++, so it is not a defect and is not reserved.
    ok("export fn f(int32_t: f64) -> f64 { return int32_t }");
    // And the scope split holds here too — an internal parameter never reaches the header,
    // so the header's macros are not its problem (Grok verify: asserted, not inferred).
    ok("fn g(INT32_MAX: f64) -> f64 { return INT32_MAX }\n\
        export fn f(x: f64) -> f64 { return g(x) }");
}

/// `generated_prefix` matched `ml_`/`mlx_` with `starts_with`, which is case-SENSITIVE — in a
/// file two lines from the comment recording that Pascal is not.
///
/// Measured: `export fn f(ML_BUF: string) -> string! { return ML_BUF + "!" }` built with exit
/// 0 and emitted
///
///     function mlx_f(ML_BUF: PAnsiChar; ml_buf: PByte; ml_cap: Integer; ...)
///
/// into the `.pas` — one Pascal identifier, twice in one parameter list. `ml_Cap` WAS refused,
/// because it happens to start with a lowercase `ml_`.
#[test]
fn the_generated_prefixes_are_reserved_the_way_delphi_reads_them() {
    for name in ["ML_BUF", "ML_cap", "MLX_f", "Ml_needed"] {
        let e = err(&format!(
            "export fn f({name}: string) -> string! {{ return {name} + \"!\" }}"
        ));
        assert!(
            e.contains("compiler generates"),
            "`{name}` is the compiler's own name as Delphi reads it: {e}"
        );
    }
    // The D17 out-param has the same problem and its own message.
    let e = err("error E = 1\n\
         export fn f(OUT_VALUE: f64) -> f64! { if OUT_VALUE < 0.0 { fail E }  return OUT_VALUE }");
    assert!(e.contains("out_value"), "{e}");
}

/// `_` is Rust's reserved identifier and was missing from the keyword list, so it passed the
/// frontend and failed inside the generated crate — the exact leak `reserved.rs` exists to
/// prevent, and its module doc says so.
///
/// Measured: `export fn f(_: f64) -> f64 { return _ }` reached
/// `mlc: codegen error: cargo build of generated crate failed`.
#[test]
fn the_rust_reserved_identifier_is_reserved() {
    let e = err("export fn f(_: f64) -> f64 { return _ }");
    assert!(
        e.contains("Rust") && !e.contains("cargo build"),
        "`_` must be refused in the frontend, not by rustc inside generated code: {e}"
    );
    let local = err("export fn f(x: f64) -> f64 { let _ = x  return x }");
    assert!(local.contains("Rust"), "a local named `_` too: {local}");
}

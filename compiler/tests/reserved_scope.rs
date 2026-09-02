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

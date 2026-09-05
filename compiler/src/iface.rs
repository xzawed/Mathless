//! Interface fingerprint (`SPEC-iface-hash.md`) — what a host is entitled to assume.
//!
//! A module's exported symbol names carry no type information, so a host built against an
//! older interface resolves every symbol of a newer one and then calls it wrongly. That was
//! measured, twice, with a real MSVC host and two generated DLLs: a reordered parameter
//! pair returned `0` where the truth was `33` (no crash, no error), and a parameter that
//! changed from `i32` to `string` turned an integer argument into a dereferenced pointer
//! (`0xC0000005`). `ml_module_abi_version` reported `1` through both, because it is a
//! constant that does not depend on any signature.
//!
//! This module computes the value that makes that visible: a fingerprint over the module's
//! **host-visible contract**, exported as `ml_iface_hash()` and pinned into the generated
//! header so a host can refuse a module it was not built for.
//!
//! **It is a contract fingerprint, not an ABI one (DP-H1).** Parameter *names* are part of
//! it, because the worse of the two measured failures is invisible at the ABI level —
//! `mlx_boxes(int32_t items, int32_t per)` and `mlx_boxes(int32_t per, int32_t items)` are
//! the same C declaration. The price is that a pure rename is a false rejection; that is a
//! decision on record (SPEC §5.3), not an oversight.
//!
//! **It is not integrity (P1).** An attacker who edits the module edits this function too.
//! The threat here is an ordinary mistake: replacing the module without rebuilding the host.

use crate::ir::IrModule;

/// FNV-1a, 64-bit. Ten lines, deterministic, and **no third-party dependency** — the
/// workspace has zero of those and this slice does not spend that.
///
/// Not a cryptographic hash, and it does not need to be (SPEC §5.4).
pub fn fnv1a64(bytes: &[u8]) -> u64 {
    const OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x0000_0100_0000_01b3;
    let mut h = OFFSET_BASIS;
    for b in bytes {
        h ^= *b as u64;
        h = h.wrapping_mul(PRIME);
    }
    h
}

/// The exact bytes the fingerprint is computed over (`ml-iface/1`, SPEC §2.1).
///
/// Reproducibility is the whole contract here, so the shape is fixed and tested:
///
/// ```text
/// ml-iface/1
/// abi=<ML_MODULE_ABI_VERSION>
/// fn <name>(<param>[, <param>]*) -> <ret>      … exported functions, sorted by name
/// err <NAME>=<code>                            … declared errors, sorted by name
/// ```
///
/// Two rules earn their place by what they *exclude*:
///
/// - **Top-level lists are sorted** (DP-H4). Declaration order reaches no host — hosts
///   resolve by name — so an edit that only moves a function must not reject anything.
/// - **Internal functions never appear.** Since #101 they are not exported, so they are not
///   part of what a host may assume. Neither are function bodies: a threshold change has to
///   keep the fingerprint or this slice breaks `ARCHITECTURE.md:68`.
pub fn manifest(module: &IrModule) -> String {
    let mut s = String::from("ml-iface/1\n");
    s.push_str(&format!("abi={}\n", crate::abi::ML_MODULE_ABI_VERSION));

    // Sorted BY NAME, which is what SPEC §2.1 / DP-H4 says and what the doc comment above
    // says — so sort on the name, not on the rendered line. The two are not the same order:
    // sorting the line compares the separator that follows the name against the next
    // character of a longer name, and for `err` lines that separator is `=` (0x3D), which
    // every digit sorts below. `error E_1` + `error E_10` came out as `err E_10` then
    // `err E_1` (measured).
    //
    // `fn` lines happened to be right, because their separator is `(` (0x28) and nothing an
    // identifier can contain sorts below it. That is a property of the separator, not of the
    // approach, so both lists now carry their key explicitly rather than relying on it.
    let mut fns: Vec<(&str, String)> = module
        .functions
        .iter()
        .filter(|f| f.exported)
        .map(|f| {
            let params = f
                .params
                .iter()
                .map(|p| {
                    // `out` is spelled, because #80 measured what happens when an out is
                    // silently taken by value: it compiles and does nothing.
                    let out = if p.out { "out " } else { "" };
                    format!("{}{}:{}", out, p.name, p.ty)
                })
                .collect::<Vec<_>>()
                .join(", ");
            // `!` is part of the return type: a fallible function's C declaration is a
            // status plus an out-param, which a host cannot read as a plain value.
            let bang = if f.fallible { "!" } else { "" };
            (
                f.name.as_str(),
                format!("fn {}({}) -> {}{}\n", f.name, params, f.ret, bang),
            )
        })
        .collect();
    fns.sort_by(|a, b| a.0.as_bytes().cmp(b.0.as_bytes()));
    for (_, line) in fns {
        s.push_str(&line);
    }

    // Error codes are compiled INTO the host as `ML_ERR_*` (header.rs), so renumbering one
    // while the host keeps the old value is the same silent misreading as a swapped
    // parameter (DP-H6).
    let mut errs: Vec<(&str, String)> = module
        .errors
        .iter()
        .map(|e| (e.name.as_str(), format!("err {}={}\n", e.name, e.code)))
        .collect();
    errs.sort_by(|a, b| a.0.as_bytes().cmp(b.0.as_bytes()));
    for (_, line) in errs {
        s.push_str(&line);
    }

    s
}

/// The module's interface fingerprint — the value exported as `ml_iface_hash()`.
pub fn fingerprint(module: &IrModule) -> u64 {
    fnv1a64(manifest(module).as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compile_to_ir;

    fn ir(src: &str) -> IrModule {
        compile_to_ir(src).expect("compile")
    }

    #[test]
    fn a_module_with_no_parameters_renders_empty_parens() {
        assert_eq!(
            manifest(&ir("export fn tick() -> i32 { return 1 }")),
            "ml-iface/1\nabi=1\nfn tick() -> i32\n"
        );
    }

    #[test]
    fn functions_are_sorted_not_left_in_declaration_order() {
        let m = manifest(&ir(
            "export fn zeta(x: f64) -> f64 { return x }\nexport fn alpha(x: f64) -> f64 { return x }",
        ));
        let alpha = m.find("fn alpha").expect("alpha present");
        let zeta = m.find("fn zeta").expect("zeta present");
        assert!(alpha < zeta, "manifest must be sorted by name:\n{m}");
    }

    #[test]
    fn a_fallible_return_is_marked() {
        let m = manifest(&ir(
            "error E = 1\nexport fn f(x: f64) -> f64! { if x < 0.0 { fail E } return x }",
        ));
        assert!(m.contains("-> f64!\n"), "{m}");
        assert!(m.contains("err E=1\n"), "{m}");
    }

    #[test]
    fn the_fingerprint_is_the_hash_of_the_manifest() {
        let module = ir("export fn f(x: f64) -> f64 { return x }");
        assert_eq!(fingerprint(&module), fnv1a64(manifest(&module).as_bytes()));
    }
}

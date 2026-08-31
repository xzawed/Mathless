//! Reserved words across all current codegen targets (Rust, C, Pascal/Delphi).
//!
//! A Mathless parameter or local name is emitted **raw** into every backend (Rust codegen,
//! C header, Delphi unit), so a name that collides with a reserved word in ANY target would
//! produce invalid output. The frontend rejects such names with a clear error — a single
//! check that protects all backends (WBS hardening).
//!
//! **Exported** function names are safe without this check: they are emitted with the `mlx_`
//! prefix. **Internal** function names are not — since SPEC-calls they are emitted as-is —
//! so `typeck` runs them through here too.
//!
//! Target keywords are only half the problem. codegen also injects its OWN identifiers into
//! the same emitted scope, and those need reserving as well — see [`generated_prefix`].

/// Return the target languages that reserve `name` (empty if the name is safe everywhere).
/// Rust and C are case-sensitive; Pascal is case-insensitive.
pub fn reserving_targets(name: &str) -> Vec<&'static str> {
    let mut targets = Vec::new();
    if RUST.contains(&name) {
        targets.push("Rust");
    }
    if C.contains(&name) {
        targets.push("C");
    }
    if PASCAL.iter().any(|k| k.eq_ignore_ascii_case(name)) {
        targets.push("Pascal");
    }
    targets
}

/// Prefixes that codegen owns. A user name carrying one of these lands in the SAME emitted
/// scope as an identifier the compiler generated, and shadowing is silent in Rust.
///
/// This is not hypothetical. Both were reachable from ordinary source before this check:
///
/// - `__d` — the divisor temporary in the `i32 /` and `%` guard. A parameter named `__d`
///   was shadowed by it, so `__d / b` returned `1` for every nonzero `b`. It compiled, the
///   number looked plausible, and it was wrong.
/// - `ml_floor` — a parameter by that name shadowed the emitted rounding helper, so
///   `floor(x)` lowered to `ml_floor(ml_floor)` and rustc reported E0618 in a file the user
///   never wrote.
///
/// There is no codegen-side escape: Mathless and Rust share an identifier character set, so
/// no generated name is unspellable. Reserving the prefix is the fix.
///
/// `out_value` is handled separately in `typeck` because it is only generated for a fallible
/// function, and its message can say so.
pub fn generated_prefix(name: &str) -> Option<&'static str> {
    // Order matters for the message only: `mlx_` is checked first so a name carrying it is
    // not reported as merely `ml_`.
    ["mlx_", "ml_", "__"]
        .into_iter()
        .find(|p| name.starts_with(p))
}

/// What the compiler generates behind each reserved prefix — used to explain the rejection
/// instead of just stating it.
pub fn generated_prefix_reason(prefix: &str) -> &'static str {
    match prefix {
        "mlx_" => "exported functions are emitted as `mlx_<name>` (D18)",
        "ml_" => {
            "the runtime namespace: `ml_module_abi_version`, the panic handler, and the \
                  rounding helpers"
        }
        _ => "compiler temporaries such as the `__d` divisor binding in the `i32 /` guard",
    }
}

/// Rust 2021 keywords (strict + reserved).
static RUST: &[&str] = &[
    "as", "break", "const", "continue", "crate", "dyn", "else", "enum", "extern", "false", "fn",
    "for", "if", "impl", "in", "let", "loop", "match", "mod", "move", "mut", "pub", "ref",
    "return", "self", "Self", "static", "struct", "super", "trait", "true", "type", "unsafe",
    "use", "where", "while", "async", "await", // strict
    "abstract", "become", "box", "do", "final", "macro", "override", "priv", "typeof", "unsized",
    "virtual", "yield", "try", "union", "gen", // reserved
];

/// C11 keywords (plus `<stdbool.h>` macros used in generated headers).
static C: &[&str] = &[
    "auto",
    "break",
    "case",
    "char",
    "const",
    "continue",
    "default",
    "do",
    "double",
    "else",
    "enum",
    "extern",
    "float",
    "for",
    "goto",
    "if",
    "inline",
    "int",
    "long",
    "register",
    "restrict",
    "return",
    "short",
    "signed",
    "sizeof",
    "static",
    "struct",
    "switch",
    "typedef",
    "union",
    "unsigned",
    "void",
    "volatile",
    "while",
    "_Bool",
    "_Complex",
    "_Imaginary",
    "_Alignas",
    "_Alignof",
    "_Atomic",
    "_Generic",
    "_Noreturn",
    "_Static_assert",
    "_Thread_local",
    "bool",
    "true",
    "false",
];

/// Delphi/Object Pascal reserved words (lowercase; compared case-insensitively).
static PASCAL: &[&str] = &[
    "and",
    "array",
    "as",
    "asm",
    // `at` and `on` are reserved in Delphi too (exception handling: `on E: … do`,
    // `raise … at …`). They were missing, so a parameter, local or module named `on`
    // produced a `unit on;` that Delphi would reject. Evidence level E1 — Embarcadero's
    // documented reserved-word list, not something we can compile here (dcc64 absent,
    // Delphi arm of gate D still open).
    "at",
    "on",
    "begin",
    "case",
    "class",
    "const",
    "constructor",
    "destructor",
    "dispinterface",
    "div",
    "do",
    "downto",
    "else",
    "end",
    "except",
    "exports",
    "file",
    "finalization",
    "finally",
    "for",
    "function",
    "goto",
    "if",
    "implementation",
    "in",
    "inherited",
    "initialization",
    "inline",
    "interface",
    "is",
    "label",
    "library",
    "mod",
    "nil",
    "not",
    "object",
    "of",
    "or",
    "out",
    "packed",
    "procedure",
    "program",
    "property",
    "raise",
    "record",
    "repeat",
    "resourcestring",
    "set",
    "shl",
    "shr",
    "string",
    "then",
    "threadvar",
    "to",
    "try",
    "type",
    "unit",
    "until",
    "uses",
    "var",
    "while",
    "with",
    "xor",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_the_right_targets() {
        assert_eq!(reserving_targets("price"), Vec::<&str>::new());
        assert_eq!(reserving_targets("type"), vec!["Rust", "Pascal"]);
        assert_eq!(reserving_targets("int"), vec!["C"]);
        assert_eq!(reserving_targets("Begin"), vec!["Pascal"]); // case-insensitive
        assert_eq!(reserving_targets("fn"), vec!["Rust"]);
    }
}

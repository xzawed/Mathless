//! Reserved words across all current codegen targets (Rust, C, Pascal/Delphi).
//!
//! A Mathless parameter (and, later, local) name is emitted **raw** into every backend
//! (Rust codegen, C header, Delphi unit), so a name that collides with a reserved word in
//! ANY target would produce invalid output. The frontend rejects such names with a clear
//! error — a single check that protects all backends (WBS hardening). Function names are
//! safe because they are emitted with the `mlx_` prefix.

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

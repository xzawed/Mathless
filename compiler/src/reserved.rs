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

/// Where a name can actually end up, which is what decides the languages it must avoid
/// (STATUS §4-2, decided 2026-09-02 on the measurement below).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NameScope {
    /// The name is written verbatim into the generated bindings: the module name (crate name,
    /// header guard, unit name) and an **exported** function's parameters.
    ///
    /// Measured: `export fn price_of(quantity: i32, discount_rate: f64)` emits
    /// `function mlx_price_of(quantity: Integer; discount_rate: Double)` into the `.pas`.
    Bindings,
    /// The name never leaves the generated module: an **internal** function's parameters, and
    /// every local.
    ///
    /// Measured: an internal parameter and a local appeared **zero** times in the generated
    /// `.h` and `.pas`. So Pascal cannot be reached from here — and cannot become reachable,
    /// because Delphi is a *binding* target, never a codegen target. C is still checked:
    /// D19 keeps the C-emit backend slot open, and that backend would put this name in C.
    GeneratedModule,
}

/// Return the target languages that reserve `name` (empty if it is safe in `scope`).
/// Rust and C are case-sensitive; Pascal is case-insensitive.
///
/// `Bindings` checks more than `GeneratedModule` because the header is read by more than one
/// compiler. It declares `#ifdef __cplusplus extern "C" {`, its own preamble says it compiles
/// as C++, and `hosts/rust-oracle/tests/c_host.rs` runs `cl /TP` over every example's header —
/// so a C++-only keyword breaks it as surely as a C one. Measured: a parameter named `new`
/// built with exit 0 and emitted `double mlx_f(double new);`, which `cl /TP` rejects with
/// C2143. `class` was already refused, but only because it is *also* a Pascal word.
pub fn reserving_targets_in(name: &str, scope: NameScope) -> Vec<&'static str> {
    let mut targets = Vec::new();
    if RUST.contains(&name) {
        targets.push("Rust");
    }
    if C.contains(&name) {
        targets.push("C");
    }
    if scope == NameScope::Bindings {
        if CPP.contains(&name) {
            targets.push("C++");
        }
        if PASCAL.iter().any(|k| k.eq_ignore_ascii_case(name)) {
            targets.push("Pascal");
        }
    }
    targets
}

/// A macro the generated header pulls in, which would be *substituted* rather than shadowed.
///
/// A reserved word and a macro fail differently, and the difference is why this is not just
/// more entries in [`C`]. A keyword collision is a name the compiler refuses; a macro
/// collision is text the preprocessor replaces before the compiler ever sees the declaration.
/// The header emits `#include <stdint.h>` itself, so these are already defined by the time
/// its declarations are read.
///
/// Measured: `export fn f(INT32_MAX: f64)` built with exit 0 and emitted
/// `double mlx_f(double INT32_MAX);`, which the preprocessor turns into
/// `double mlx_f(double 2147483647);` — rejected by `cl` as C **and** as C++ (C2143, C2059).
///
/// Only names this project's own headers actually bring in are listed, so the rule stays
/// "the macros we include" and not "uppercase names are banned": `MAX_QTY` is still a legal
/// parameter name, and a test pins that.
pub fn included_macro(name: &str) -> Option<&'static str> {
    STDINT_MACROS.contains(&name).then_some("stdint.h")
}

/// The prefixes codegen owns, matched the way the target reads them.
///
/// [`generated_prefix`] used `starts_with`, which is case-SENSITIVE — two lines below a
/// comment recording that Pascal is not. Measured: `export fn f(ML_BUF: string) -> string!`
/// built with exit 0 and emitted
///
/// ```text
/// function mlx_f(ML_BUF: PAnsiChar; ml_buf: PByte; ml_cap: Integer; out ml_needed: Integer)
/// ```
///
/// into the `.pas` — one Pascal identifier declared twice in one parameter list. `ml_Cap` was
/// refused all along, purely because it happens to start with a lowercase `ml_`.
///
/// `GeneratedModule` stays case-sensitive on purpose: that name lands in Rust (or, under the
/// D19 C-emit slot, in C), and in both `ML_BUF` and `ml_buf` are two different identifiers.
/// Rejecting it there would take away a legal name for no collision.
pub fn generated_prefix_in(name: &str, scope: NameScope) -> Option<&'static str> {
    // Order matters for the message only: `mlx_` is checked first so a name carrying it is
    // not reported as merely `ml_`.
    ["mlx_", "ml_", "__"].into_iter().find(|p| match scope {
        NameScope::Bindings => name
            .get(..p.len())
            .is_some_and(|head| head.eq_ignore_ascii_case(p)),
        NameScope::GeneratedModule => name.starts_with(p),
    })
}

/// Every target, for a name that reaches the bindings.
pub fn reserving_targets(name: &str) -> Vec<&'static str> {
    reserving_targets_in(name, NameScope::Bindings)
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
    generated_prefix_in(name, NameScope::GeneratedModule)
}

/// What the compiler generates behind each reserved prefix — used to explain the rejection
/// instead of just stating it.
pub fn generated_prefix_reason(prefix: &str) -> &'static str {
    match prefix {
        "mlx_" => "exported functions are emitted as `mlx_<name>` (D18)",
        "ml_" => {
            "the runtime namespace: `ml_module_abi_version`, `ml_iface_hash`, the panic \
                  handler, and the rounding helpers"
        }
        _ => "compiler temporaries such as the `__d` divisor binding in the `i32 /` guard",
    }
}

/// Rust 2021 keywords (strict + reserved).
static RUST: &[&str] = &[
    // `_` is Rust's reserved identifier, and leaving it out was the exact leak this module
    // exists to prevent: `export fn f(_: f64) -> f64 { return _ }` passed the frontend and
    // failed inside the generated crate, where the positions mean nothing to the user.
    "_", "as", "break", "const", "continue", "crate", "dyn", "else", "enum", "extern", "false",
    "fn", "for", "if", "impl", "in", "let", "loop", "match", "mod", "move", "mut", "pub", "ref",
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

/// C++ keywords that are NOT already in [`C`]. Checked for `Bindings` only — the generated
/// header is the C++ artifact; the generated Rust is not, and the D19 C-emit backend slot
/// would produce C.
///
/// C++23's list, minus the ones C already reserves, minus the alternative tokens (`and`,
/// `or`, `not`, `xor`, `bitand`, …) — those are kept, because they are also macros in C's
/// `<iso646.h>` and reserving them costs a user two ordinary-looking words for a header this
/// project does not include. `and`/`or`/`not`/`xor` are already refused as Pascal words in
/// this scope anyway.
static CPP: &[&str] = &[
    "alignas",
    "alignof",
    "asm",
    "catch",
    "char8_t",
    "char16_t",
    "char32_t",
    "class",
    "concept",
    "consteval",
    "constexpr",
    "constinit",
    "const_cast",
    "co_await",
    "co_return",
    "co_yield",
    "decltype",
    "delete",
    "dynamic_cast",
    "explicit",
    "export",
    "friend",
    "mutable",
    "namespace",
    "new",
    "noexcept",
    "nullptr",
    "operator",
    "private",
    "protected",
    "public",
    "reinterpret_cast",
    "requires",
    "static_assert",
    "static_cast",
    "template",
    "this",
    "thread_local",
    "throw",
    "try",
    "typeid",
    "typename",
    "using",
    "virtual",
    "wchar_t",
];

/// Object-like macros from `<stdint.h>`, which every generated header includes.
///
/// The function-like ones (`INT32_C` and friends) are here too. They only expand when
/// followed by `(`, so `double mlx_f(double INT32_C)` is harmless today — but the cost of
/// listing them is one line each, and the cost of leaving them out is that the rule stops
/// being "the macros this header brings in" and becomes a judgement about which of them are
/// currently reachable.
///
/// `<stdbool.h>`'s macros are already covered: `bool`/`true`/`false` are in [`C`], and
/// `__bool_true_false_are_defined` starts with a reserved prefix.
static STDINT_MACROS: &[&str] = &[
    "INT8_MIN",
    "INT16_MIN",
    "INT32_MIN",
    "INT64_MIN",
    "INT8_MAX",
    "INT16_MAX",
    "INT32_MAX",
    "INT64_MAX",
    "UINT8_MAX",
    "UINT16_MAX",
    "UINT32_MAX",
    "UINT64_MAX",
    "INT_LEAST8_MIN",
    "INT_LEAST16_MIN",
    "INT_LEAST32_MIN",
    "INT_LEAST64_MIN",
    "INT_LEAST8_MAX",
    "INT_LEAST16_MAX",
    "INT_LEAST32_MAX",
    "INT_LEAST64_MAX",
    "UINT_LEAST8_MAX",
    "UINT_LEAST16_MAX",
    "UINT_LEAST32_MAX",
    "UINT_LEAST64_MAX",
    "INT_FAST8_MIN",
    "INT_FAST16_MIN",
    "INT_FAST32_MIN",
    "INT_FAST64_MIN",
    "INT_FAST8_MAX",
    "INT_FAST16_MAX",
    "INT_FAST32_MAX",
    "INT_FAST64_MAX",
    "UINT_FAST8_MAX",
    "UINT_FAST16_MAX",
    "UINT_FAST32_MAX",
    "UINT_FAST64_MAX",
    "INTPTR_MIN",
    "INTPTR_MAX",
    "UINTPTR_MAX",
    "INTMAX_MIN",
    "INTMAX_MAX",
    "UINTMAX_MAX",
    "PTRDIFF_MIN",
    "PTRDIFF_MAX",
    "SIG_ATOMIC_MIN",
    "SIG_ATOMIC_MAX",
    "SIZE_MAX",
    "WCHAR_MIN",
    "WCHAR_MAX",
    "WINT_MIN",
    "WINT_MAX",
    "INT8_C",
    "INT16_C",
    "INT32_C",
    "INT64_C",
    "UINT8_C",
    "UINT16_C",
    "UINT32_C",
    "UINT64_C",
    "INTMAX_C",
    "UINTMAX_C",
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

# Mathless

> Compile typed logic into a native module. A host loads it over a plain **C ABI** — no source
> shipped, no host rebuild.

[![CI](https://github.com/xzawed/Mathless/actions/workflows/ci.yml/badge.svg)](https://github.com/xzawed/Mathless/actions/workflows/ci.yml)
![Rust](https://img.shields.io/badge/rust-1.97.1-orange?logo=rust&logoColor=white)
![Status](https://img.shields.io/badge/status-Phase%201%20complete%20except%20Delphi-blue)
![Target](https://img.shields.io/badge/target-Windows%20x64-informational)
![License](https://img.shields.io/badge/license-Apache--2.0%20OR%20MIT-blue)

**Mathless** is a statically-typed extension language for putting logic into an application you
cannot — or would rather not — recompile. You write the logic in a small surface syntax; the
compiler turns it, **at compile time**, into a native module (`.dll` / `.so`) that the application
loads over a **C ABI**. What you ship is the binary, not the source.

It is aimed at people maintaining a native host — Delphi, C, C++, C# — who need customer- or
domain-specific rules to be typed, fast, and not casually readable in the field.

한국어 → **[README.ko.md](README.ko.md)**

## How it works

```
.mls  →  parse / typecheck  →  typed IR  →  native codegen  →  module (.dll/.so)  →  [ C ABI ]  →  host
```

All of that happens at compile time. There is no runtime interpreter and no bytecode VM: what the
host loads is native code behind a C ABI.

## Why

Delphi and Object Pascal give strong static types and native speed, but sit in a closed ecosystem
with a dated extension story. Scripting languages are flexible, yet weak on typing — and on
protecting what you distribute. Mathless takes the typed, native side and makes it loadable.

Four commitments hold throughout, and everything below is measured against them:

- **Native only** — no default VM or bytecode runtime.
- **No source shipped** — distribution is a native shared library.
- **The C ABI is the one first-class boundary** — per-language bindings are thin wrappers on top of
  it, which is why this is not a Delphi-only tool.
- **Protection is reported as cost, never as impossibility** — exported-symbol count, stripped
  binary size, absence of source in the artifact. Never framed as "reversing difficulty".

## Status

Phase 1's acceptance is complete **on the C side**. Measured on `main` (Windows x64, pinned
toolchain):

- **Compiler `mlc`** — lex → parse → typecheck → backend-independent IR → codegen (IR → `no_std`,
  `extern "C"` Rust → a `cargo` cdylib).
- **Language today** — `f64`, `bool` and `i32`, with arithmetic (`+ - *`, and `/` on `f64` only),
  comparisons, and explicit `as` conversion between the two numeric types; `if` (no `else` yet),
  `while` and `return`; `let` and `let mut` with assignment; unary `-` and `!`; `&&` and `||`;
  fallible functions — `-> T!` with `error NAME = N` and `fail NAME`, lowered to an integer status
  plus an out-parameter; and internal `fn` declarations with calls, where recursion is rejected at
  compile time. [docs/LANGUAGE.md](docs/LANGUAGE.md) is the definitive list.
- **CLI** — `mlc build <file.mls> -o <dir>` produces `<name>.dll`, `<name>.h` (C header) and
  `<name>.pas` (Delphi import unit).
- **Two measured host paths** — a Rust `kernel32` oracle, and a real C host built with MSVC that
  compiles the generated header and calls the module through `LoadLibrary` / `GetProcAddress`.

Acceptance **A** (it compiles) · **B** (the oracle loads and calls it) · **C** (export and size
proxies) · **D** (a real C host loads the same module) all pass. The stripped `no_std` module is
about 9.7 KB and exports exactly the two intended symbols — cross-checked against
`dumpbin /exports`, so the count does not rest on our own PE reader alone.

**Delphi is not verified.** D covers the C arm only: there is no `dcc64` on the build machine, so
nothing has ever compiled the generated `.pas` and it carries a DRAFT note. D14 makes Delphi the
flagship host, which leaves half the host story unproven.

Current numbers, open decisions and the next slice live in [docs/STATUS.md](docs/STATUS.md).

## Example

```text
export fn discount(price: f64, vip: bool) -> f64 {
  let rate = 0.9
  if vip { return price * rate }
  return price
}
```

```sh
mlc build discount.mls -o out/
#  out/discount.dll   native module — exports mlx_discount + ml_module_abi_version
#  out/discount.h     C header
#  out/discount.pas   Delphi import unit
```

A C host loads `discount.dll` over the C ABI and calls `mlx_discount(100.0, true) == 90.0`, with no
rebuild of the host itself.

## Honest limits

- **Windows x64** (`.dll`) for now. A Linux `.so` target is a later, explicit decision, not an
  oversight.
- The surface is deliberately small, and no bigger than the summary above says. For the exact,
  maintained list of what compiles — and what does not yet — see
  [docs/LANGUAGE.md](docs/LANGUAGE.md).
- **A call into a module may not return.** A `while` loop can run forever, and nothing across the C
  ABI can time it out or cancel it. Modules are trusted code; call them on a worker thread if the
  host has to stay responsive.
- File extensions (`.mls`, `.mll`) and the C API names are **provisional**.

## Documentation

| Document | What it covers |
|----------|----------------|
| [docs/STATUS.md](docs/STATUS.md) | Measured state of `main`, open decisions, and the next slice |
| [docs/VISION.md](docs/VISION.md) | Why Mathless exists, and what success looks like |
| [docs/LANGUAGE.md](docs/LANGUAGE.md) | Surface syntax vs the internal model |
| [docs/HOST_ABI.md](docs/HOST_ABI.md) | The C ABI, host integration, and the liveness contract |
| [docs/SECURITY.md](docs/SECURITY.md) | Protection goals, stages, and the measured proxies |
| [docs/ROADMAP.md](docs/ROADMAP.md) | MVP → expansion |
| [docs/DECISIONS.md](docs/DECISIONS.md) | Confirmed decisions (D01–D22) and rejected alternatives |
| [docs/slices/](docs/slices/README.md) | Per-feature SPECs — the unit of work — and their status |

## Contributing

PR-first: no direct commits to `main`. Every change is test-driven, and CI runs `fmt`, `clippy` and
the test suite on two jobs — `windows-latest`, where the acceptance tests actually execute, and
`ubuntu-latest`, which compiles the platform-independent frontend as insurance. See
[CONTRIBUTING.md](CONTRIBUTING.md).

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option — the Rust ecosystem's conventional pair. MIT keeps things simple and
GPLv2-compatible; Apache-2.0 adds an explicit patent grant.

Unless you state otherwise, any contribution you intentionally submit for inclusion in this work, as
defined in the Apache-2.0 license, shall be dual licensed as above, without additional terms.

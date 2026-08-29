# Mathless

> Compile typed logic into a native module. A host loads it over a plain **C ABI** —
> no source shipped, no host rebuild.

[![CI](https://github.com/xzawed/Mathless/actions/workflows/ci.yml/badge.svg)](https://github.com/xzawed/Mathless/actions/workflows/ci.yml)
![Rust](https://img.shields.io/badge/rust-1.97.1-orange?logo=rust&logoColor=white)
![Status](https://img.shields.io/badge/status-Phase%201%20complete%20except%20Delphi-blue)
![Target](https://img.shields.io/badge/target-Windows%20x64-informational)
![License](https://img.shields.io/badge/license-Apache--2.0%20OR%20MIT-blue)

You maintain a native application, and some of its rules keep moving. Pricing, discounts,
eligibility, the things each customer wants slightly differently.

Recompiling the whole host for every change is not practical. Shipping those rules as readable
source is not acceptable either.

**Mathless** is a small statically-typed language for that gap. You write the rules in its
surface syntax, and the compiler turns them into a native module at compile time. Your
application loads that module over a plain **C ABI**. What you ship is the binary.

It is built for native hosts: Delphi, C, C++, C#.

한국어 → **[README.ko.md](README.ko.md)**

## How it works

```
.mls  →  parse / typecheck  →  typed IR  →  native codegen  →  module (.dll/.so)  →  [ C ABI ]  →  host
```

All of that happens at compile time. Nothing interprets anything at runtime, and there is no
bytecode VM. The host loads native code and calls it like any other library.

## Why

Delphi and Object Pascal give you strong static types and native speed. They also sit in a closed
ecosystem with a dated story for extensions and tooling.

Scripting languages solve the flexibility problem, but they are weak on typing. They are weaker
still on protecting what you hand to a customer.

Mathless takes the typed, native side and makes it loadable. Four commitments hold throughout,
and the rest of this file is measured against them.

- **Native only.** No default VM, no bytecode runtime.
- **No source shipped.** What you distribute is a native shared library.
- **The C ABI is the one first-class boundary.** Per-language bindings are thin wrappers on top
  of it, which is why this is not a Delphi-only tool.
- **Protection is reported as cost, never as impossibility.** We measure the exported-symbol
  count, the stripped binary size, and the absence of source in the artifact. We never translate
  those into "reversing difficulty".

## Status

Phase 1's acceptance is complete on the C side. Everything below is measured on `main`, on
Windows x64 with a pinned toolchain.

**The compiler.** `mlc` runs lex, parse, typecheck, a backend-independent IR, then codegen. The
backend emits `no_std`, `extern "C"` Rust and builds it as a `cargo` cdylib.

**The language today.** Three types: `f64`, `bool` and `i32`. They come with arithmetic (`+`,
`-`, `*`, and `/` on `f64` only), comparisons, and an explicit `as` conversion between the two
numeric types. Control flow is `if`, `while` and `return`; there is no `else` yet. Locals
are `let` and `let mut`, with assignment. Operators include unary `-` and `!`, plus `&&` and
`||`. A function can be fallible: `-> T!` with `error NAME = N` and `fail NAME`, which lowers to
an integer status and an out-parameter. Internal `fn` declarations can call each other, but
recursion is rejected at compile time. [docs/LANGUAGE.md](docs/LANGUAGE.md) keeps the definitive
list.

**The CLI.** `mlc build <file.mls> -o <dir>` writes three files side by side: the `.dll` module,
a `.h` C header, and a `.pas` Delphi import unit.

**Two host paths, both measured.** A Rust `kernel32` oracle loads the module and calls it. So
does a real C host built with MSVC, which compiles the generated header and resolves the exports
through `LoadLibrary` and `GetProcAddress`.

Acceptance A, B, C and D all pass. It compiles, the oracle calls it, the export and size proxies
hold, and a real C host loads the same module. The stripped `no_std` build is about 9.7 KB and
exports exactly the two symbols it should. We cross-check that count against `dumpbin /exports`,
so it does not rest on our own PE reader alone.

**Delphi is not verified.** Acceptance D covers the C arm only. There is no `dcc64` on the build
machine, so nothing has ever compiled the generated `.pas`, and it still carries a DRAFT note.
D14 makes Delphi the flagship host, which leaves half the host story unproven.

For current numbers, open decisions and the next piece of work, see
[docs/STATUS.md](docs/STATUS.md).

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

A C host loads `discount.dll`, calls `mlx_discount(100.0, true)`, and gets `90.0` back. The host
itself is never rebuilt.

## Honest limits

- **Windows x64 for now.** A Linux `.so` target is a decision we have not taken yet, rather than
  an oversight.
- **The surface is small on purpose.** [docs/LANGUAGE.md](docs/LANGUAGE.md) lists exactly what
  compiles today, and what does not.
- **A call into a module may not return.** A `while` loop can run forever, and nothing across the
  C ABI can time it out or cancel it. Treat modules as trusted code, and call them on a worker
  thread if your host has to stay responsive.
- **Names are provisional.** The `.mls` and `.mll` extensions, and the C API names, may still
  change.

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

Every change arrives as a PR, and nothing is committed to `main` directly. Work is test-driven,
and CI runs `fmt`, `clippy` and the test suite on two jobs.

`windows-latest` is the one that matters, because the acceptance tests actually execute there.
`ubuntu-latest` compiles the platform-independent frontend as insurance. See
[CONTRIBUTING.md](CONTRIBUTING.md) for the rest.

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option. This is the Rust ecosystem's usual pair: MIT keeps things simple and
GPLv2-compatible, and Apache-2.0 adds an explicit patent grant.

Unless you say otherwise, any contribution you intentionally submit for inclusion in this work,
as defined in the Apache-2.0 license, is dual licensed the same way, with no extra terms.

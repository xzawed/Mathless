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

It is built for native hosts: Delphi, C, C++, C#. Today the path that has actually been proven
end to end is C — see Status below.

한국어 → **[README.ko.md](README.ko.md)**

## How it works

```
.mls  →  parse / typecheck  →  typed IR  →  native codegen  →  module (.dll)  →  [ C ABI ]  →  host
```

All of that happens at compile time. Today the module is a Windows `.dll`; a Linux `.so` is a
target we have not taken on yet. Nothing interprets anything at runtime, and there is no
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

**The language today.** Four types: `f64`, `bool`, `i32` and `string`. The numeric ones come
with arithmetic (`+`, `-`, `*`, `/`, and `%` on `i32`), comparisons, and an explicit `as`
conversion between them; `i32` division is total, so `x / 0` is `0` rather than a trap.
A `string` can be a parameter or a `-> string!` return; the operations on it are `==` and
`!=`, which compare bytes. Returning one uses a caller-allocated buffer — the module never
allocates. Control flow is `if`, `while` and `return`; there is no `else` yet. Locals are
`let` and `let mut`, with assignment. Operators include unary `-` and `!`, plus `&&` and
`||`. There are four built-ins — `floor`, `ceil`, `round`, `trunc` — which match C's
`<math.h>` exactly. A function can be fallible: `-> T!` with `error NAME = N` and
`fail NAME`, which lowers to an integer status and an out-parameter, and it can declare
extra `out` parameters to return several values. Internal `fn` declarations can call each
other, but recursion is rejected at compile time.
[docs/LANGUAGE.md](docs/LANGUAGE.md) keeps the definitive list.

**The CLI.** `mlc build <file.mls> -o <dir>` writes four files side by side: the `.dll` module,
a `.h` C header, a `.pas` Delphi import unit, and a `.lib` import library so a C host can
link against the header instead of resolving every symbol at run time.

**Three host paths, all measured.** A Rust `kernel32` oracle loads the module and calls it. So
does a real C host built with MSVC, which compiles the generated header and resolves the exports
through `LoadLibrary` and `GetProcAddress`. And a second C host does it the ordinary way —
`#include` the header, link the packaged `.lib`, call the function, with no run-time lookup at
all. Both C hosts check the module's interface fingerprint first and refuse one that drifted.

Acceptance A, B, C and D all pass. It compiles, the oracle calls it, the export and size proxies
hold, and a real C host loads the same module. The stripped `no_std` build is about 9.0-9.5 KB —
the exact byte count is machine-dependent (measured: 9,728 B here, 9,216 B on GitHub's
`windows-latest`, same pinned rustc; the pin covers rustc, not the MSVC linker) — and it
exports exactly the three symbols it should — `mlx_discount` plus the reserved
`ml_module_abi_version` and `ml_iface_hash`. We cross-check that count against
`dumpbin /exports`, so it does not rest on our own PE reader alone.

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
#  out/discount.dll   native module — exports mlx_discount + ml_module_abi_version + ml_iface_hash
#  out/discount.h     C header
#  out/discount.pas   Delphi import unit
#  out/discount.lib   MSVC import library (link-time binding)
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
| [docs/DECISIONS.md](docs/DECISIONS.md) | Confirmed decisions (D01–D23) and rejected alternatives |
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

**What `mlc` produces from your source is yours** — see
[LICENSE-OUTPUT-EXCEPTION](LICENSE-OUTPUT-EXCEPTION). The generated `.dll`, `.h`, `.pas` and `.lib`
carry no obligation from the licences above, even though a generated header is mostly our
template text (measured: `discount.h` is 39 lines, 1 from your source and 38 from ours — which
is exactly why the exception is written down rather than assumed). It covers **output only**;
the compiler itself stays Apache-2.0 OR MIT. It is not legal advice.

Unless you say otherwise, any contribution you intentionally submit for inclusion in this work,
as defined in the Apache-2.0 license, is dual licensed the same way, with no extra terms.

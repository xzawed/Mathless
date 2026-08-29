# Mathless

> Compile typed logic into a native module. A host loads it over a plain **C ABI** — no source shipped, no host rebuild.

[![CI](https://github.com/xzawed/Mathless/actions/workflows/ci.yml/badge.svg)](https://github.com/xzawed/Mathless/actions/workflows/ci.yml)
![Rust](https://img.shields.io/badge/rust-1.97.1-orange?logo=rust&logoColor=white)
![Phase](https://img.shields.io/badge/phase-1_·_vertical_slice-blue)
![Target](https://img.shields.io/badge/target-Windows_x64-informational)

**Mathless** is a statically-typed extension language. You write typed logic in a small surface
syntax; the compiler turns it — **at compile time** — into a **protected native module**
(`.dll` / `.so`) that an existing application loads over a **C ABI**. What you ship is a native
binary, **not** source.

*For developers who want to add typed, protected logic to a native host — Delphi, C, C++, C# … —
without recompiling that host.*

한국어 → **[README.ko.md](README.ko.md)**

## How it works — compile-time only

```
.mls  →  parse / typecheck  →  typed IR  →  native codegen  →  module (.dll/.so)  →  [ C ABI ]  →  host
```

No runtime interpreter and no bytecode VM: the module is native code behind a C ABI.

## Why

Delphi / Object Pascal gives you strong static types and native performance, but a closed
ecosystem and a dated extension/tooling story. Scripting languages (Python, Lua, JS) are flexible
but weak on typing — and on protecting what you distribute. Mathless goes for both: a familiar
typed surface, a native module underneath, shipped without source and loaded **without recompiling
the host**.

Four things hold throughout:

- **Native only** — no default VM or bytecode runtime.
- **No source shipped** — distribution is a native shared library.
- **The C ABI is the one first-class boundary** — per-language bindings are thin wrappers on top.
- **Protection raises the cost of analysis and tampering** — *not* a claim that reversing is impossible.

## Status — Phase 1 (vertical slice)

Measured on Windows (`cargo test --workspace` = **157 green**; CI runs `windows-latest` — the authority, where acceptance A/B/C/D actually execute — plus `ubuntu-latest` for the frontend; toolchain pinned):

- **Compiler `mlc`** — lex → parse → typecheck → backend-independent IR → codegen (IR → `no_std`
  `extern "C"` Rust → `cargo` cdylib).
- **Language today** — `f64` / `bool` / `i32`, `if` / `return`, **fallible functions** (`-> T!` = integer
  status + out-parameter), **locals** — `let` / `let mut` + assignment, **`while` loops**, **unary `-` / `!`**, **`&&` / `||`**, **internal `fn` + calls**.
- **CLI** — `mlc build <file.mls> -o <dir>` produces `<name>.dll` + `<name>.h` (C header) +
  `<name>.pas` (Delphi import unit).
- **Loaded & called** — a Rust `kernel32` *oracle* loads the compiled module and calls the typed
  function; the stripped `no_std` module exports **only** the intended symbols (~9.7 KB).

Acceptance **A** (compile) · **B** (load-and-call via the oracle) · **C** (export/size protection
proxies) · **D** (a real **C host** loads the same module) all pass — D on Windows x64 with MSVC
`cl`: [`hosts/c-host/host.c`](hosts/c-host/host.c) compiles the generated `.h`, resolves the exports
with `LoadLibrary`/`GetProcAddress`, and checks both the scalar and the error-path calls. The same
run cross-checks our export measurement against `dumpbin /exports`.

**Delphi is still unverified.** D covers the C arm only; there is no `dcc64` here, so nothing has
ever compiled the generated `.pas`, and it keeps its DRAFT note.

## Example

```rust
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

A C host loads `discount.dll` over the C ABI and calls `mlx_discount(100.0, true) == 90.0` — with no
host rebuild.

## Hosts

The first-class boundary is the **C ABI**; per-language bindings are thin wrappers on top of it.
Phase 1 targets **Delphi** (the flagship demo host) and **C** (the ABI reference). It is **not**
Delphi-only.

## Honest limits

- Phase 1 targets **Windows x64** (`.dll`); a Linux / `.so` build is a later target.
- Phase 1 is a vertical slice: a small surface, and two measured host paths — the Rust oracle and a
  C host built with MSVC. **Delphi is not one of them yet** (no `dcc64`), and D14 makes Delphi the
  flagship, so the host story is half-proven.
- Protection is reported only through **proxies** — exported-symbol count, stripped binary size, no
  source in the artifact — never framed as "reversing difficulty."
- File extensions (`.mls`, `.mll`) and the C API names are **provisional**.

## Documentation

| Document | What it covers |
|----------|----------------|
| [docs/VISION.md](docs/VISION.md) | Why Mathless exists, and what success looks like |
| [docs/LANGUAGE.md](docs/LANGUAGE.md) | Surface syntax vs the internal model |
| [docs/HOST_ABI.md](docs/HOST_ABI.md) | The C ABI and host integration |
| [docs/SECURITY.md](docs/SECURITY.md) | Protection goals and stages |
| [docs/ROADMAP.md](docs/ROADMAP.md) | MVP → expansion |
| [docs/DECISIONS.md](docs/DECISIONS.md) | Confirmed decisions (D14–D22) and rejected alternatives |
| [docs/STATUS.md](docs/STATUS.md) | Measured state of `main` and the remaining work (session handoff) |
| [docs/slices/](docs/slices/README.md) | Per-feature SPECs (the SDD unit) and their status |

## Contributing

PR-first — no direct commits to `main`; each change is test-driven and gated by CI (`fmt` +
`clippy` + tests on `windows-latest`). See [CONTRIBUTING.md](CONTRIBUTING.md).

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option. This is the Rust ecosystem's conventional dual license: MIT keeps things simple and
GPLv2-compatible, Apache-2.0 adds an explicit patent grant. Take whichever you prefer.

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in
this work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any
additional terms or conditions.

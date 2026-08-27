# Mathless

> **Status:** design stage (pre-implementation) · Phase 0 decisions locked (D14–D18)
> **Languages:** English (this file) · [한국어 → README.ko.md](README.ko.md)

**Mathless** is a statically-typed extension language that keeps Object Pascal / Delphi's
type safety and native performance, is written in a surface syntax familiar to a broad
developer audience, and is distributed **not as source but as protected native modules**
loaded over a **C ABI**.

Tagline candidates:

- *Mathless — Pascal for those who dropped math*
- *Mathless — 수학포기자를 위한 현대적 Pascal*

## One-line definition

Modern surface syntax + strong static typing + compile-time transformation + native binary
distribution + C-ABI host integration.

## Why

Delphi / Object Pascal is strong on type safety, native performance, and structure, but its
ecosystem is closed and its dynamic-extension, tooling, and AI experience lag. Existing script
languages (Python, Lua, JS) are flexible but weak on typing and on protecting the distributed
artifact. Mathless pulls Delphi's strengths **outward**: a familiar surface, a native execution
model underneath, source-free native-module distribution, and host integration **without
recompiling the host**.

## Confirmed core

1. Name is **Mathless**.
2. Execution model is **pure native** — no default VM / bytecode runtime.
3. Distribution is a **native shared library** (DLL / `.so`) — **no source shipped**.
4. Protection goal is to **raise the cost of analysis and tampering** — *not* a claim that
   reversing is impossible.
5. Hosts are **not limited to Delphi**. The first-class boundary is the **C ABI**.
6. Expressiveness targets **complex logic and state**, not simple rule tables.
7. The surface syntax is broader than Delphi-specific syntax; the internal model stays
   Delphi / native.
8. The middle layer is **compile-time only** — no runtime interpretation.
9. Browser / web is **not** a first target (WASM considered later, as a separate target).
10. One strength first — no chasing several goals at once.

## Phase 0 decisions (Q1–Q5 → D14–D18)

| # | Decision |
|---|----------|
| D14 | Primary hosts = **Delphi** (flagship demo / first-class host) + **C** (ABI reference boundary) |
| D15 | Surface family = **brace-based, statically-typed, value-first C-family** (MVP subset = struct + free functions + modules) |
| D16 | Memory = **3-layer ownership** — args = borrowed (call duration only) / returns = caller-allocates / long-lived state = explicit context handle |
| D17 | Errors = **integer status code + out-parameter across the ABI**; surface `Result` is sugar; no exceptions cross the boundary |
| D18 | Module format = **standard DLL/SO + exported ABI-version symbol + module-specific export prefix**; encrypted/signed container deferred to P1 |

Details and rejected alternatives: [docs/DECISIONS.md](docs/DECISIONS.md).

## Document map (read in this order)

| # | File | Content |
|---|------|---------|
| 0 | [CLAUDE.md](CLAUDE.md) | Agent working rules, do-nots, priorities |
| 1 | [docs/VISION.md](docs/VISION.md) | Why, success criteria |
| 2 | [docs/DECISIONS.md](docs/DECISIONS.md) | Confirmed decisions / rejected alternatives |
| 3 | [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) | Compile / distribute / run pipeline |
| 4 | [docs/LANGUAGE.md](docs/LANGUAGE.md) | Surface syntax vs internal model |
| 5 | [docs/HOST_ABI.md](docs/HOST_ABI.md) | C ABI, host integration |
| 6 | [docs/SECURITY.md](docs/SECURITY.md) | Protection goals and stages |
| 7 | [docs/ROADMAP.md](docs/ROADMAP.md) | MVP → expansion |
| 8 | [docs/OPEN_QUESTIONS.md](docs/OPEN_QUESTIONS.md) | Still-open questions |
| 9 | [docs/COMPETITIVE.md](docs/COMPETITIVE.md) | Prior art and differentiation |
| 10 | [docs/GLOSSARY.md](docs/GLOSSARY.md) | Terms |

## Success criterion (initial)

Load a Mathless-compiled native module into a host **without recompiling the host** and call a
typed function (e.g. `discount(price, vip)`).

## Contributing / workflow

All changes go through **Pull Requests** — no direct commits to `main`. See
[CONTRIBUTING.md](CONTRIBUTING.md).

## Topics / tags

**English (GitHub topics):** `programming-language` · `compiler` · `native` · `c-abi` ·
`delphi` · `object-pascal` · `dll` · `shared-library` · `plugin-system` · `static-typing` ·
`code-protection` · `extension-language` · `compile-time` · `design-docs`

**한글 태그:** 프로그래밍언어 · 컴파일러 · 네이티브 · C-ABI · 델파이 · 오브젝트파스칼 · DLL ·
공유라이브러리 · 플러그인시스템 · 정적타입 · 코드보호 · 확장언어 · 컴파일타임 · 설계문서

> GitHub topics accept only lowercase ASCII + hyphens, so the Korean tags above are documented
> here rather than set as repository topics.

## Project status

This repository is currently a **design-baseline**, not yet an implementation repo. Phase 0
decisions (D14–D18) are locked; the next step is the Phase 1 vertical slice (C ABI draft →
minimal compiler pipeline → load into one Delphi host). See [docs/ROADMAP.md](docs/ROADMAP.md).

## License

Not yet decided (TBD).

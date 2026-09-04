# c-host — the acceptance-D host

A small C11 program that loads a module `mlc` produced and calls it. It is the answer to
`docs/phase1/SPEC.md` §3-D: *"load the same `discount.dll` from a real Delphi or C host"* —
the C arm of it.

It is **not** a crate and not part of the workspace. It is compiled by
`hosts/rust-oracle/tests/c_host.rs`, which builds the artifacts with `mlc`, invokes MSVC `cl`
against the **generated** headers, runs the resulting executable, and asserts on its output.

## What a green run proves

- The generated `.h` is valid C11 and compiles clean under `/W4 /WX`.
- Its declarations are *correct*, not merely parseable: each function-pointer type is checked
  against the header's own declaration with `_Static_assert` + `_Generic`, so a changed
  signature breaks the build instead of silently mismatching at runtime.
- The module resolves and runs the way D18 / `HOST_ABI.md` describe a host works: open the
  file, look exports up **by name**, call. This host uses no import library and no link-time
  binding — the host is never rebuilt when the module is replaced. (That is *this* host's
  property, not a limit of the toolchain: since `SPEC-linkable-bindings`, `mlc build` also
  ships a `.lib` and [`hosts/c-host-link`](../c-host-link) proves the other path.)
- Both the scalar path (`mlx_discount`) and the D17 error path (`mlx_safe_div`: status +
  out-param, out untouched on failure, `ML_SAFE_DIV_ERR_DIV_BY_ZERO` taken from the header).

## What it does NOT prove

- **Anything about Delphi.** There is no `dcc64` here; the generated `.pas` has never been
  compiled and keeps its DRAFT note. D14's official pair is Delphi + C, and only C is proven.
- Any C compiler other than MSVC, and any target other than Windows x64 (D22).
- That a *third-party* host rejects an ABI major-version mismatch. This host does reject —
  `gate()` refuses on a version or fingerprint mismatch before the first call, on every
  module it loads — but that is one host obeying the contract, not the contract being
  enforced. Nothing in the emitted module can make an inattentive host check
  (`HOST_ABI.md` §버전, `SPEC-iface-hash.md` §5.1).

## Running it

```
cargo test -p ml_oracle --test c_host -- --nocapture
```

With no MSVC installed the test prints `GATE_D_SKIPPED` and returns — **a skip is not a
pass**. CI sets `MATHLESS_GATE_D=require`, which turns a missing toolchain into a failure so
the gate cannot quietly stop running.

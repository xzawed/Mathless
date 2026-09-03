# `c-host-link` — the C host that binds at link time

The second consumption path, added by [`SPEC-linkable-bindings`](../../docs/slices/SPEC-linkable-bindings.md)
(§3-B, §3-D). Sibling of [`hosts/c-host`](../c-host), not a replacement for it.

|  | `hosts/c-host` | `hosts/c-host-link` (this one) |
|---|---|---|
| binds | at run time | at **link** time |
| needs | `.dll` + `.h` | `.dll` + `.h` + **`.lib`** |
| resolves symbols with | `LoadLibrary` / `GetProcAddress` | the linker |
| if the module is absent | the host runs and reports it | the process **does not start** |
| proves | the dynamic path (acceptance D) | that the generated header is actually linkable |

Both are kept. Acceptance D's checks are the evidence for the dynamic path; replacing them
with this one would have deleted that evidence rather than added to it (DP-L4).

## Why it exists

A C programmer handed a `.h` and a `.dll` usually includes the header, links, and calls the
function. Until the import library shipped, that did not work — the generated header emits
plain prototypes and nothing to link against — and nobody noticed, because the reference
host uses those declarations only as a `_Generic` type oracle and calls through
`GetProcAddress`. `compiler/src/header.rs` had recorded link-time binding as unverified.

## What it does NOT prove

- **That skipping the gate is safe.** This host calls `ml_module_abi_version()` and
  `ml_iface_hash()` and refuses on a mismatch, because a linked host is *more* exposed than a
  dynamic one: a drifted module exporting the same names with the same C types resolves
  perfectly well. Measured — the harness runs this binary beside a drifted `discount.dll`
  and it exits `3` with `refuse: interface …`. Nothing in the module forces a third-party
  host to check (`SPEC-iface-hash.md` §5.1).
- **Anything about Delphi**, or any C compiler other than MSVC.

## Running it

```
MATHLESS_GATE_D=require cargo test -p ml_oracle --test c_host -- --nocapture
```

The harness lives beside acceptance D in `hosts/rust-oracle/tests/c_host.rs`
(`a_c_host_that_links_against_the_import_library`). It is a SEPARATE `#[test]` with its own
temp tree, so the two hosts do not share a working directory — they run concurrently.
A run that prints `GATE_LINK_OK` closed the gate; `GATE_LINK_SKIPPED` did not.

## Exit codes

`0` passed · `2` ABI version mismatch · `3` interface fingerprint mismatch · `4` wrong value.
Distinct on purpose, so the harness can tell which check refused without reading prose.

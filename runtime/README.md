# runtime/

Thin C-ABI surface for Mathless modules (Phase 1). No VM, no interpreter (D02/D11).

## Reserved symbol namespaces (D18)

| Namespace | Owner | Example |
|-----------|-------|---------|
| `ml_*` | runtime / reserved | `ml_module_abi_version` |
| `mlx_*` | user module exports | `mlx_discount` |

- Hosts resolve exports by name via `GetProcAddress` (Windows, measured) or `dlsym` (POSIX, not yet: there is no `.so` target).
- `ml_module_abi_version() -> u32`: hosts are **required** to refuse a **major** mismatch. The reference C host does refuse, on every module it loads, before the first call (`hosts/c-host/host.c`, acceptance D). For a **third-party** host it stays a contract — nothing in the module enforces it, and the Rust oracle only asserts the value matches the compiler constant.
- `ml_abi.h`: C header artifact. The **C** binding is verified — a real MSVC-built host
  loads a module and calls it (acceptance D, `hosts/c-host`). The **Delphi** binding is
  still unverified: no `dcc64` here, so no generated `.pas` has ever been compiled.
  See `docs/phase1/SPEC.md` §3-D.

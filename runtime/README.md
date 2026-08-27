# runtime/

Thin C-ABI surface for Mathless modules (Phase 1). No VM, no interpreter (D02/D11).

## Reserved symbol namespaces (D18)

| Namespace | Owner | Example |
|-----------|-------|---------|
| `ml_*` | runtime / reserved | `ml_module_abi_version` |
| `mlx_*` | user module exports | `mlx_discount` |

- Hosts resolve exports by name via `GetProcAddress` / `dlsym`.
- `ml_module_abi_version() -> u32`: the host refuses to load on a **major** mismatch.
- `ml_abi.h`: C header artifact (DRAFT — the C/Delphi binding is unverified while the
  D14 host-load gate is BLOCKED; see `docs/phase1/SPEC.md` §3-D).

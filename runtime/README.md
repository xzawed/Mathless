# runtime/

Thin C-ABI surface for Mathless modules (Phase 1). No VM, no interpreter (D02/D11).

## Reserved symbol namespaces (D18)

| Namespace | Owner | Example |
|-----------|-------|---------|
| `ml_*` | runtime / reserved | `ml_module_abi_version` |
| `mlx_*` | user module exports | `mlx_discount` |

- Hosts resolve exports by name via `GetProcAddress` (Windows, measured) or `dlsym` (POSIX, not yet: there is no `.so` target).
- `ml_module_abi_version() -> u32`: hosts are **required** to refuse a **major** mismatch. The reference C host does refuse, on every module it loads, before the first call (`hosts/c-host/host.c`, acceptance D). For a **third-party** host it stays a contract — nothing in the module enforces it, and the Rust oracle only asserts the value matches the compiler constant.
- `ml_iface_hash() -> u64`: the module's **interface fingerprint** — a hash over its
  host-visible contract (exported signatures including parameter names, and the error table).
  A host that was built against a different interface must refuse the module. **Both reference
  C hosts do**, before the first call, on every module they load — and the drifted module is
  refused with the symbols still resolving and the ABI version still matching, which is the
  whole point. Both hosts are cited because both check: `hosts/c-host/host.c` `gate()` on the
  dynamic path, and `hosts/c-host-link/host.c:42-45` on the linked one, which compares
  `ml_iface_hash()` against `ML_DISCOUNT_IFACE_HASH` and exits 3 (acceptance D). The generated header pins the
  expected value as `ML_<MODULE>_IFACE_HASH`. Like the version above, for a **third-party**
  host this is a contract: nothing in the module enforces it (`SPEC-iface-hash` §5.1).
- `ml_abi.h`: the hand-written C header in this directory. **Not compiler output** — `mlc`
  emits `.dll`/`.h`/`.pas`/`.lib` and never copies this file, which is why
  `LICENSE-OUTPUT-EXCEPTION` §3 names it as *not* Compiler Output. The **C** binding is
  verified — a real MSVC-built host
  loads a module and calls it (acceptance D, `hosts/c-host`). The **Delphi** binding is
  still unverified: no `dcc64` here, so no generated `.pas` has ever been compiled.
  See `docs/phase1/SPEC.md` §3-D.

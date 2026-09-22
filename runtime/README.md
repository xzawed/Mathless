# runtime/

Thin C-ABI surface for Mathless modules (Phase 1). No VM, no interpreter (D02/D11).

## Reserved symbol namespaces (D18)

| Namespace | Owner | Example |
|-----------|-------|---------|
| `ml_*` | runtime / reserved | `ml_module_abi_version` |
| `mlx_*` | user module exports | `mlx_discount` |

- A **dynamic** host resolves exports by name via `GetProcAddress` (Windows, measured) or
  `dlsym` (POSIX, not yet: there is no `.so` target). **Two of the three host directories
  below do not** — `hosts/c-host-link` is bound by the linker from the `.lib`, and
  `hosts/delphi-host` consumes a generated unit whose functions are `external ML_MODULE`,
  bound when the program loads. The name still has to match; what differs is who resolves it
  and when, and that is why a missing reserved symbol fails at link or at startup there
  rather than returning NULL.
- `ml_module_abi_version() -> u32`: hosts are **required** to refuse a **major** mismatch. The reference C host does refuse, on every module it loads, before the first call (`hosts/c-host/host.c`, acceptance D). For a **third-party** host it stays a contract — nothing in the module enforces it, and the Rust oracle only asserts the value matches the compiler constant.
- `ml_iface_hash_<module>() -> u64`: the module's **interface fingerprint** — a hash over its
  host-visible contract (exported signatures including parameter names, and the error table).
  **The name carries the module stem**, so `discount.dll` answers to `ml_iface_hash_discount`
  and nothing answers to a bare `ml_iface_hash`. That is not cosmetic: every module exporting
  the same bare name meant a *linked* host could bind only one of them and the linker chose
  silently (`SPEC-qualified-iface-hash`, #215). `ml_abi.h` in this directory says the same.
  A host that was built against a different interface must refuse the module. **Both reference
  C hosts do**, before the first call, on every module they load — and the drifted module is
  refused with the symbols still resolving and the ABI version still matching, which is the
  whole point. Both hosts are cited because both check: `hosts/c-host/host.c` `gate()` on the
  dynamic path, and `hosts/c-host-link/host.c:42-45` on the linked one, which compares
  `ml_iface_hash_discount()` against `ML_DISCOUNT_IFACE_HASH` and exits 3 (acceptance D). The generated header pins the
  expected value as `ML_<MODULE>_IFACE_HASH`. Like the version above, for a **third-party**
  host this is a contract: nothing in the module enforces it (`SPEC-iface-hash` §5.1).
- `ml_abi.h`: the hand-written C header in this directory. **Not compiler output** — `mlc`
  emits `.dll`/`.h`/`.pas`/`.lib` and never copies this file, which is why
  `LICENSE-OUTPUT-EXCEPTION` §3 names it as *not* Compiler Output.

  **Which binding is verified how is stated in `ml_abi.h` itself, not here.** That header is
  the contract file and the one the compiler tests read; this paragraph used to carry a second
  copy of its status, and the two drifted — `ml_abi.h` learned the qualified fingerprint name
  on 2026-09-12 and this file did not. One copy, in the file that owns it.

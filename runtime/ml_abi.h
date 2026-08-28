/* Mathless module ABI — Phase 1 DRAFT.
 * See docs/HOST_ABI.md and decisions D18/D19/D21.
 *
 * STATUS: this header is a generated-artifact seed. The C/Delphi binding is NOT yet
 * verified — the D14 host-load gate is BLOCKED (no cl/gcc/dcc64 on the dev machine).
 * Do not treat the boolean/marshalling details below as confirmed until a C or Delphi
 * host actually loads a module (SPEC §3-D).
 */
#ifndef ML_ABI_H
#define ML_ABI_H

#include <stdint.h>
#include <stdbool.h>

#ifdef __cplusplus
extern "C" {
#endif

/* Reserved runtime/version symbol (ml_* namespace). Contract for hosts: resolve
 * this via GetProcAddress/dlsym and refuse the module on a major-version
 * mismatch. NOTE: that refusal is a requirement on the host, not yet implemented
 * anywhere in this repo — the Rust oracle only asserts the value matches. */
uint32_t ml_module_abi_version(void);

/* User module exports use the mlx_ prefix (distinct from the runtime ml_*).
 * `vip` is a 1-byte boolean to match the module's `extern "C" fn(f64, bool)` ABI;
 * a Delphi host must use a 1-byte Boolean (not LongBool). PROVISIONAL — see STATUS. */
double mlx_discount(double price, bool vip);

#ifdef __cplusplus
}
#endif

#endif /* ML_ABI_H */

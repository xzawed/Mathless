/* Mathless module ABI - Phase 1.
 * See docs/HOST_ABI.md and decisions D18/D19/D21.
 *
 * STATUS: this header is a generated-artifact seed. The C binding IS verified - a C11
 * host built with MSVC loads a module and calls it (acceptance D, hosts/c-host). The
 * DELPHI binding is still unverified: there is no dcc64 here, so no generated .pas has
 * ever been compiled. Treat the Delphi-facing details as unconfirmed (SPEC section 3-D).
 */
#ifndef ML_ABI_H
#define ML_ABI_H

#include <stdint.h>
#include <stdbool.h>

#ifdef __cplusplus
extern "C" {
#endif

/* Reserved runtime/version symbol (ml_* namespace). Contract for hosts: resolve
 * this via GetProcAddress (Windows today; there is no .so target yet, so the
 * dlsym path is unexercised) and refuse the module on a major-version
 * mismatch. The reference C host does exactly that since 2026-09-02 — it refuses
 * before the first call on every module it loads (`hosts/c-host/host.c`, gate()).
 * For a third-party host it stays a CONTRACT: nothing in the module enforces it,
 * and the Rust oracle only asserts the value matches. */
uint32_t ml_module_abi_version(void);

/* User module exports use the mlx_ prefix (distinct from the runtime ml_*).
 * `vip` is a 1-byte boolean to match the module's `extern "C" fn(f64, bool)` ABI;
 * a Delphi host must use a 1-byte Boolean (not LongBool). PROVISIONAL — see STATUS. */
double mlx_discount(double price, bool vip);

#ifdef __cplusplus
}
#endif

#endif /* ML_ABI_H */

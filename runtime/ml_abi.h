/* Mathless module ABI - Phase 1.
 * See docs/HOST_ABI.md and decisions D17/D18/D19/D21.
 *
 * WHAT THIS FILE IS. The reserved half of the module ABI, written by hand: the symbols
 * every Mathless module exports whatever its source says, and the status codes reserved by
 * the runtime. It is a CONTRACT, not a build input - nothing includes it, and `mlc` does
 * not copy it into a user's tree (D23: the generated .h/.pas/.dll are the artifacts).
 *
 * Read it beside a generated header, not instead of one. The generated header declares the
 * module's own `mlx_*` functions and repeats the reserved declarations below so it stands
 * alone; the definitions here use the same `#ifndef` shape, so including both is safe.
 *
 * STATUS: the C binding IS verified in CI - a C11 host built with MSVC loads a module and
 * calls it (acceptance D, hosts/c-host). The DELPHI binding is verified too, but only on a
 * developer machine: MATHLESS_GATE_DELPHI builds hosts/delphi-host with dcc64 through the
 * IDE (bds.exe -b, since the edition here refuses command-line builds) and calls the
 * modules. That gate cannot run in CI - no Delphi, no interactive session - so the
 * Delphi-facing details are checked, but not on every push (SPEC section 3-D).
 */
#ifndef ML_ABI_H
#define ML_ABI_H

#include <stdint.h>

/* ---------------------------------------------------------------- status space (D17)
 *
 * A fallible function returns `int32_t` and writes its result through an out-parameter.
 * The space is flat (Q13, closed 2026-08-28):
 *
 *     0        success
 *     > 0      module-defined domain error, emitted as ML_<MODULE>_ERR_<NAME> in the
 *              module's generated header (Q14, closed 2026-09-03). The module is in the
 *              name because an error name is module-scoped in the surface language: two
 *              modules that both declare E_NEG with different values would otherwise
 *              define the same macro twice, and a host including both headers failed to
 *              build - measured, C4005 under /W4 /WX. Same stem as ML_<MODULE>_IFACE_HASH
 *              below, so this file teaches one naming rule rather than two.
 *     < 0      RESERVED for the runtime and the ABI. A module never invents one.
 *
 * DP-E2 said the reserved negative range would be standardised here. This is that: the
 * codes below are the whole allocation, and -2 .. -255 is held in reserve. A host may
 * treat any unknown negative value as "the call failed for a reason this host predates".
 *
 * DP-E3: when the status is not 0, the host does NOT read the out-parameter.
 */
#define ML_ST_OK 0

/* Q12 caller-allocates protocol: the buffer was too small to hold the result, NUL
 * included. Truncation is a FAILURE, not a short success - nothing is written to the
 * buffer, and *ml_needed carries the exact size to allocate, in the same unit as ml_cap.
 *
 * Guarded because every generated header carrying a `-> string!` function emits the same
 * definition, so a translation unit may see both. Keep the value identical there. */
#ifndef ML_ST_INSUFFICIENT_BUFFER
#define ML_ST_INSUFFICIENT_BUFFER (-1)
#endif

#ifdef __cplusplus
extern "C" {
#endif

/* ---------------------------------------------------------------- reserved exports
 *
 * Every module exports these two, whatever its source says. `doc_claims.rs` fails if the
 * compiler starts emitting a reserved `ml_*` declaration that is missing from this file -
 * which is how ml_iface_hash came to be absent here for a slice after it shipped.
 *
 * One of the two is spelled with the module's name in it, so only ONE of them can be
 * declared here. Which one, and why it is that one, is the subject of the second block
 * below.
 */

/* The module ABI version. Contract for hosts: resolve it (GetProcAddress on Windows today;
 * there is no .so target yet, so the dlsym path is unexercised) and refuse the module on a
 * major mismatch. The reference C host does refuse, since 2026-09-02, and it does so
 * before the first call on every module it loads (hosts/c-host/host.c, gate()). For a
 * third-party host it stays a CONTRACT: nothing in the module enforces it, and the Rust
 * oracle only asserts the value matches.
 *
 * Deliberately NOT qualified with the module name, unlike the fingerprint below. Its value
 * is a compiler constant - every module built by one compiler answers the same number - so
 * a linker binding one module's copy for all of them returns the right answer anyway. And
 * it is D18's bootstrap: resolving this symbol is the only way a host can ask a module
 * which ABI it speaks, so a host that had to know the module name first would have nothing
 * left to negotiate with. The cost is recorded rather than hidden: modules built by two
 * DIFFERENT compiler versions and linked together would bind one version symbol and skip
 * the other's. That is outside the one-compiler premise, it is an argument and not a
 * measurement, and no slice fixes it today. */
uint32_t ml_module_abi_version(void);

/* A fingerprint of the module's host-visible interface - the exported signatures including
 * parameter names, and the error codes reachable through them. The generated header pins
 * the value it was built against as ML_<MODULE>_IFACE_HASH, so a host can refuse a module
 * whose interface moved even though every symbol still resolves and the ABI version still
 * matches. That failure was measured before this existed: a silently wrong answer, and an
 * access violation.
 *
 * The export carries the MODULE NAME, so this file cannot declare it - the name is not
 * fixed. Every generated header declares its own:
 *
 *   uint64_t ml_iface_hash_<module>(void);
 *
 * It was a bare `ml_iface_hash` until SPEC-qualified-iface-hash. This is the one reserved
 * export whose VALUE differs per module while its name did not, and that combination has a
 * measured consequence: a C host that LINKS two modules gets one binding for both, chosen
 * by the linker with no `cl /W4` diagnostic, and calls the other module with no interface
 * check at all - the exact state the fingerprint exists to prevent. The version above keeps
 * its bare name on purpose (see there).
 *
 * A DYNAMIC host builds the name from the module it is loading: the module name is the
 * source file's stem, so `discount.dll` answers to `ml_iface_hash_discount`.
 *
 * NOT integrity (P1). An attacker who edits the module edits this function too. What it
 * stops is a module swapped without rebuilding the host. */

/* User modules export their own functions under the mlx_ prefix, distinct from the runtime
 * ml_* namespace above (D18). Those declarations belong in the module's generated header,
 * not here - this file used to declare `mlx_discount` from one example, which is exactly
 * the kind of module-specific detail that goes stale in a file nobody compiles.
 *
 * One convention worth stating once: a `bool` parameter is a 1-byte boolean, matching the
 * module's `extern "C" fn(f64, bool)`. A Delphi host must use a 1-byte Boolean, not
 * LongBool. MEASURED (2026-09-07): declaring LongBool instead makes a module's false read
 * as TRUE in the host, with the module still writing exactly one byte - no crash, wrong
 * answer. Pinned by the Pascal host's bool canary. */

#ifdef __cplusplus
}
#endif

#endif /* ML_ABI_H */

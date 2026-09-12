/* Acceptance B and D of SPEC-linkable-bindings: a C host that binds at LINK time.
 *
 * The other host (hosts/c-host) proves the dynamic path - LoadLibrary, GetProcAddress, a
 * function pointer per export. That is one way to consume a Mathless module, and until this
 * slice it was the only way anyone had ever proved. A C programmer handed a `.h` and a
 * `.dll` will usually do the other thing: include the header, link the import library, call
 * the function. That path did not work, because no import library was shipped.
 *
 * So this file exists to be the small, boring host. There is deliberately NO GetProcAddress
 * and no LoadLibrary in it: every symbol below is resolved by the linker from discount.lib,
 * and the module is bound when the process starts. If discount.dll is missing, this program
 * does not start at all - which is the same coupling the generated Delphi unit has, and one
 * reason the two are worth proving separately.
 *
 * usage: host_link <expected_abi_version>
 *
 * Exit codes are distinct so the harness can tell WHICH check refused, rather than reading
 * prose out of stdout:
 *   0  everything passed
 *   2  ABI version mismatch
 *   3  interface fingerprint mismatch
 *   4  the module loaded and answered, but with the wrong value
 */
#include <stdio.h>
#include <stdlib.h>

#include "discount.h"
/* And a buffer-triple return, because a scalar one does not exercise the interesting half.
 * Until 2026-09-11 this host linked ONE scalar module while the generated header told every
 * reader the generator was verified "both ways a C host can consume it" -- true of the shape
 * `discount` has, and never tested for the shape Q12 produces. Found by audit. */
#include "schedule.h"

int main(int argc, char **argv) {
    unsigned long expected_abi = (argc > 1) ? strtoul(argv[1], NULL, 10) : 1UL;

    /* The gate, through linked symbols. Nothing here is looked up by name at run time, so
     * the only thing standing between this host and a swapped module is this comparison -
     * which is exactly the point SPEC-iface-hash section 5.1 makes about hosts that skip it.
     * A linked host has MORE reason to check, not less: the symbols resolve either way. */
    uint32_t abi = ml_module_abi_version();
    if (abi != (uint32_t)expected_abi) {
        printf("refuse: module abi %u, host built for %lu\n", abi, expected_abi);
        return 2;
    }

    /* ONE fingerprint check per linked module, and that is the whole point of the symbol
     * carrying the module's name. Until SPEC-qualified-iface-hash every module exported a
     * bare `ml_iface_hash`, so the two calls below would have been the same call: the linker
     * bound one of them for both, silently, and this host checked `discount` twice while
     * calling `schedule` unchecked. Measured, with no `cl /W4` diagnostic.
     *
     * The refusal names the module, because "which one drifted" is the question a host
     * operator actually has. The exit code stays 3 for either - it is the same kind of
     * refusal, and a caller that needs the detail reads the line. */
    uint64_t iface = ml_iface_hash_discount();
    if (iface != ML_DISCOUNT_IFACE_HASH) {
        printf("refuse discount: interface %016llX, header pinned %016llX\n",
               (unsigned long long)iface, (unsigned long long)ML_DISCOUNT_IFACE_HASH);
        return 3;
    }

    uint64_t iface_schedule = ml_iface_hash_schedule();
    if (iface_schedule != ML_SCHEDULE_IFACE_HASH) {
        printf("refuse schedule: interface %016llX, header pinned %016llX\n",
               (unsigned long long)iface_schedule,
               (unsigned long long)ML_SCHEDULE_IFACE_HASH);
        return 3;
    }

    /* A plain call. No cast, no function-pointer typedef, no adapter to get wrong - the
     * declaration in the header IS the calling contract, which is the property the dynamic
     * host has to reconstruct by hand with _Generic. */
    double vip = mlx_discount(100.0, true);
    double std = mlx_discount(100.0, false);
    if (vip != 90.0 || std != 100.0) {
        printf("FAIL: discount(100,true)=%f discount(100,false)=%f\n", vip, std);
        return 4;
    }

    /* The Q12 shape, through the linker. The declaration in schedule.h is the whole contract:
     * five parameters, the last three of them the buffer triple, and getting any of them
     * wrong is a COMPILE error here rather than a silently shifted argument. That is the
     * property a dynamic host cannot have -- it rebuilds the signature by hand in a typedef.
     *
     * ml_cap and ml_needed count ELEMENTS for an array return, so the allocation carries the
     * multiplication. Writing malloc(needed) here would promise four times the room actually
     * present, and the module would believe it. */
    int32_t needed = -1;
    if (mlx_schedule(100000, 7, NULL, 0, &needed) >= 0 || needed != 7) {
        printf("FAIL: probe gave status>=0 or needed=%d\n", needed);
        return 4;
    }
    int32_t *parts = malloc((size_t)needed * sizeof *parts);
    if (parts == NULL) {
        return 4;
    }
    int32_t got = -1;
    int32_t st = mlx_schedule(100000, 7, parts, needed, &got);
    int32_t sum = 0;
    for (int32_t i = 0; i < got; i++) {
        sum += parts[i];
    }
    free(parts);
    if (st != 0 || got != 7 || sum != 99995) {
        printf("FAIL: schedule st=%d needed=%d sum=%d\n", st, got, sum);
        return 4;
    }

    printf("LINK_GATE_OK: bound by the linker, gate passed, "
           "discount(100,true)=%.1f discount(100,false)=%.1f, "
           "schedule sums to %d over %d elements\n", vip, std, sum, got);
    return 0;
}

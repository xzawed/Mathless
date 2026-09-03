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

    uint64_t iface = ml_iface_hash();
    if (iface != ML_DISCOUNT_IFACE_HASH) {
        printf("refuse: interface %016llX, header pinned %016llX\n",
               (unsigned long long)iface, (unsigned long long)ML_DISCOUNT_IFACE_HASH);
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

    printf("LINK_GATE_OK: bound by the linker, gate passed, "
           "discount(100,true)=%.1f discount(100,false)=%.1f\n", vip, std);
    return 0;
}

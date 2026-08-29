/* Mathless Gate-D host - a real C program that loads a module `mlc` produced and calls it.
 *
 * This is acceptance D (docs/phase1/SPEC.md section 3-D): the same `.dll` the Rust oracle loads,
 * loaded instead by a C11 host built with MSVC `cl`, over the plain C ABI.
 *
 * What it proves, and how:
 *   - The GENERATED headers are valid C11 and their declarations are correct: each function
 *     pointer type below is checked against the header's own declaration with `_Static_assert`
 *     + `_Generic`, so if a generated signature ever changes shape this file stops compiling.
 *     (The check is unevaluated, so it needs no import library.) The error constant comes
 *     from the header too (`ML_ERR_DIV_BY_ZERO`), not from a number retyped here.
 *   - The module is resolved the way D18 / HOST_ABI describe a host does it: open the file,
 *     look exports up BY NAME, call. No import library, no link-time binding - the host is
 *     never rebuilt when the module is replaced.
 *
 * What it does NOT prove: anything about Delphi (`.pas` stays DRAFT), and it is not a check
 * that a host *rejects* an ABI major mismatch - that remains a contract on hosts, unimplemented.
 *
 * usage: host <artifact_dir> <expected_abi_version>
 */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <windows.h>

#include "discount.h"
#include "safe_div.h"
#include "sum_to.h"
#include "negate_if.h"
#include "count_bounded.h"
#include "discount4.h"
#include "line_total.h"

typedef uint32_t (*abi_version_fn)(void);
typedef double (*discount_fn)(double, bool);
typedef int32_t (*safe_div_fn)(double, double, double *);
typedef int32_t (*sum_to_fn)(int32_t);
typedef int32_t (*negate_if_fn)(int32_t, bool);
typedef int32_t (*count_bounded_fn)(int32_t, int32_t);
typedef double (*discount4_fn)(double, bool);
typedef double (*line_total_fn)(double, int32_t);

/* These are the teeth: each pointer type must be *identical* to the type of the function the
   generated header declares. `_Generic` selects on the declaration's own type and the whole
   expression is unevaluated, so this is a pure compile-time check that links nothing. Change
   a generated signature and the C host stops building instead of silently calling through a
   mismatched pointer. */
_Static_assert(_Generic(&ml_module_abi_version, abi_version_fn: 1, default: 0),
               "generated ml_module_abi_version signature changed");
_Static_assert(_Generic(&mlx_discount, discount_fn: 1, default: 0),
               "generated mlx_discount signature changed");
_Static_assert(_Generic(&mlx_safe_div, safe_div_fn: 1, default: 0),
               "generated mlx_safe_div signature changed");
_Static_assert(_Generic(&mlx_sum_to, sum_to_fn: 1, default: 0),
               "generated mlx_sum_to signature changed");
_Static_assert(_Generic(&mlx_negate_if, negate_if_fn: 1, default: 0),
               "generated mlx_negate_if signature changed");
_Static_assert(_Generic(&mlx_count_bounded, count_bounded_fn: 1, default: 0),
               "generated mlx_count_bounded signature changed");
_Static_assert(_Generic(&mlx_discount4, discount4_fn: 1, default: 0),
               "generated mlx_discount4 signature changed");
_Static_assert(_Generic(&mlx_line_total, line_total_fn: 1, default: 0),
               "generated mlx_line_total signature changed");

static int failures = 0;

static void check(int ok, const char *what) {
    if (ok) {
        printf("  ok   %s\n", what);
    } else {
        printf("  FAIL %s\n", what);
        failures++;
    }
}

static HMODULE load(const char *dir, const char *name) {
    char path[MAX_PATH];
    if (snprintf(path, sizeof path, "%s\\%s", dir, name) >= (int)sizeof path) {
        printf("  FAIL path too long for %s\n", name);
        failures++;
        return NULL;
    }
    HMODULE m = LoadLibraryA(path);
    if (m == NULL) {
        printf("  FAIL LoadLibraryA(%s) -> error %lu\n", path, GetLastError());
        failures++;
    }
    return m;
}

static FARPROC sym(HMODULE m, const char *name) {
    FARPROC p = GetProcAddress(m, name);
    if (p == NULL) {
        printf("  FAIL GetProcAddress(%s) -> error %lu\n", name, GetLastError());
        failures++;
    }
    return p;
}

int main(int argc, char **argv) {
    if (argc < 3) {
        fprintf(stderr, "usage: host <artifact_dir> <expected_abi_version>\n");
        return 2;
    }
    const char *dir = argv[1];
    unsigned long expected_abi = strtoul(argv[2], NULL, 10);

    printf("mathless c host: loading from %s\n", dir);

    /* --- discount.dll: the scalar happy path (SPEC section 3-B) --- */
    HMODULE d = load(dir, "discount.dll");
    if (d == NULL) {
        return 1;
    }
    abi_version_fn d_abi = (abi_version_fn)sym(d, "ml_module_abi_version");
    discount_fn discount = (discount_fn)sym(d, "mlx_discount");
    if (d_abi && discount) {
        check(d_abi() == (uint32_t)expected_abi, "discount.dll ml_module_abi_version");
        check(discount(100.0, true) == 90.0, "mlx_discount(100, true) == 90");
        check(discount(100.0, false) == 100.0, "mlx_discount(100, false) == 100");
    }

    /* --- safe_div.dll: the D17 error path, status + out-param --- */
    HMODULE s = load(dir, "safe_div.dll");
    if (s == NULL) {
        return 1;
    }
    abi_version_fn s_abi = (abi_version_fn)sym(s, "ml_module_abi_version");
    safe_div_fn safe_div = (safe_div_fn)sym(s, "mlx_safe_div");
    if (s_abi && safe_div) {
        check(s_abi() == (uint32_t)expected_abi, "safe_div.dll ml_module_abi_version");

        double out = -1.0;
        int32_t status = safe_div(10.0, 2.0, &out);
        check(status == 0, "safe_div(10, 2) status == 0");
        check(out == 5.0, "safe_div(10, 2) writes 5 through the out-param");

        /* On failure the module must leave the out-param untouched (D17). */
        double untouched = 12345.0;
        status = safe_div(1.0, 0.0, &untouched);
        check(status == ML_ERR_DIV_BY_ZERO, "safe_div(1, 0) status == ML_ERR_DIV_BY_ZERO");
        check(untouched == 12345.0, "safe_div(1, 0) leaves the out-param untouched");
    }

    /* --- sum_to.dll: a loop. Until the `while` slice every Mathless function terminated
       trivially; this call is the first one whose return depends on the loop finishing. --- */
    HMODULE w = load(dir, "sum_to.dll");
    if (w == NULL) {
        return 1;
    }
    sum_to_fn sum_to = (sum_to_fn)sym(w, "mlx_sum_to");
    if (sum_to) {
        check(sum_to(10) == 55, "sum_to(10) == 55 (loop accumulates)");
        check(sum_to(0) == 0, "sum_to(0) == 0 (body runs zero times)");
    }

    /* --- negate_if.dll: unary `-` and `!`. --- */
    HMODULE u = load(dir, "negate_if.dll");
    if (u == NULL) {
        return 1;
    }
    negate_if_fn negate_if = (negate_if_fn)sym(u, "mlx_negate_if");
    if (negate_if) {
        check(negate_if(7, false) == 7, "negate_if(7, false) == 7 (!flip)");
        check(negate_if(7, true) == -7, "negate_if(7, true) == -7 (unary minus)");
    }

    /* --- count_bounded.dll: two conditions in one loop header (`&&`). --- */
    HMODULE l = load(dir, "count_bounded.dll");
    if (l == NULL) {
        return 1;
    }
    count_bounded_fn count_bounded = (count_bounded_fn)sym(l, "mlx_count_bounded");
    if (count_bounded) {
        check(count_bounded(10, 3) == 3, "count_bounded(10, 3) == 3 (cap stops it)");
        check(count_bounded(3, 10) == 3, "count_bounded(3, 10) == 3 (n stops it)");
    }

    /* --- discount4.dll: an internal helper decides the rate. The helper is NOT exported, so
       GetProcAddress must fail for it - that is the D04/D05 claim, checked from the host. --- */
    HMODULE h4 = load(dir, "discount4.dll");
    if (h4 == NULL) {
        return 1;
    }
    discount4_fn discount4 = (discount4_fn)sym(h4, "mlx_discount4");
    if (discount4) {
        check(discount4(100.0, true) == 90.0, "discount4(100, true) == 90 (helper picked 0.9)");
        check(discount4(100.0, false) == 100.0, "discount4(100, false) == 100");
    }
    check(GetProcAddress(h4, "vip_rate") == NULL, "the internal helper is not exported");
    check(GetProcAddress(h4, "mlx_vip_rate") == NULL, "...not under the mlx_ prefix either");

    /* --- line_total.dll: a cast lets a count meet a price. --- */
    HMODULE lt = load(dir, "line_total.dll");
    if (lt == NULL) {
        return 1;
    }
    line_total_fn line_total = (line_total_fn)sym(lt, "mlx_line_total");
    if (line_total) {
        check(line_total(2.5, 4) == 10.0, "line_total(2.5, 4) == 10 (qty as f64)");
        check(line_total(2.5, 0) == 0.0, "line_total(2.5, 0) == 0");
    }

    FreeLibrary(d);
    FreeLibrary(s);
    FreeLibrary(w);
    FreeLibrary(u);
    FreeLibrary(l);
    FreeLibrary(h4);
    FreeLibrary(lt);

    if (failures == 0) {
        printf("GATE_D_OK\n");
        return 0;
    }
    printf("GATE_D_FAILED (%d)\n", failures);
    return 1;
}

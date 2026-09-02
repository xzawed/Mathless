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
 *   - A host REFUSES a module it was not built for. Every module below passes a load-time
 *     gate (abi version + interface fingerprint) before a single call, and with a third
 *     argument the host is handed a deliberately drifted module and must turn it away.
 *
 * What it does NOT prove: anything about Delphi (`.pas` stays DRAFT).
 *
 * usage: host <artifact_dir> <expected_abi_version> [drifted_module.dll]
 */
/* <math.h> is here for the rounding checks: DP-R3 says the module's floor/ceil/round/trunc
   match C's exactly, so the honest test is to call both and compare - including signbit(),
   because the sign of zero is invisible to ==. */
#include <math.h>
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
#include "commission.h"
#include "deduction.h"
#include "pack.h"
#include "vat.h"
#include "carrier.h"
#include "quote.h"

typedef uint32_t (*abi_version_fn)(void);
typedef uint64_t (*iface_hash_fn)(void);
typedef double (*discount_fn)(double, bool);
typedef int32_t (*safe_div_fn)(double, double, double *);
typedef int32_t (*sum_to_fn)(int32_t);
typedef int32_t (*negate_if_fn)(int32_t, bool);
typedef int32_t (*count_bounded_fn)(int32_t, int32_t);
typedef double (*discount4_fn)(double, bool);
typedef double (*line_total_fn)(double, int32_t);
typedef int32_t (*boxes_fn)(int32_t, int32_t);
typedef int32_t (*boxes_checked_fn)(int32_t, int32_t, int32_t *);
typedef double (*commission_fn)(double, int32_t *);
typedef double (*unary_f64_fn)(double);
typedef int32_t (*commission_checked_fn)(double, int32_t *, double *);
typedef double (*vat_rate_fn)(const char *);
typedef int32_t (*issuer_of_fn)(const char *);
typedef bool (*is_export_item_fn)(const char *);
typedef int32_t (*carrier_name_fn)(const char *, char *, int32_t, int32_t *);
typedef int32_t (*carrier_label_fn)(const char *, int32_t *, char *, int32_t, int32_t *);
typedef int32_t (*unit_price_fn)(double, int32_t, double *);
typedef int32_t (*line_check_fn)(int32_t, int32_t *, int32_t *);

/* These are the teeth: each pointer type must be *identical* to the type of the function the
   generated header declares. `_Generic` selects on the declaration's own type and the whole
   expression is unevaluated, so this is a pure compile-time check that links nothing. Change
   a generated signature and the C host stops building instead of silently calling through a
   mismatched pointer. */
_Static_assert(_Generic(&ml_module_abi_version, abi_version_fn: 1, default: 0),
               "generated ml_module_abi_version signature changed");
_Static_assert(_Generic(&ml_iface_hash, iface_hash_fn: 1, default: 0),
               "generated ml_iface_hash signature changed");
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
_Static_assert(_Generic(&mlx_boxes, boxes_fn: 1, default: 0),
               "generated mlx_boxes signature changed");
_Static_assert(_Generic(&mlx_boxes_checked, boxes_checked_fn: 1, default: 0),
               "generated mlx_boxes_checked signature changed");
/* An out-param must reach C as a POINTER. If the generator ever emitted it by value again,
   these two would stop matching and the host would fail to compile - which is the whole
   point: the bug this slice fixes was a signature that looked right and was not. */
_Static_assert(_Generic(&mlx_commission, commission_fn: 1, default: 0),
               "generated mlx_commission signature changed");
_Static_assert(_Generic(&mlx_commission_checked, commission_checked_fn: 1, default: 0),
               "generated mlx_commission_checked signature changed");
_Static_assert(_Generic(&mlx_deduction, unary_f64_fn: 1, default: 0),
               "generated mlx_deduction signature changed");
/* DP-S1: a string parameter must reach C as `const char*`. If the generator ever emitted a
   pointer+length pair, or a plain char*, these stop compiling - which is the point: the shape
   appears at every host call site, so it has to be caught at the boundary, not at runtime. */
_Static_assert(_Generic(&mlx_vat_rate, vat_rate_fn: 1, default: 0),
               "generated mlx_vat_rate signature changed");
_Static_assert(_Generic(&mlx_issuer_of, issuer_of_fn: 1, default: 0),
               "generated mlx_issuer_of signature changed");
_Static_assert(_Generic(&mlx_is_export_item, is_export_item_fn: 1, default: 0),
               "generated mlx_is_export_item signature changed");
/* The Q12 buffer triple must reach C as `char*, int32_t, int32_t*` and the function must
   return the STATUS, not the string. Getting any of the three slots wrong is the kind of
   defect that still compiles and still produces a plausible number, so it is pinned here at
   compile time rather than discovered by reading bytes. */
_Static_assert(_Generic(&mlx_carrier_name, carrier_name_fn: 1, default: 0),
               "generated mlx_carrier_name signature changed");
_Static_assert(_Generic(&mlx_carrier_label, carrier_label_fn: 1, default: 0),
               "generated mlx_carrier_label signature changed");
/* The fallible-calls slice must not touch the C ABI at all: a function that propagates a
   helper's status is declared exactly like any other D17 fallible export. If the internal
   Result shape ever leaked into the boundary, these two would stop matching. */
_Static_assert(_Generic(&mlx_unit_price, unit_price_fn: 1, default: 0),
               "generated mlx_unit_price signature changed");
_Static_assert(_Generic(&mlx_line_check, line_check_fn: 1, default: 0),
               "generated mlx_line_check signature changed");

static int failures = 0;

static void check(int ok, const char *what) {
    if (ok) {
        printf("  ok   %s\n", what);
    } else {
        printf("  FAIL %s\n", what);
        failures++;
    }
}

static HMODULE load_raw(const char *dir, const char *name) {
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

/* The load-time gate (DP-H8/H9). Returns 1 only if the module is the one this host was
 * built against.
 *
 * Both halves matter and neither is decoration:
 *   - the ABI version was already a documented host duty, but HOST_ABI recorded the
 *     rejection path as unimplemented; here it is implemented.
 *   - the interface fingerprint is the half the version cannot do. Two modules with
 *     incompatible signatures report the same version, export the same names, and resolve
 *     identically - measured, with a wrong return value in one case and 0xC0000005 in the
 *     other (SPEC-iface-hash section 0.1).
 *
 * Deliberately silent about WHAT differs beyond the numbers: the host is not a diagnostic
 * tool for the module author, and printing signatures would leak the module's surface. */
static int gate(HMODULE m, const char *name, unsigned long expected_abi, uint64_t pinned) {
    abi_version_fn abi = (abi_version_fn)(void *)GetProcAddress(m, "ml_module_abi_version");
    iface_hash_fn hash = (iface_hash_fn)(void *)GetProcAddress(m, "ml_iface_hash");
    if (abi == NULL || hash == NULL) {
        printf("  refuse %s: a reserved symbol is missing\n", name);
        return 0;
    }
    if (abi() != (uint32_t)expected_abi) {
        printf("  refuse %s: module abi %u, host built for %lu\n", name, abi(), expected_abi);
        return 0;
    }
    if (hash() != pinned) {
        printf("  refuse %s: interface %016llX, header pinned %016llX\n", name,
               (unsigned long long)hash(), (unsigned long long)pinned);
        return 0;
    }
    return 1;
}

/* Every module this host loads passes the gate before a single call is made. Before this
 * slice only discount.dll and safe_div.dll compared even the version; the other eleven
 * compared nothing. */
static HMODULE load(const char *dir, const char *name, unsigned long expected_abi,
                    uint64_t pinned) {
    HMODULE m = load_raw(dir, name);
    if (m == NULL) {
        return NULL;
    }
    if (!gate(m, name, expected_abi, pinned)) {
        FreeLibrary(m);
        printf("  FAIL %s did not pass the load-time gate\n", name);
        failures++;
        return NULL;
    }
    printf("  ok   %s passed the abi + interface gate\n", name);
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
    HMODULE d = load(dir, "discount.dll", expected_abi, ML_DISCOUNT_IFACE_HASH);
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
    HMODULE s = load(dir, "safe_div.dll", expected_abi, ML_SAFE_DIV_IFACE_HASH);
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
    HMODULE w = load(dir, "sum_to.dll", expected_abi, ML_SUM_TO_IFACE_HASH);
    if (w == NULL) {
        return 1;
    }
    sum_to_fn sum_to = (sum_to_fn)sym(w, "mlx_sum_to");
    if (sum_to) {
        check(sum_to(10) == 55, "sum_to(10) == 55 (loop accumulates)");
        check(sum_to(0) == 0, "sum_to(0) == 0 (body runs zero times)");
    }

    /* --- negate_if.dll: unary `-` and `!`. --- */
    HMODULE u = load(dir, "negate_if.dll", expected_abi, ML_NEGATE_IF_IFACE_HASH);
    if (u == NULL) {
        return 1;
    }
    negate_if_fn negate_if = (negate_if_fn)sym(u, "mlx_negate_if");
    if (negate_if) {
        check(negate_if(7, false) == 7, "negate_if(7, false) == 7 (!flip)");
        check(negate_if(7, true) == -7, "negate_if(7, true) == -7 (unary minus)");
    }

    /* --- count_bounded.dll: two conditions in one loop header (`&&`). --- */
    HMODULE l = load(dir, "count_bounded.dll", expected_abi, ML_COUNT_BOUNDED_IFACE_HASH);
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
    HMODULE h4 = load(dir, "discount4.dll", expected_abi, ML_DISCOUNT4_IFACE_HASH);
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
    HMODULE lt = load(dir, "line_total.dll", expected_abi, ML_LINE_TOTAL_IFACE_HASH);
    if (lt == NULL) {
        return 1;
    }
    line_total_fn line_total = (line_total_fn)sym(lt, "mlx_line_total");
    if (line_total) {
        check(line_total(2.5, 4) == 10.0, "line_total(2.5, 4) == 10 (qty as f64)");
        check(line_total(2.5, 0) == 0.0, "line_total(2.5, 0) == 0");
    }

    /* --- pack.dll: i32 `/` and `%` are TOTAL. Both edge cases are the ones where C's own
       integer division is undefined behaviour, so a C host is exactly the right place to
       check that the module RETURNS a defined value instead of trapping. --- */
    HMODULE pk = load(dir, "pack.dll", expected_abi, ML_PACK_IFACE_HASH);
    if (pk == NULL) {
        return 1;
    }
    boxes_fn boxes = (boxes_fn)sym(pk, "mlx_boxes");
    boxes_fn loose = (boxes_fn)sym(pk, "mlx_loose");
    if (boxes && loose) {
        check(boxes(17, 5) == 3, "boxes(17, 5) == 3");
        check(loose(17, 5) == 2, "loose(17, 5) == 2");
        check(boxes(-17, 5) == -3, "boxes(-17, 5) == -3 (truncates toward zero)");
        check(loose(-17, 5) == -2, "loose(-17, 5) == -2 (sign follows the dividend)");
        /* The whole point of the slice: in C these two would be UB. */
        check(boxes(17, 0) == 0, "boxes(17, 0) == 0 (total, not a trap)");
        check(loose(17, 0) == 0, "loose(17, 0) == 0 (total, not a trap)");
        check(boxes(INT32_MIN, -1) == INT32_MIN, "boxes(INT32_MIN, -1) wraps to INT32_MIN");
        check(loose(INT32_MIN, -1) == 0, "loose(INT32_MIN, -1) == 0");
    }
    boxes_checked_fn boxes_checked = (boxes_checked_fn)sym(pk, "mlx_boxes_checked");
    if (boxes_checked) {
        int32_t out = -999;
        int32_t status = boxes_checked(17, 5, &out);
        check(status == 0, "boxes_checked(17, 5) status == 0");
        check(out == 3, "boxes_checked(17, 5) writes 3 through the out-param");

        int32_t untouched = -999;
        status = boxes_checked(17, 0, &untouched);
        check(status == ML_ERR_E_EMPTY_BOX, "boxes_checked(17, 0) status == ML_ERR_E_EMPTY_BOX");
        check(untouched == -999, "boxes_checked(17, 0) leaves the out-param untouched");
    }

    /* --- commission.dll: a declared `out` parameter. The host allocates; the module writes
       through the pointer. This is the case that could not be expressed at all before. --- */
    HMODULE cm = load(dir, "commission.dll", expected_abi, ML_COMMISSION_IFACE_HASH);
    if (cm == NULL) {
        return 1;
    }
    commission_fn commission = (commission_fn)sym(cm, "mlx_commission");
    if (commission) {
        int32_t tier = -1;
        double fee = commission(500000.0, &tier);
        check(fee == 500000.0 * 0.03, "commission(500000) fee");
        check(tier == 1, "commission(500000) writes tier 1 through the out-param");

        tier = -1;
        fee = commission(9000000.0, &tier);
        check(fee == 9000000.0 * 0.07, "commission(9000000) fee");
        check(tier == 3, "commission(9000000) writes tier 3");
    }
    commission_checked_fn commission_checked =
        (commission_checked_fn)sym(cm, "mlx_commission_checked");
    if (commission_checked) {
        /* DP-O1: (inputs..., declared outs..., out_value). */
        int32_t tier = -1;
        double fee = -1.0;
        int32_t status = commission_checked(500000.0, &tier, &fee);
        check(status == 0, "commission_checked(500000) status == 0");
        check(tier == 1, "commission_checked writes the declared out");
        check(fee == 500000.0 * 0.03, "commission_checked writes out_value");

        /* DP-O3: a failure writes neither. */
        tier = -7;
        fee = -7.0;
        status = commission_checked(-1.0, &tier, &fee);
        check(status == ML_ERR_E_NEGATIVE, "commission_checked(-1) status == ML_ERR_E_NEGATIVE");
        check(tier == -7, "a failed call leaves the declared out untouched");
        check(fee == -7.0, "a failed call leaves out_value untouched");
    }

    /* --- deduction.dll: the rounding builtins. A C host is the right place to check these,
       because DP-R3 says they match <math.h> exactly - so this compares the module against
       the C library sitting right next to it, not against our own expectations. --- */
    HMODULE dd = load(dir, "deduction.dll", expected_abi, ML_DEDUCTION_IFACE_HASH);
    if (dd == NULL) {
        return 1;
    }
    unary_f64_fn deduction = (unary_f64_fn)sym(dd, "mlx_deduction");
    if (deduction) {
        check(deduction(3000000.0) == 135000.0, "deduction(3000000) == 135000");
        /* The whole reason for the slice: `(x) as i32 as f64` returned 2147483647 here. */
        check(deduction(50000000000.0) == 2250000000.0,
              "deduction(50000000000) == 2250000000 (no longer saturated)");
        check(deduction(1000000000000.0) == 45000000000.0,
              "deduction(1000000000000) == 45000000000");
    }
    unary_f64_fn fl = (unary_f64_fn)sym(dd, "mlx_fl");
    unary_f64_fn ce = (unary_f64_fn)sym(dd, "mlx_ce");
    unary_f64_fn ro = (unary_f64_fn)sym(dd, "mlx_ro");
    unary_f64_fn tr = (unary_f64_fn)sym(dd, "mlx_tr");
    if (fl && ce && ro && tr) {
        check(fl(2.6) == floor(2.6) && fl(-2.4) == floor(-2.4), "floor agrees with <math.h>");
        check(ce(2.4) == ceil(2.4) && ce(-2.6) == ceil(-2.6), "ceil agrees with <math.h>");
        check(ro(2.5) == round(2.5) && ro(-2.5) == round(-2.5), "round agrees with <math.h>");
        check(tr(2.9) == trunc(2.9) && tr(-2.9) == trunc(-2.9), "trunc agrees with <math.h>");
        /* The trap a naive `floor(x + 0.5)` falls into. */
        check(ro(0.49999999999999994) == round(0.49999999999999994),
              "round(0.49999999999999994) agrees with <math.h> (not 1)");
        /* Sign of zero - invisible to ==, so compare the bit patterns. */
        check(signbit(ce(-0.5)) == signbit(ceil(-0.5)), "ceil(-0.5) keeps the sign of zero");
        check(signbit(fl(-0.0)) == signbit(floor(-0.0)), "floor(-0.0) keeps the sign of zero");
    }

    /* --- vat.dll: string INPUT parameters (SPEC-string-input sections 3-B and 3-B2).
       A C host is the right place for this one: it passes ordinary C string literals, which
       is what DP-S1 chose the ABI for, and the comparison is byte equality up to the NUL. --- */
    HMODULE vt = load(dir, "vat.dll", expected_abi, ML_VAT_IFACE_HASH);
    if (vt == NULL) {
        return 1;
    }
    vat_rate_fn vat_rate = (vat_rate_fn)sym(vt, "mlx_vat_rate");
    issuer_of_fn issuer_of = (issuer_of_fn)sym(vt, "mlx_issuer_of");
    is_export_item_fn is_export_item = (is_export_item_fn)sym(vt, "mlx_is_export_item");
    if (vat_rate && issuer_of && is_export_item) {
        check(vat_rate("KR") == 0.1, "vat_rate(\"KR\") == 0.1");
        check(vat_rate("JP") == 0.08, "vat_rate(\"JP\") == 0.08");
        check(vat_rate("US") == 0.0, "vat_rate(\"US\") == 0.0 (falls through)");

        /* Section 3-B2 - each of these passes under some looser comparison. */
        check(vat_rate("kr") == 0.0, "vat_rate(\"kr\") == 0.0 (case matters)");
        check(vat_rate("") == 0.0, "vat_rate(\"\") == 0.0");
        check(vat_rate("KRW") == 0.0, "vat_rate(\"KRW\") == 0.0 (longer is not a match)");
        check(vat_rate("K") == 0.0, "vat_rate(\"K\") == 0.0 (a prefix is not a match)");

        /* The NUL ends it: a buffer with garbage after the terminator still matches "KR".
           Built here rather than written as a literal so the trailing bytes are certainly
           present in memory. */
        char padded[8] = {'K', 'R', '\0', 'Z', 'Z', 'Z', 'Z', 'Z'};
        check(vat_rate(padded) == 0.1, "vat_rate(\"KR\\0ZZZZZ\") == 0.1 (stops at the NUL)");

        check(issuer_of("4") == 1, "issuer_of(\"4\") == 1");
        check(issuer_of("51") == 2, "issuer_of(\"51\") == 2");
        check(issuer_of("5") == 0, "issuer_of(\"5\") == 0 (\"5\" is not \"51\")");

        /* `!=` is the negated loop, so it needs its own value. */
        check(is_export_item("DOM") == false, "is_export_item(\"DOM\") == false");
        check(is_export_item("EXP") == true, "is_export_item(\"EXP\") == true");
    }

    /* --- carrier.dll: the Q12 caller-allocates protocol (SPEC-string-return section 3-D).
       A C host is where this one belongs: the host owns the buffer, and the two-call probe is
       an idiom a C author already knows from GetUserNameA / snprintf. Note the buffers are
       ZEROED before every call - DP-T2 chose "the module writes nothing on failure" over the
       Annex K fail-safe, so a correct host does not read a buffer it did not initialise. That
       is the habit this example is here to show. --- */
    HMODULE cr = load(dir, "carrier.dll", expected_abi, ML_CARRIER_IFACE_HASH);
    if (cr == NULL) {
        return 1;
    }
    carrier_name_fn carrier_name = (carrier_name_fn)sym(cr, "mlx_carrier_name");
    carrier_label_fn carrier_label = (carrier_label_fn)sym(cr, "mlx_carrier_label");
    if (carrier_name && carrier_label) {
        char buf[64];
        int32_t needed;
        int32_t st;

        /* Section 3-B: the ordinary call. `needed` counts the NUL, so "UPS Ground" is 11. */
        memset(buf, 0, sizeof buf);
        needed = -7;
        st = carrier_name("UPSN", buf, (int32_t)sizeof buf, &needed);
        check(st == 0, "carrier_name(\"UPSN\") status == 0");
        check(strcmp(buf, "UPS Ground") == 0, "carrier_name(\"UPSN\") writes \"UPS Ground\"");
        check(needed == 11, "needed == 11 (10 characters + the NUL)");

        /* Section 3-B2: truncation is a FAILURE, and nothing is written. The canary here is
           the zero fill: a partial copy would leave "UPS" in the buffer. */
        memset(buf, 0, sizeof buf);
        needed = -7;
        st = carrier_name("UPSN", buf, 4, &needed);
        check(st == ML_ST_INSUFFICIENT_BUFFER, "cap = 4 -> ML_ST_INSUFFICIENT_BUFFER");
        check(buf[0] == '\0', "a truncated call writes nothing");
        check(needed == 11, "and reports the exact size to allocate");

        /* Section 3-B3: the probe. `ml_buf` may be NULL exactly when `ml_cap` is 0 (DP-T7),
           and the answer is in the same unit as the capacity, so the retry always fits. */
        needed = -7;
        st = carrier_name("UPSN", NULL, 0, &needed);
        check(st == ML_ST_INSUFFICIENT_BUFFER, "probe (NULL, 0) does not crash");
        check(needed == 11, "probe reports 11");
        char *exact = (char *)malloc((size_t)needed);
        if (exact != NULL) {
            memset(exact, 0, (size_t)needed); /* same habit: never read what you did not write */
            int32_t again = -7;
            st = carrier_name("UPSN", exact, needed, &again);
            check(st == 0, "the probed size is enough (converges in two calls)");
            check(strcmp(exact, "UPS Ground") == 0, "and the second call fills it");
            free(exact);
        }

        /* Section 3-B4: exact fit is success; one less is truncation. */
        memset(buf, 0, sizeof buf);
        needed = -7;
        check(carrier_name("UPSN", buf, 11, &needed) == 0, "cap == needed is success");
        memset(buf, 0, sizeof buf);
        needed = -7;
        check(carrier_name("UPSN", buf, 10, &needed) == ML_ST_INSUFFICIENT_BUFFER,
              "cap == needed - 1 is truncation (the NUL does not fit)");

        /* Section 3-B5: an empty result is a value. `needed` is 1, never 0. */
        memset(buf, 0xAA, sizeof buf);
        needed = -7;
        st = carrier_name("NONE", buf, 1, &needed);
        check(st == 0, "an empty result is a success");
        check(buf[0] == '\0' && needed == 1, "empty means one byte: the NUL");

        /* Section 3-B6: a domain error touches neither the buffer nor needed (DP-T8). */
        memset(buf, 0xAA, sizeof buf);
        needed = -7;
        st = carrier_name("ZZ99", buf, (int32_t)sizeof buf, &needed);
        check(st == ML_ERR_E_UNKNOWN_SCAC, "an unknown code is a positive D17 status");
        check((unsigned char)buf[0] == 0xAA, "a failed call writes no bytes");
        check(needed == -7, "and leaves *ml_needed alone");

        /* DP-O1: a declared out comes before the triple. */
        memset(buf, 0, sizeof buf);
        int32_t tier = -7;
        needed = -7;
        st = carrier_label("UPSN", &tier, buf, (int32_t)sizeof buf, &needed);
        check(st == 0 && tier == 1, "carrier_label writes the declared out first");
        check(strcmp(buf, "UPS Ground") == 0, "...and the string into the triple");
    }

    /* --- quote.dll: a status propagated out of a reused internal helper
       (SPEC-fallible-calls section 3-D). What this proves from the C side is that the
       propagation is invisible here: `mlx_unit_price` looks and behaves like any other D17
       fallible export, and the code it returns is the helper's own. --- */
    HMODULE qt = load(dir, "quote.dll", expected_abi, ML_QUOTE_IFACE_HASH);
    if (qt == NULL) {
        return 1;
    }
    unit_price_fn unit_price = (unit_price_fn)sym(qt, "mlx_unit_price");
    line_check_fn line_check = (line_check_fn)sym(qt, "mlx_line_check");
    if (unit_price && line_check) {
        double v = -7.0;
        int32_t st = unit_price(100.0, 4, &v);
        check(st == 0, "unit_price(100, 4) status == 0");
        check(v == 25.0, "unit_price(100, 4) == 25");

        /* The helper's code, two levels down, arriving unchanged. */
        v = -7.0;
        st = unit_price(100.0, 0, &v);
        check(st == ML_ERR_E_BAD_QTY, "unit_price(100, 0) propagates ML_ERR_E_BAD_QTY");
        check(v == -7.0, "a propagated failure leaves out_value untouched");

        /* A declared out alongside a propagating call: written on success, and never reached
           on failure because the `try` left the function first. */
        int32_t tier = -7;
        int32_t iv = -7;
        st = line_check(5, &tier, &iv);
        check(st == 0 && tier == 1 && iv == 5, "line_check(5) writes both outs");

        tier = -7;
        iv = -7;
        st = line_check(0, &tier, &iv);
        check(st == ML_ERR_E_BAD_QTY, "line_check(0) propagates the helper's code");
        check(tier == -7, "the try exited before the out was assigned");
        check(iv == -7, "and out_value is untouched");
    }

    FreeLibrary(qt);
    FreeLibrary(cr);
    FreeLibrary(vt);
    FreeLibrary(dd);
    FreeLibrary(cm);
    FreeLibrary(pk);
    FreeLibrary(d);
    FreeLibrary(s);
    FreeLibrary(w);
    FreeLibrary(u);
    FreeLibrary(l);
    FreeLibrary(h4);
    FreeLibrary(lt);

    /* --- the gate must actually REFUSE (SPEC-iface-hash section 3-F) ---
     *
     * argv[3], when present, names a module in `dir` built from a CHANGED pack interface:
     * `boxes(qty, per_box)` became `boxes(per_box, qty)`. That drift is invisible to C -
     * both are `int32_t mlx_boxes(int32_t, int32_t)` - so name resolution succeeds and the
     * call returns a plausible wrong number. Measured: 33 became 0.
     *
     * A gate that is merely present proves nothing (`runtime/ml_abi.h` is this repo's own
     * example of a contract nobody calls), so the refusal is exercised here, and the two
     * checks below say WHY it is the fingerprint doing the work: the symbol still resolves,
     * and the ABI version still matches. */
    if (argc >= 4) {
        HMODULE drift = load_raw(dir, argv[3]);
        if (drift != NULL) {
            check(!gate(drift, argv[3], expected_abi, ML_PACK_IFACE_HASH),
                  "the gate refuses a module whose interface drifted");
            check(GetProcAddress(drift, "mlx_boxes") != NULL,
                  "control: the drifted module still resolves mlx_boxes by name");
            abi_version_fn drift_abi =
                (abi_version_fn)(void *)GetProcAddress(drift, "ml_module_abi_version");
            check(drift_abi != NULL && drift_abi() == (uint32_t)expected_abi,
                  "control: the drifted module still reports the expected abi version");
            FreeLibrary(drift);
        }
    }

    if (failures == 0) {
        printf("GATE_D_OK\n");
        return 0;
    }
    printf("GATE_D_FAILED (%d)\n", failures);
    return 1;
}

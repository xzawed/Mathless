# `hosts/delphi-host` — gated locally, not in CI

**`MATHLESS_GATE_DELPHI` builds this host with `dcc64` and runs it, and it passes.** The
edition available refuses command-line builds, so the gate drives the IDE instead
(`bds.exe -b`), which that edition does allow. Free Pascal also builds and runs this host on
every full local test — a different compiler answering a smaller question.

**The gate does not run in CI**: the runner has neither Delphi nor an interactive desktop
session. So this arm is checked, but not on every push — whoever changes the generator or
this host has to run the gate themselves.

This file has twice described a state it had already left, so the record is kept below
rather than rewritten:

- **2026-09-02** — `dcc64` was not on this machine (measured: absent from PATH and disk).
  `host.dpr` was a draft that nothing had ever compiled.
- **2026-09-07** (§9-14, §9-15) — Free Pascal compiled it first and that run found a real
  defect in it; a Delphi IDE build then passed every check by hand (`GATE_DELPHI_OK`).
- **2026-09-09** (§9-20) — the gate learned the `bds.exe -b` fallback, so the run repeats.

## Why it exists

`D14` names **Delphi + C** as the two official hosts. `hosts/c-host` proves the C half on
every push; this directory proves the Delphi half, but only where a Delphi is installed.
The failure mode it exists to catch is not hypothetical, and it is one **no C host can
reach**: passing a Delphi `UnicodeString` where a `PAnsiChar` is expected **compiles, does
not crash, and matches no code at all**. The module cannot detect it — it sees bytes. That
hazard is now measured rather than feared (§9-23), and this host writes all three spellings
on purpose so the gate pins what each one sends.

## Running it

```
cargo test -p ml_oracle --test delphi_host -- --nocapture
```

Without a compiler this prints `GATE_DELPHI_SKIPPED` and returns. **A skipped gate is not a
passed gate** — the message says so on purpose.

To demand it instead:

```
MATHLESS_GATE_DELPHI=require cargo test -p ml_oracle --test delphi_host -- --nocapture
```

`require` turns a missing compiler into a failure instead of a skip, and that is how this
host is actually verified. **CI does not set it, and cannot**: the runner has neither Delphi
nor an interactive desktop session, and the gate needs the IDE because the edition available
refuses command-line builds.

## What it checks

The same ground `hosts/c-host` covers, from the other language:

- the load-time gate — abi version **and** interface fingerprint (`SPEC-iface-hash`);
- scalars, with `Boolean` as **1 byte** (the generated unit says so; `LongBool` would read
  three bytes of noise);
- D17 — status plus an out-param, and the out-param **untouched** when the call fails;
- Q12 — the caller's buffer, with truncation as a failure that writes **nothing** (canary),
  and a declared `out` ordered before the buffer triple (DP-O1).

## One real difference from the C host

`hosts/c-host` resolves every symbol with `LoadLibrary`/`GetProcAddress`. The generated `.pas`
instead declares `external ML_MODULE`, which Delphi binds when the **program** loads. So a
Delphi host cannot decline to start when a module is missing — the loader refuses first and
the process never reaches `begin`. The fingerprint check is therefore written as *refuse to
use*, not *refuse to load*. That difference between the two official hosts is worth knowing
before anyone writes a binding for a third language.

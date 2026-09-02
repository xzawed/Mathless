# `hosts/delphi-host` — staged, unverified

**Nothing in this directory has ever been compiled.** `dcc64` is not on this machine
(measured: absent from PATH and disk; the registry's `Embarcadero\Studio\15.0` key is the
leftover of a removed install). `host.dpr` is a **draft**, written ahead of time so that the
day a Delphi compiler arrives, verifying D14's other half is one command rather than one
session.

Do not cite this directory as evidence of anything. The generated `.pas` units remain
**DRAFT** — that is what `D21` and `STATUS.md` §1 say, and it stays true until the gate below
passes on a real machine.

## Why it exists

`D14` names **Delphi + C** as the two official hosts. Only C is proven: `hosts/c-host` builds
with MSVC, loads the produced `.dll` and checks values (acceptance D). Nothing has ever
compiled the generated `.pas`, so half of the flagship story is untested — and the failure
mode it would catch is not hypothetical. `HOST_ABI.md` records one already: passing a Delphi
`UnicodeString` where a `PAnsiChar` is expected **compiles, does not crash, and matches no
code at all**. The module cannot detect it; only a real Delphi host can.

## Running it

```
cargo test -p ml_oracle --test delphi_host -- --nocapture
```

Without a compiler this prints `GATE_DELPHI_SKIPPED` and returns. **A skipped gate is not a
passed gate** — the message says so on purpose.

With `dcc64` installed:

```
MATHLESS_GATE_DELPHI=require cargo test -p ml_oracle --test delphi_host -- --nocapture
```

`require` turns a missing compiler into a failure instead of a skip. CI does **not** set it:
demanding a toolchain nobody has would only paint the build red. That variable is the whole
switch on the day the compiler exists.

## What it checks, once it runs

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

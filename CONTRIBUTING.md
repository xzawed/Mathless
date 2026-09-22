# Contributing / 기여 가이드

> English first, 한국어 아래. This project works **PR-first**: `main` is never committed to directly.

## Setting up a machine (English)

Everything below is measured on the machine that wrote it. Nothing here is optional folklore —
each item is what a gate actually shells out to.

**Install**

| what | why | check |
|---|---|---|
| **Rust**, via `rustup` | `rust-toolchain.toml` pins **1.97.1** and rustup installs it on first `cargo` run. Bumping it means re-measuring the size proxies — the pin says so. | `rustc --version` |
| **MSVC Build Tools**, "Desktop development with C++" | acceptance D compiles and runs two real C hosts. It supplies `cl`, `link`, `dumpbin`, and the `vswhere.exe` + `vcvars64.bat` the tests use to find them. | `cargo test -p ml_oracle --test c_host -- --nocapture` prints `GATE_D_OK` — **the flag is not optional**: the gate prints the host's transcript from a passing test, and cargo swallows that. Without it you see `ok` and no evidence. |
| **Windows SDK 10.0.20348 or newer** (the floor is Microsoft's fix version, not something measured here; what was measured is the pair below) | acceptance D compiles the generated header after `<windows.h>` with `/W4 /WX /std:c11`. On **10.0.19041** that combination fails inside Microsoft's own `winbase.h(9572)` with **C5105** ("macro expansion producing 'defined'"), which the conforming C11 preprocessor turns on; Microsoft fixed the header in 20348. Measured 2026-09-07: 19041 fails two tests, 26100 passes. A newer SDK usually arrives with the workload above — check it if those two are the only red ones. | `cl /W4 /WX /std:c11` on a file that only `#include <windows.h>` exits 0 |

**Optional, and a skip-gate when absent:** **Free Pascal 3.2.2** (`winget install
FreePascal.FreePascalCompiler`) compiles every generated `.pas` under `-Mdelphi -Sew`.
It is **not** the Delphi gate — D14 names `dcc64`, and that arm has its own gate
(`MATHLESS_GATE_DELPHI`, which runs locally, never in CI). This one proves only that the
generated text is valid Object Pascal, the same kind of claim the C++ gate makes about the
header. `MATHLESS_GATE_FPC=require` turns a missing compiler
into a failure, and **CI sets it** — the windows job installs Free Pascal first, so the
gate can never quietly stop running there. Locally it skips, loudly, when fpc is absent. Check: `cargo test -p ml_oracle --test fpc_units --
--nocapture` prints `GATE_FPC_OK`. A second test in that file builds and RUNS
`hosts/delphi-host/host.dpr` against real modules; it needs an fpc that can target
x86_64 (the official win32.and.win64 installer; chocolatey's package delivers an
i386-only fpc, which is why the windows job installs that file directly), so it has its
own `MATHLESS_GATE_FPC_HOST` — and **CI sets that to `require` too**, so it prints
`GATE_FPC_HOST_LOADBIND_OK` rather than skipping. It is not the Delphi gate either.

Nothing else. The suite shells out to **no other tool** — no node, no python, no make. Third-party
Rust dependencies are **zero** (`Cargo.lock` holds the two local crates and nothing more).

`dcc64` (Delphi) was absent on every machine until 2026-09-07, and where it exists now its
edition **refuses command-line builds** — it prints "does not support command line compiling",
writes nothing, and exits 0. **That no longer blocks the gate.** When `dcc64` refuses,
`MATHLESS_GATE_DELPHI` falls back to `bds.exe -b` (the IDE builder, which the same edition
allows) and builds a Win64 host in about twelve seconds; it passes on the development machine
(2026-09-09). The generated `.pas` is compiled by **Free Pascal on every push**
(`MATHLESS_GATE_FPC`, above) and by **real `dcc64` locally**, and it does not ship marked DRAFT
— the banner it carries names the gate that checks it.

**What is left is CI, and that is a different problem**: the runner has neither Delphi nor an
interactive session, so `MATHLESS_GATE_DELPHI` cannot run there. Installing Delphi on your own
machine is therefore not "fixing the setup" — it lets you run one more local gate.

> Until 2026-09-22 this paragraph said the opposite on all three counts — that nothing had
> ever compiled the generated unit, that the Delphi arm was blocked, and that the unit shipped
> carrying a draft marker. All three were false; the first was contradicted twenty lines above
> in this same file. It stood for sixteen days because no guard read this document. One does
> now — which is also why this note describes the old sentences rather than quoting them, so
> that recording the correction cannot itself trip the guard.

**Then run what CI runs, and judge by the exit code** (the three commands are in step 8 below).

### What legitimately differs on another machine — do not read these as breakage

1. **The module byte-size pin will fail on a third machine, on purpose.**
   `hosts/rust-oracle/tests/protection.rs` pins **9,728 B** on the development machine and
   **9,216 B** on GitHub's `windows-latest`, chosen by `GITHUB_ACTIONS`. Anything else asserts
   out. That is not a defect in the module: the toolchain pin covers **rustc**, not MSVC
   `link.exe` or the Windows SDK, so a different SDK gives a different size. The failure
   message says exactly this and what to do — re-measure, and update the constant **and every
   document that publishes the pair** in the same commit. `doc_claims.rs` guards five —
   `README.md`, `README.ko.md`, `docs/SECURITY.md`, `docs/STATUS.md` and **this file**, which
   joined on 2026-09-22 after publishing both values four times while telling you only four
   documents were guarded. A partial update stays red.

   Still grep rather than trusting this list: the guard's scope is a hand-written array, and
   a sixth document can publish the pair without anything noticing. That is the general limit
   of every hand-scoped guard here, and it is why the gate list in step 8 is derived from
   `ci.yml` instead.
2. **`host.c` and the other hand-written C must stay pure ASCII**, and the reason is
   machine-dependent: MSVC reads them in the machine's ANSI code page, so a non-ASCII byte is
   `C4819` → `C2220` under `/W4 /WX` — but only on a code page that cannot represent it. It was
   found on a CP949 machine and would have compiled clean on CP1252. A text-only test guards it
   now, and it runs on both CI jobs so no one has to have the right code page.
3. **A crashed `mlc` leaves `%TEMP%\mlc-build-*` behind.** Normal runs clean up; a process that
   aborts (a deliberate stack-overflow probe, for instance) cannot. Harmless, and
   `docs/STATUS.md` §5-5 keeps it open rather than claiming it fixed.

### Where to start work

`docs/STATUS.md` §9, the **"▶ 여기서 시작한다"** block. It is the handoff: what is closed, what is
deliberately left, and what not to reopen. Read it before the backlog tables, which age faster.

## Workflow (English)

1. **Never commit to `main` directly.** Every change lands via a Pull Request.
2. Branch from `main` using a typed prefix:
   - `docs/*` — documentation / design docs
   - `feat/*` — new implementation
   - `fix/*` — bug fixes
   - `chore/*` — tooling, meta, housekeeping
3. Keep a PR scoped to one concern. Reference the decision/question it touches
   (e.g. `D16`, `Q12`).
4. **Do not overturn a decision in `docs/DECISIONS.md`** without first writing the rationale
   and trade-off into that file (see [CLAUDE.md](CLAUDE.md)).
5. **Evidence level** must match the phase (per CLAUDE.md):
   - Phase 0 (docs) → **E0** (doc consistency) / **E1** (cited external facts). No fabricated
     measurements.
   - Once code exists → **E2** (real build/run artifacts) is required before "done".
6. **Second-order verification via Grok** is required for implementation / diagnosis / code
   review before a PR is marked complete. If a Grok tool fails, report it and ask — do not
   silently self-substitute.
7. Squash-merge into `main`. Delete the branch after merge.
8. **CI** (GitHub Actions) runs `cargo fmt --check` + `clippy -D warnings`
   + `cargo test --workspace` on every PR, in **two jobs**. `windows-latest` is the
   authoritative gate: it is where the `#![cfg(windows)]` acceptance tests actually execute,
   and `MATHLESS_GATE_D=require` makes a missing MSVC a failure rather than a silent skip, so
   acceptance D (a real C host loading the module) cannot quietly stop running. `ubuntu-latest`
   is **insurance, not authority** — it catches a Windows-only assumption in the platform-
   independent frontend, but the acceptance tests compile away there. Keep both green before
   merge. (A cross-platform SO/ELF **target** is still deferred with D22 — the Linux job is
   not it.)

   **Locally, on Windows, run what that job runs — and read the exit codes:**

   ```powershell
   # PowerShell — the default shell on Windows, and it has NO inline `VAR=value cmd` prefix
   cargo fmt --all --check
   cargo clippy --workspace --all-targets -- -D warnings
   $env:MATHLESS_GATE_D = "require"
   $env:MATHLESS_GATE_FPC = "require"
   $env:MATHLESS_GATE_FPC_HOST = "require"
   cargo test --workspace --locked
   ```

   ```sh
   # Git Bash / WSL
   cargo fmt --all --check
   cargo clippy --workspace --all-targets -- -D warnings
   MATHLESS_GATE_D=require MATHLESS_GATE_FPC=require MATHLESS_GATE_FPC_HOST=require \
     cargo test --workspace --locked
   ```

   **The PowerShell form is first because the bash prefix silently does nothing there.**
   `MATHLESS_GATE_D=require cargo test` is a parse error in `cmd` and sets no variable in
   PowerShell, so the natural repair — dropping the prefixes — is exactly the one-gate run this
   change exists to stop. Same shape as the pipe warning below: the wrong form still exits 0.

   **All three, because CI requires all three.** Until 2026-09-22 this block set only
   `MATHLESS_GATE_D`, and that is worse than it sounds: on a machine without Free Pascal the
   other two **skip**, the suite prints green and exits 0, and CI is the one that says no —
   after review time has been spent. A skipped gate is loud on stdout and silent in the exit
   code, which is the same failure the paragraph below warns about for pipes. It warned about
   the wrapper and not about itself. `doc_claims.rs` now derives this list from `ci.yml`, so a
   fourth required gate cannot be added without this block failing.

   `MATHLESS_GATE_DELPHI` is **not** in that list on purpose — it is not `require` in CI,
   because the runner has no Delphi. Run it separately if you have one.

   Without `MATHLESS_GATE_D=require`, a machine with no MSVC **skips** acceptance D and still
   prints a green summary — the skip is loud on stdout, but the exit code is 0. And never pipe
   these: `cargo fmt --all --check | tail -1 && echo clean` reports **`tail`'s** exit code, so
   it says "clean" forever. A commit passed that way and CI caught it.
   Test counts belong in `docs/STATUS.md` §1, not here.

   **Run those commands the way CI runs them, and judge them by the exit code.** Do not wrap
   one in a pipeline to shorten its output. `cargo fmt --all --check | tail -1 && echo clean`
   reports the exit status of `tail`, which is always `0`, so it prints `clean` on a tree that
   fails formatting — that exact wrapper put a badly formatted commit past a local check and
   into CI (`docs/STATUS.md` §7-1). A guard that cannot fail proves nothing.

## Methodology: SDD + WBS + TDD (mandatory from Phase 1)

1. **SDD (spec-first):** write the spec before code — `docs/slices/SPEC-<name>.md` with inputs, outputs,
   contracts, and measurable acceptance criteria. Get user confirmation before implementing.
2. **WBS:** break the spec into PR-sized tasks in `docs/phaseN/WBS.md` (the phase plan, alongside `docs/phaseN/SPEC.md`), in dependency order; each
   task = one PR with a measurable done-criterion.
3. **TDD:** write the failing test first (Red → Green → Refactor). Test results are the E2 evidence.
4. **Grok + measured data (both doing and reviewing):** perform and verify each task with Grok,
   grounded in real data (test results, build artifacts, export dumps, run logs). `grok_build_verify`
   is required before a PR is marked complete.

Order: **SPEC → (user confirm) → WBS → per task [failing test → implement → pass → Grok verify] → PR → merge.**
Never write "it works / it's fast / it's protected" without a measurement.

## Positioning rule

Never describe the protection as "impossible to reverse". The honest phrasing is
**"raises the cost of analysis and tampering"** (see `docs/SECURITY.md`, decision D05).

---

## 머신 준비 (한국어)

아래는 전부 이 문서를 쓴 머신에서 **실측한 것**이다. 관례가 아니라, 게이트가 실제로 호출하는 것들이다.

**설치**

| 무엇 | 왜 | 확인 |
|---|---|---|
| **Rust** (`rustup`) | `rust-toolchain.toml`이 **1.97.1**로 고정한다. 첫 `cargo` 실행 때 rustup이 받아 온다. 올리면 크기 프록시를 다시 재야 하고, 핀 주석이 그렇게 적어 두었다. | `rustc --version` |
| **MSVC Build Tools** — "C++를 사용한 데스크톱 개발" | 수용 D가 실제 C 호스트 **둘**을 컴파일·실행한다. `cl`·`link`·`dumpbin`, 그리고 테스트가 그것들을 찾는 데 쓰는 `vswhere.exe`·`vcvars64.bat`을 준다. | `cargo test -p ml_oracle --test c_host -- --nocapture`가 `GATE_D_OK`를 찍는다 — **플래그는 선택이 아니다**: 게이트는 통과한 테스트에서 호스트의 출력을 찍는데 cargo가 그것을 삼킨다. 빼면 `ok`만 보이고 근거는 안 보인다. |
| **Windows SDK 10.0.20348 이상** (바닥값은 MS가 고친 버전이지 여기서 재 것이 아니다 — 재 것은 아래 두 지점이다) | 수용 D는 생성 헤더를 `<windows.h>` **뒤에** 놓고 `/W4 /WX /std:c11`로 컴파일한다. **10.0.19041**에서는 그 조합이 마이크로소프트 자신의 `winbase.h(9572)`에서 **C5105**("macro expansion producing 'defined'")로 깨진다 — C11 적합 전처리기가 켜는 경고이고, MS가 20348에서 헤더를 고쳤다. 실측(2026-09-07): 19041은 테스트 2건 실패, 26100은 통과. 보통 위 워크로드와 함께 최신 SDK가 들어오니, **저 둘만 빨갛다면 여기를 보라.** | `#include <windows.h>` 한 줄짜리 파일이 `cl /W4 /WX /std:c11`에서 exit 0 |

**선택 사항이고, 없으면 skip-게이트다:** **Free Pascal 3.2.2**
(`winget install FreePascal.FreePascalCompiler`)가 생성 `.pas` 전부를 `-Mdelphi -Sew`로
컴파일한다. **Delphi 게이트가 아니다** — D14는 `dcc64`를 지목하고, 그 반쪽에는 별도 게이트가
있다(`MATHLESS_GATE_DELPHI` — 로컬에서만 돌고 CI에서는 돌지 않는다). 이쪽이 증명하는 것은
**생성 텍스트가 유효한 Object Pascal인가**까지이며, C++ 게이트가 헤더에 대해 하는 주장과 같은 종류다. `MATHLESS_GATE_FPC=require`가 부재를 실패로 바꾸고,
**CI는 그것을 설정한다** — windows 잡이 Free Pascal을 먼저 설치하므로 거기서는 게이트가 조용히
멈출 수 없다. 로컬에서는 fpc가 없으면 **시끄럽게** skip한다. 확인: `cargo test -p ml_oracle --test fpc_units -- --nocapture`가
`GATE_FPC_OK`를 찍는다. 같은 파일의 두 번째 테스트는 `hosts/delphi-host/host.dpr`를
빌드해 **실제 모듈을 로드·호출**한다. 그것은 x86_64를 타깃할 수 있는 fpc가 필요하므로
(공식 win32.and.win64 설치본. chocolatey 패키지는 i386 전용 fpc를 깔기 때문에
windows 잡이 그 파일을 직접 설치한다) **별도 변수 `MATHLESS_GATE_FPC_HOST`** 를 쓰고,
**CI도 그것을 `require`로 설정한다** — skip이 아니라 `GATE_FPC_HOST_LOADBIND_OK`를 찍는다.
이것도 Delphi 게이트가 아니다.

그 외에는 없다. 스위트가 호출하는 **다른 도구는 하나도 없다** — node도, python도, make도. 서드파티
Rust 의존성은 **0개**다(`Cargo.lock`에 로컬 크레이트 둘뿐).

`dcc64`(Delphi)는 2026-09-07까지 어느 머신에도 없었고, 지금 있는 것은 에디션이 **명령줄 빌드를
거부한다**("does not support command line compiling"을 찍고 아무것도 만들지 않으면서 exit 0).
**그러나 그것이 게이트를 막지 않는다.** `dcc64`가 거부하면 `MATHLESS_GATE_DELPHI`가
`bds.exe -b`(같은 에디션이 허용하는 IDE 빌더)로 넘어가 약 12초에 Win64 호스트를 빌드하고,
개발 머신에서 **통과한다**(2026-09-09). 생성 `.pas`는 **매 푸시마다 Free Pascal이**
(위 `MATHLESS_GATE_FPC`), 그리고 **로컬에서는 진짜 `dcc64`가** 컴파일하며, DRAFT로 나가지
않는다 — 유닛이 달고 나가는 배너는 자기를 검사하는 게이트 이름을 적는다.

**남은 것은 CI이고, 그것은 다른 문제다**: 러너에 Delphi도 대화형 세션도 없어
`MATHLESS_GATE_DELPHI`가 거기서 돌 수 없다. 따라서 Delphi를 설치하는 것은 "환경을 맞추는 일"이
아니라 **로컬 게이트를 하나 더 돌릴 수 있게 되는 일**이다.

> 2026-09-22까지 이 문단은 세 가지 모두에서 정반대를 적었다 — 생성 유닛을 컴파일한 것이 하나도
> 없다고, Delphi 쪽이 막혀 있다고, 유닛이 초안 표시를 달고 나간다고. **셋 다 거짓이었고**,
> 첫 번째는 같은 파일 스무 줄 위가 반박하고 있었다. 이 문서를 읽는 가드가 없어서 16일 서 있었다.
> 지금은 있다 — 그래서 이 정정 기록도 옛 문장을 **인용하지 않고 서술한다**. 정정을 적는 일이
> 가드를 트립시키면 안 되기 때문이다.

**그다음 CI가 부르는 것을 그대로 부르고, 종료 코드로 판단한다**(명령 셋은 아래 8번에 있다).

### 다른 머신에서 정당하게 달라지는 것 — 고장으로 읽지 말 것

1. **모듈 바이트 크기 핀은 제3의 머신에서 반드시 실패한다. 의도된 것이다.**
   `hosts/rust-oracle/tests/protection.rs`가 개발 머신 **9,728 B**, GitHub `windows-latest`
   **9,216 B**로 고정하고 `GITHUB_ACTIONS`로 둘을 가른다. 다른 값은 assert에 걸린다. 모듈의 결함이
   아니다 — 툴체인 핀은 **rustc**를 덮지 MSVC `link.exe`나 Windows SDK를 덮지 않으므로, SDK가
   다르면 크기가 다르다. 실패 메시지가 정확히 이 말과 할 일을 적는다: 다시 재고, **상수와 그 값을
   싣는 문서 다섯**(`README.md`·`README.ko.md`·`docs/SECURITY.md`·`docs/STATUS.md`, 그리고
   **이 파일**)을 **같은 커밋에서** 고친다. `doc_claims.rs`가 다섯을 전부 검사하므로 반만 고치면
   계속 빨갛다. **이 파일은 2026-09-22에 그 목록에 들어왔다** — 그때까지 두 값을 네 번 싣고 있으면서
   "문서 넷이 가드된다"고 적고 있었다.

   그래도 목록을 믿지 말고 grep하라: 가드의 범위는 **손으로 적은 배열**이라 여섯 번째 문서가
   아무 소리 없이 들어올 수 있다. 여기 모든 손-범위 가드의 공통 한계이고, 위 8번의 게이트 목록을
   `ci.yml`에서 **유도**하는 이유다.
2. **`host.c`를 비롯한 손으로 쓴 C는 순수 ASCII여야 하고, 그 이유가 머신 의존이다.** MSVC는 이
   파일들을 머신의 ANSI 코드 페이지로 읽으므로, 비ASCII 바이트는 `/W4 /WX`에서 `C4819` → `C2220`이
   된다 — **다만 그 코드 페이지가 표현하지 못할 때만.** CP949 머신에서 발견됐고 CP1252였다면 깨끗하게
   컴파일됐을 것이다. 지금은 텍스트만 보는 테스트가 막고, 두 CI 잡 모두에서 돌므로 누구도 특정
   코드 페이지를 가질 필요가 없다.
3. **`mlc`가 크래시하면 `%TEMP%\mlc-build-*`가 남는다.** 정상 종료는 정리하지만 중단된 프로세스는
   할 수 없다(예: 일부러 스택을 넘기는 프로브). 무해하며, `docs/STATUS.md` §5-5가 "고쳤다"가 아니라
   **열어 둔 채로** 유지한다.

### 어디서 시작하나

`docs/STATUS.md` §9의 **"▶ 여기서 시작한다"** 블록. 그것이 인수인계다 — 무엇이 닫혔고, 무엇을
의도적으로 남겼고, 무엇을 다시 열지 말아야 하는지. 백로그 표보다 먼저 읽는다(표가 더 빨리 낡는다).

## 워크플로 (한국어)

1. **`main`에 직접 커밋 금지.** 모든 변경은 Pull Request로만 반영한다.
2. `main`에서 분기하고, 접두어로 종류를 표시한다:
   - `docs/*` — 문서 / 설계 문서
   - `feat/*` — 신규 구현
   - `fix/*` — 버그 수정
   - `chore/*` — 도구·메타·정리
3. PR은 한 가지 관심사로 좁힌다. 관련 결정/질문 번호(예: `D16`, `Q12`)를 명시한다.
4. `docs/DECISIONS.md`의 **결정을 뒤집으려면**, 먼저 그 파일에 근거와 트레이드오프를 쓴다
   (자세한 규칙은 [CLAUDE.md](CLAUDE.md)).
5. **근거 수준**은 단계에 맞춘다 (CLAUDE.md 규칙):
   - Phase 0(문서) → **E0**(문서 정합) / **E1**(출처 있는 외부 사실). 없는 측정값 날조 금지.
   - 코드가 생기면 → "완료" 선언 전 **E2**(실제 빌드/실행 산출물) 필수.
6. **Grok 2차 검증**은 구현·진단·코드 검토에서 PR 완료 전 필수. Grok 도구가 실패하면
   보고하고 판단을 구한다 — 임의 대체 금지.
7. `main`에는 squash-merge. 머지 후 브랜치 삭제.
8. **CI**(GitHub Actions)가 모든 PR에서 `cargo fmt --check` + `clippy -D warnings` +
   `cargo test --workspace`를 실행한다. **`windows-latest`가 정본 게이트** — `#![cfg(windows)]`
   수용 테스트를 실제로 돌리며, `MATHLESS_GATE_D=require`로 수용 D(실제 C 호스트)가 조용히
   skip되지 않게 한다. **`ubuntu-latest`는 프런트엔드 보험**(Windows 전용 가정 조기 발견)이지
   권위가 아니다 — 수용 테스트는 거기서 컴파일되지 않는다. **분할 수치는 여기 적지 않는다**:
   한때 "88개 실행 / 19개 제외"라고 적어 뒀다가 그대로 낡았다(그 합 107은 오늘 수치가 아니다).
   테스트 수의 정본은 `docs/STATUS.md` §1이다. 머지 전 둘 다 green 유지.
   (`.so`/ELF **타깃**은 여전히 D22와 함께 이연 — Linux 잡은 D22가 아니다.)

   **CI가 부르는 방식 그대로 부른다는 것은 게이트 셋을 전부 건다는 뜻이다**(위 영문 블록):

   ```powershell
   # PowerShell — Windows 기본 셸이고, `VAR=value cmd` 접두어가 **없다**
   $env:MATHLESS_GATE_D = "require"
   $env:MATHLESS_GATE_FPC = "require"
   $env:MATHLESS_GATE_FPC_HOST = "require"
   cargo test --workspace --locked
   ```

   ```sh
   # Git Bash / WSL
   MATHLESS_GATE_D=require MATHLESS_GATE_FPC=require MATHLESS_GATE_FPC_HOST=require \
     cargo test --workspace --locked
   ```

   **PowerShell 형태를 먼저 두는 이유**: bash 접두어는 PowerShell에서 **변수를 설정하지 않고**
   `cmd`에서는 파스 에러다. 그러면 자연스러운 수리가 "접두어를 떼는 것"이 되고, 그것이 바로 이
   변경이 막으려는 **게이트 하나짜리 실행**이다. 아래 파이프 경고와 같은 모양 — 틀린 형태도 exit 0이다.

   **하나만 걸면 안 된다.** 2026-09-22까지 위 블록은 `MATHLESS_GATE_D` 하나만 걸었고, Free Pascal이
   없는 머신에서는 **나머지 둘이 조용히 skip**되어 초록·exit 0이 나온다. 기여자는 그것을 믿고
   올리고 CI가 빨개진다. **skip된 게이트는 stdout에서 시끄럽고 종료 코드에서는 조용하다** —
   바로 아래 문단이 파이프에 대해 경고하는 그 실패다. `MATHLESS_GATE_DELPHI`는 **일부러 뺀다**:
   CI가 그것을 require로 걸지 않는다(러너에 Delphi가 없다).

   **이 명령들은 CI가 부르는 방식 그대로 부르고, 종료 코드로 판단한다.** 출력을 줄이려고
   파이프로 감싸지 않는다. `cargo fmt --all --check | tail -1 && echo clean`은 **파이프라인의**
   종료 코드(= 항상 0인 `tail`의 것)를 보므로, 포맷이 깨진 트리에서도 `clean`을 찍는다 —
   바로 그 래퍼가 잘못 포맷된 커밋을 로컬 검사에서 통과시켜 CI가 잡았다(`docs/STATUS.md` §7-1).
   **실패할 수 없는 가드는 아무것도 증명하지 않는다.**

## 라이선스와 기여

이 저장소는 **Apache-2.0 또는 MIT** 이중 라이선스다(`LICENSE-APACHE` / `LICENSE-MIT`).
명시적으로 달리 밝히지 않는 한, 제출된 기여는 Apache-2.0이 정의하는 바에 따라 위와 동일하게
이중 라이선스된다.

## 대외 표현 규칙

보호를 "리버싱 불가능"으로 표현하지 않는다. 정직한 문구는 **"분석과 변조 비용을 높인다"**
이다 (`docs/SECURITY.md`, 결정 D05 참고).

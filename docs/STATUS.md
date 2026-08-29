# STATUS — 세션 핸드오프 (현재 상태 · 잔여 작업)

> **스냅샷: 2026-08-29.** 다음 세션은 이 문서를 먼저 읽는다. 측정값은 그 시점 `main` 기준이며,
> `git log`·`docs/phase1/*`(phase 계획)·`docs/slices/*`(기능 SPEC)이 정본이다. 이 문서가 오래되면 갱신하거나 폐기한다.

## 1. 현재 상태 (실측, `main`)

- **테스트:** `cargo test --workspace` = **101 pass / 0 fail**. `clippy -D warnings` clean, `fmt` clean.
- **CI:** GitHub Actions `windows-latest`, 툴체인 핀 `rust-toolchain.toml` = 1.97.1.
- **코드:** ~3,308 LOC Rust. `src`에 TODO/FIXME 없음.
- **언어(surface):** 타입 `f64` / `bool` / `i32`; `if`(else 없음)/`return`; **실패 가능 함수**(`-> T!`,
  `error NAME=N` + `fail NAME`, i32 status + out-param, D17/Q13); **지역 변수**(`let` 불변 / `let mut` +
  대입, 블록 스코프).
- **파이프라인:** `mlc` = lex → parse → typecheck → 백엔드 독립 IR → codegen(IR → `no_std`
  `extern "C"` Rust → `cargo` cdylib). **CLI** `mlc build <f.mls> -o <dir>` → `.dll`+`.h`+`.pas`.
  **오라클**(Rust kernel32)이 산출 모듈을 로드·호출.
- **수용:** A(컴파일)·B(오라클 로드/호출)·C(export/strip 프록시) 통과. **D(실제 Delphi/C 호스트
  로드)는 BLOCKED** — 빌드 머신에 `cl`/`gcc`/`dcc64` 없음.
- **문서/메타:** README EN/KO + GitHub About 갱신. Q13 닫힘(`OPEN_QUESTIONS` #28), D17 상세
  `DECISIONS`(#30) 반영.

## 2. 이번 세션 요약

- **머지: PR #11~#31 (21건).** 하드닝(예약어/`out_value`/중복 함수·파라미터/비유한 리터럴/abi 단일소스/
  all-paths-return을 typeck로/parser peek 경계/PE 음성 테스트/workdir 격리), CI + 툴체인 핀, `mlc build`
  CLI, D17 에러-경로 슬라이스, `let` 지역 변수, `i32` 타입, README/About.
- **열린 PR: #32** — `let mut`(가변 지역 변수) + 대입 슬라이스 **SPEC 초안, 설계 확인 대기**.
- 방식: 각 슬라이스 SDD(SPEC→사용자 확인)·TDD(Red→Green)·Grok(plan+verify)·CI green·PR.

## 2b. 다음 세션(2026-08-29) 진행분

- **3b-#1 문서 정합화 완료 (PR #34)** — 문서 전용(코드 변경 0). 실측 재확인: `cargo test --workspace`
  **67 pass / 0 fail**, `mlc build examples/discount.mls` → `discount.dll` **9,728 B** + `.h` + `.pas`,
  `cl`/`gcc`/`dcc64` **여전히 없음**(수용 D BLOCKED 유지).
  Grok verify가 **실제 과대 주장 1건**을 잡음: "ABI major 불일치 시 로드 거부"는 구현되어 있지 않다
  (`Module::load`는 `LoadLibraryW`만, 오라클은 로드 **후** 값 일치를 assert). 리포 전체 4곳을 "호스트
  계약 / 여기서는 미강제"로 수정.
- **3b-#2 W1 fixture 제거 완료 (PR #35)** — 테스트 67 → 66.
- **LICENSE (PR #38)** — 처음 MIT로 정한 뒤 판단 요청을 받아 **Apache-2.0 OR MIT 이중**으로 확장
  (Rust 관례, MIT의 상위집합). 파생 질문 **Q15**(생성 산출물의 라이선스 지위) 등록.
- **Gate-D 툴체인 (PR #37)** — **MSVC Build Tools 확정**(설치 대기, 수용 D는 계속 BLOCKED).
- **`let mut` 슬라이스 (SPEC #32 / 구현 #39)** — 가변 지역 변수 + 대입문. 테스트 66 → 86.
- **SPEC 재배치 (PR #40)** — 기능 SPEC은 `docs/slices/`(색인 포함), `docs/phaseN/`은 phase 계획만.
- **3b-#5 진단 (PR #41)** — `CompileError: Display` + `IrType: Display`. 테스트 86 → 94.
- **3b-#4 emit 견고성 (PR #42)** — 스테이지→이동(+롤백), 모듈명 검증. 테스트 94 → 101.
  `discount3.dll` = **9,728 B**로 스칼라 `discount.dll`과 동일 — 가변 지역 변수는 ABI·크기에 영향 없음.

## 3. 잔여 작업 — 다음 세션 착수

### 3a. 즉시 재개 (확인/블록 대기)
- ~~**PR #32 `let mut`**~~ — **완료.** DP-M1~M4 사용자 승인(2026-08-29) → SPEC 머지(#32) →
  WM1~WM5 TDD 구현(#39). 다음 후보는 `while`(이 대입을 그대로 재사용)이나 복합 대입이지만,
  **STATUS 3b의 잔여(#4/#5)가 먼저다.**

### 3b. Grok 실측 검토 — fix/improve 우선순위 (다음 세션 권장, 슬라이스와 별개)
> **#1 완료(2026-08-29).** 21 PR 후 문서가 `main`보다 뒤처져 있었고(“awaiting confirmation” 배너,
> README 58 → 실제 67, `LANGUAGE.md` MVP가 while/for/string을 미래로 나열, `OPEN_QUESTIONS`의 Q13
> “DECISIONS pending” 잔여) 다음 SDD 사이클이 닫힌 작업을 재논의할 위험이 있었다 → 한 문서 PR로 해소.
> **다음 우선순위는 #2(fixture 제거).**

1. ~~**문서 정합화 (최우선)**~~ — **완료(문서 정합 PR).** 출시된 SPEC 4종을 `상태: 확정 · 구현 완료`로
   전환(승인·구현 PR 번호 명기), W7 이후 작업(STEP1/W8~W10 + 하드닝 9건)을 WBS에 기록, `LANGUAGE.md`를
   “MVP 목표(제안)” vs “현재 구현된 표면(E2)”으로 분리, README EN/KO 테스트 수 58→67 및 `i32` 반영,
   `OPEN_QUESTIONS` Q13 잔여 문구 정리, `ARCHITECTURE`(실제 레이아웃)·`HOST_ABI`(현재 구현된 경계)·
   `SECURITY`(P0 실측 프록시) 보강. **`DECISIONS.md`는 건드리지 않음**(규칙 8).
2. ~~**`examples/fixture` + `loads_fixture` 제거**~~ — **완료.** 크레이트·테스트·워크스페이스 멤버·
   `Cargo.lock` 항목 삭제. `end_to_end`가 동일한 §3-B 3개 assert를 **컴파일러 산출 DLL**로 이미 덮고
   있어 커버리지 손실 없음. 테스트 **67 → 66**(삭제한 그 1개만 감소), fmt/clippy clean.
3. **skip-게이트 C(이후 Delphi) 호스트를 `hosts/`에 추가** — `cl`/`gcc`/`dcc64`가 있을 때만 컴파일.
   수용 D를 “됐다”고 위장하지 않으면서 Gate-D 준비를 이어감(CI green 유지, 툴체인 오는 날 즉시 검증).
4. ~~**`mlc build` 산출을 원자적으로** + 나쁜 모듈 stem 거부~~ — **완료(PR #42).** 세 산출물을 `out_dir`
   안 스테이지에 모두 만든 뒤 이동하고, 이동 실패 시 밀어냈던 기존 파일까지 되돌린다(강제 실패 테스트 2종).
   모듈명은 진입 즉시 검증 — 식별자 + `reserved.rs` 전 대상. `if.mls`/`my-mod.mls`는 이제 cargo가 아니라
   `mlc`가 파일명을 짚어 거부한다. **부수 발견:** 이름이 생성 `Cargo.toml`에 그대로 보간되므로 따옴표가
   섞인 stem은 TOML 문자열을 탈출할 수 있었다(로컬·파일명 통제 필요 — 이제 차단).
5. ~~**`CompileError`에 실제 `Display`**~~ — **완료.** `CompileError`가 `Display` + `std::error::Error`
   (`source()`)를 구현하고 실패한 단계에 위임한다. `EmitError::Compile`은 더 이상 `{e:?}`로 감싸지
   않는다. CLI 실측: `mlc: parse error at 1:13: expected parameter name, found keyword \`mut\` …`
   (이전에는 `mlc: compile error: Parse(ParseError { … })`). 실제 바이너리를 돌려 stderr를 단언하는
   테스트 포함. `IrType`에 `Display`를 붙여 진단이 `F64`가 아니라 표면 이름 `f64`를 인용한다.
   **남은 것:** `TypeError`에는 아직 line/col이 없다(AST에 span이 없음 — 별도 작업, ROADMAP Phase 3).
6. **`ubuntu-latest` 컴파일 전용 CI 잡** 추가(비-`cfg(windows)` 프런트엔드 `fmt`/`clippy`/`test`).
   lex/parse/typeck·임시경로 코드의 false-green 보험. **D22 아님**(`.so`/ELF 빌드·로드 없음).
7. **`grok_build_plan`을 착수 게이트에서 내림, `grok_build_verify`는 완료 게이트 유지.** 이번 세션에서
   plan 도구가 반복적으로 얇았음 — 다음 세션을 여기서 멈추지 말 것. verify 실패는 보고(임의 대체 금지).
8. ~~**새 언어 SPEC를 `docs/phase1/`에서 이동**~~ — **완료.** 기능 슬라이스 SPEC 4종을
   **`docs/slices/`**로 옮기고, `docs/phase1/`에는 phase(캠페인) 문서인 `SPEC.md` + `WBS.md`만 남겼다.
   Grok 교차검토도 `docs/slices/` 지지(`docs/lang/`은 다음 슬라이스들이 ABI 작업이라 즉시
   `docs/abi/`를 요구하고, phase 번호는 다시 개명해야 함). Grok의 반론("평평한 폴더는 결국 같은
   더미가 된다")에 따라 **`docs/slices/README.md` 색인**을 함께 둔다 — 슬라이스마다 갱신할 것.

### 3c. 사용자 결정 대기
- ~~**LICENSE**~~ — **Apache-2.0 OR MIT 이중 확정**(2026-08-29). 처음 MIT로 정한 뒤, 사용자 요청으로
  판단해 Rust 생태계 관례인 이중으로 확장(MIT의 상위집합 — 특허 허여 추가, GPLv2 호환 유지).
  `LICENSE-APACHE`/`LICENSE-MIT`, README EN/KO, CONTRIBUTING 반영 완료.
  - **남은 것:** 저작권자 표기는 계정명 `xzawed`다. 법인/실명이 필요하면 한 줄 PR.
  - **남은 것:** **Q15**(생성 산출물·`ml_abi.h`의 라이선스 지위) — `OPEN_QUESTIONS.md`에 기록됨.
- **홈페이지 URL**: About 텍스트는 갱신됨, URL만 미설정.
- ~~**수용 D 툴체인**~~ — **MSVC Build Tools 확정**(2026-08-29 사용자 결정). 근거: 이 머신의 rustc
  host triple이 이미 `x86_64-pc-windows-msvc`라 산출 cdylib가 MSVC CRT/링커 계열이고, C 호스트를
  같은 계열로 맞추면 CRT·링커 불일치 변수를 없앤 상태로 수용 D를 측정할 수 있다. **D22를 바꾸지
  않는다** — D22는 모듈 *포맷* 결정에 "msvc"를 못박지 않는다는 뜻이고, 이것은 **Gate-D 검증용
  호스트 툴체인** 선택이다. 설치 전까지는 3b-#3(skip-게이트 호스트)만 진행. 설치되면: 실제 C 호스트
  로드 테스트 + `dumpbin /exports` 교차 확인(현재 export 측정은 자체 PE 리더 단독).
- **D22(SO/ELF) 개시 여부.** (새 SPEC 위치 3b-#8은 `docs/slices/`로 해소됨.)

### 3d. 하지 말 것 (Grok)
- 위 문서 정합(#1)과 emit 버그 2건(#4)이 닫히기 전에는 **D16 / 문자열·구조체 / 콜백 / 두 번째 호스트**를
  시작하지 않는다.
- **`packager/`·빈 `backend/`·`host/delphi` 크레이트 생성 금지** — D18은 아직 평범한 DLL + export 심볼,
  지금 크레이트는 조기 경계.
- **D22(`.so`/ELF) 미개시** — 명시적 “D22 개시” 결정 필요. Linux 컴파일 전용 잡은 D22가 아니다.

## 4. 다음 세션 재개 방법

1. 이 문서 → `README.md`(문서 지도) → `docs/phase1/WBS.md`(phase 계획) → `docs/slices/README.md`(기능 슬라이스 색인) 순.
2. 규칙: `CLAUDE.md`·`CONTRIBUTING.md`. 절차 = **SPEC → (사용자 확인) → TDD(Red→Green) → Grok verify → PR → squash-merge**.
3. `main` 직접 커밋 금지. 각 변경은 CI(`windows-latest`, 툴체인 핀) green + Grok verify 후 머지.
4. 권장 다음 순서: **3b-#1 문서 정합 ✅ → 3b-#2 fixture 제거 ✅ → `let mut` 슬라이스 ✅ → 3b-#4/#5 emit·진단 → 3b-#3 skip-게이트 C 호스트 → 이후 슬라이스(`while` 등).**

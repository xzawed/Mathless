# STATUS — 세션 핸드오프 (현재 상태 · 잔여 작업)

> **스냅샷: 2026-08-29.** 다음 세션은 이 문서를 먼저 읽는다. 측정값은 그 시점 `main` 기준이며,
> `git log`·`docs/phase1/*`·각 SPEC이 정본이다. 이 문서가 오래되면 갱신하거나 폐기한다.

## 1. 현재 상태 (실측, `main`)

- **테스트:** `cargo test --workspace` = **67 pass / 0 fail**. `clippy -D warnings` clean, `fmt` clean.
- **CI:** GitHub Actions `windows-latest`, 툴체인 핀 `rust-toolchain.toml` = 1.97.1.
- **코드:** ~3,308 LOC Rust. `src`에 TODO/FIXME 없음.
- **언어(surface):** 타입 `f64` / `bool` / `i32`; `if`(else 없음)/`return`; **실패 가능 함수**(`-> T!`,
  `error NAME=N` + `fail NAME`, i32 status + out-param, D17/Q13); **불변 지역 변수**(`let`, 블록 스코프).
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

## 3. 잔여 작업 — 다음 세션 착수

### 3a. 즉시 재개 (확인/블록 대기)
- **PR #32 `let mut`**: 사용자가 DP-M1~M4 확인 → WM1(렉서 `mut`)부터 TDD. (SPEC: `docs/phase1/SPEC-let-mut.md`)

### 3b. Grok 실측 검토 — fix/improve 우선순위 (다음 세션 권장, 슬라이스와 별개)
> **#1이 최우선.** 21 PR 후 문서가 `main`보다 뒤처져 있어(“awaiting confirmation” 배너, README 58 →
> 실제 67, `LANGUAGE.md` MVP가 아직 while/for/string을 미래로 나열, `OPEN_QUESTIONS`의 Q13
> “DECISIONS pending” 잔여) 다음 SDD 사이클이 닫힌 작업을 재논의할 위험 → **한 문서 PR로 해소.**

1. **문서 정합화 (최우선):** SPEC/WBS/`LANGUAGE.md`/README/`OPEN_QUESTIONS`를 실측 `main`에 맞춘다.
   출시된 SPEC를 “accepted”로, W7 이후 작업을 WBS에 기록, “MVP 제안” vs “구현됨” 분리, 테스트 수 58→67,
   Q13 잔여 문구 정리.
2. **`examples/fixture` + `loads_fixture` 제거**(워크스페이스에서도). `mlc build`가 W5 경로를 대체 —
   손수 만든 fixture는 두 번째 ABI 소스이고 매 `cargo test`가 빌드한다.
3. **skip-게이트 C(이후 Delphi) 호스트를 `hosts/`에 추가** — `cl`/`gcc`/`dcc64`가 있을 때만 컴파일.
   수용 D를 “됐다”고 위장하지 않으면서 Gate-D 준비를 이어감(CI green 유지, 툴체인 오는 날 즉시 검증).
4. **`mlc build` 산출을 원자적으로**(stage → rename) + `reserved.rs`에 나쁜 모듈 stem 거부. WBS 잔여에
   기록됨: `.h`/`.pas` 기록 실패가 `.dll`만 남길 수 있고, `if.mls`/`my-mod.mls`가 typeck 아닌 cargo에서 죽음.
5. **`CompileError`에 실제 `Display`** + CLI stderr가 `Debug`가 아니라 소스-위치 메시지임을 단언
   (`emit.rs`가 이미 gap 표기). `let`/`T!`/`i32`가 늘수록 진단이 썩는다(ROADMAP Phase 3, 스텁은 지금).
6. **`ubuntu-latest` 컴파일 전용 CI 잡** 추가(비-`cfg(windows)` 프런트엔드 `fmt`/`clippy`/`test`).
   lex/parse/typeck·임시경로 코드의 false-green 보험. **D22 아님**(`.so`/ELF 빌드·로드 없음).
7. **`grok_build_plan`을 착수 게이트에서 내림, `grok_build_verify`는 완료 게이트 유지.** 이번 세션에서
   plan 도구가 반복적으로 얇았음 — 다음 세션을 여기서 멈추지 말 것. verify 실패는 보고(임의 대체 금지).
8. **새 언어 SPEC를 `docs/phase1/`에서 이동**(phase1.5 / `docs/lang/` / Phase 2). Phase 1 A/B/C는 끝났고
   `#25`/`#29`/`#32`를 여기 쌓으면 “Phase 1 완료(단 D)” 가독성이 나빠짐. **← 위치는 사용자 결정 필요.**

### 3c. 사용자 결정 대기
- **LICENSE**: MIT / Apache-2.0 / 둘 다 / 독점 — 정하면 5분 PR.
- **홈페이지 URL**: About 텍스트는 갱신됨, URL만 미설정.
- **수용 D 툴체인**: MSVC Build Tools vs MinGW/LLVM vs `dcc64`. 그 전까지는 3b-#3(skip-게이트 호스트)만.
- **새 SPEC 위치**(3b-#8), **D22(SO/ELF) 개시 여부**.

### 3d. 하지 말 것 (Grok)
- 위 문서 정합(#1)과 emit 버그 2건(#4)이 닫히기 전에는 **D16 / 문자열·구조체 / 콜백 / 두 번째 호스트**를
  시작하지 않는다.
- **`packager/`·빈 `backend/`·`host/delphi` 크레이트 생성 금지** — D18은 아직 평범한 DLL + export 심볼,
  지금 크레이트는 조기 경계.
- **D22(`.so`/ELF) 미개시** — 명시적 “D22 개시” 결정 필요. Linux 컴파일 전용 잡은 D22가 아니다.

## 4. 다음 세션 재개 방법

1. 이 문서 → `README.md`(문서 지도) → `docs/phase1/WBS.md` → 열린 SPEC(`SPEC-let-mut.md` 등) 순.
2. 규칙: `CLAUDE.md`·`CONTRIBUTING.md`. 절차 = **SPEC → (사용자 확인) → TDD(Red→Green) → Grok verify → PR → squash-merge**.
3. `main` 직접 커밋 금지. 각 변경은 CI(`windows-latest`, 툴체인 핀) green + Grok verify 후 머지.
4. 권장 다음 순서: **PR #32 확인 → (또는 먼저) 3b-#1 문서 정합 → 3b-#2 fixture 제거 → 3b-#4/#5 emit·진단 → 이후 슬라이스.**

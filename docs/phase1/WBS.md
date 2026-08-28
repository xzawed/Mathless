# Phase 1 WBS — Work Breakdown Structure

의존 순서. **각 작업 = 1 PR**, 측정 가능한 완료 기준(DoD). DP1~DP4 확인 후 W0부터 착수.
방법론은 SDD+WBS+TDD (CLAUDE.md). 각 코드 작업은 실패 테스트 → 구현 → 통과 → `grok_build_verify`.

> 진행(2026-08-29 실측): **W0~W7 ✅** · **STEP1 CLI ✅** · **W8~W10 ✅**(D17 에러 경로 · `let` 지역 변수 · `i32`).
> **Phase 1 빌드 가능 범위(수용 A/B/C + W7 산출물) 완료**; **수용 D만 툴체인 확보 대기(BLOCKED)**.
> `cargo test --workspace` = **66 pass / 0 fail**, `clippy -D warnings`·`fmt` clean, CI `windows-latest`(툴체인 핀 1.97.1).
> 잔여 작업 목록의 정본은 `docs/STATUS.md` §3.
>
> **STEP1(Gate-D prep) ✅**: `mlc build <f.mls> -o <dir>` CLI가 `.dll`+`.h`+`.pas` 3종을 디스크로 산출한다(라이브러리 `emit::emit_artifacts`, bin은 argv만). 실측 E2(STEP1 당시): `cargo test` 30 그린(현재 66), 오라클이 **산출 dll**을 로드해 `mlx_discount(100,true)=90`/`abi_version=1`·export 2개 통과, 실 CLI 실행이 `discount.dll(9,728 B)`+`.h`+`.pas` 생성. `.h`/`.pas`의 실제 C/Delphi 로드는 여전히 **BLOCKED**(생성물에 DRAFT 표기 유지).
>
> 수용 A+B 실측: `discount.mls` → 컴파일러 → `discount.dll` → 오라클 로드 → `discount(100,true)=90`·`(100,false)=100`·`abi_version=1`. codegen은 "모든 경로 return"을 강제(미충족 시 codegen 에러).
> 수용 C 실측: no_std+strip+lto+opt-z DLL = **9,728 B**(std ~107,008 B 대비 ~11×↓), export = **정확히 `mlx_discount` + `ml_module_abi_version`**(PE 리더로 파싱), 소스 코멘트/파일명 비유출. 프록시만 측정 — "리버싱 난이도" 주장 없음(D05).

| ID | 작업 | 산출물 | 완료 기준 (측정) | 의존 |
|----|------|--------|------------------|------|
| **W0** | DP1~DP4 확인 → `DECISIONS.md` D19~D22 반영 + export 측정 도구 확보 | DECISIONS 갱신, export 덤프 방법(dumpbin 또는 llvm-objdump/PE 리더) | `discount.dll`의 export 목록을 실제로 출력해 첨부 | SPEC 확인 |
| **W1** | 리포 스캐폴드 | `compiler/`(Rust), `runtime/`(예약 심볼 규약 문서+헤더), `hosts/rust-oracle/`(kernel32 로더), `examples/discount.mls` | `cargo test` 그린: 오라클이 **손수 만든 fixture DLL**을 로드해 §3-B 3개 assert 통과 *(그 fixture 크레이트·테스트는 W5가 대체한 뒤 삭제됨 — 아래 하드닝 표)* | W0 |
| **W2** | 렉서·파서 (TDD) | `compiler`의 lexer/parser | `discount.mls` → AST 스냅샷 테스트 통과; 잘못된 입력은 명확한 에러 | W1 |
| **W3** | 타입체크 + 독립 IR (TDD) | typecheck(f64/bool), 비-Rust IR | AST → typed IR 테스트 통과; 타입 오류 케이스 실패 처리 | W2 |
| **W4** | 코드젠 (TDD) | IR → `no_std`+`extern "C"`+`repr(C)` Rust emit → `cargo cdylib` 호출 | 생성 Rust가 빌드되어 DLL 산출; 유닛 테스트 그린 | W3 |
| **W5** | 통합 (수용 A+B) | `mlc build examples/discount.mls` | 오라클이 **컴파일러 산출 DLL**(fixture 아님)로 §3-B 통과 | W4 |
| **W6** | 보호 측정 (수용 C) | strip/no_std 빌드 설정 | export 덤프 = `mlx_discount` + `ml_module_abi_version`만; 소스/디버그/패닉 문자열 최소임을 수치로 첨부 | W5 |
| **W7** | D14 산출물 | C 헤더(`.h`) + Delphi import unit(`.pas`) 생성기 | 헤더/유닛 생성 확인. **로드 게이트(수용 D)는 dcc64/cl/gcc 확보 후** — BLOCKED 표기 유지 | W5 |

## W7 이후 — 실제로 수행된 작업 (기록)

W0~W7은 원래 SPEC의 계획이었다. 아래는 그 뒤에 **별도 SPEC + 사용자 확인**을 거쳐 머지된 슬라이스다.
각 슬라이스의 설계 근거는 해당 SPEC 문서에 있다(전부 `상태: 확정 · 구현 완료`).

| ID | 작업 | SPEC | 완료 기준 (측정, E2) | PR |
|----|------|------|----------------------|----|
| **STEP1** | `mlc build <f.mls> -o <dir>` CLI — `.dll`+`.h`+`.pas`를 디스크로 산출 | (W7 연장) | 실 CLI 실행이 `discount.dll`(9,728 B)+`.h`+`.pas` 생성, 오라클이 그 산출 dll을 로드 | #11 |
| **W8** | **D17 에러 경로** — 실패 가능 함수 `-> T!`, `error NAME = N`, `fail NAME` → i32 status + out-param | `SPEC-D17-error-abi.md` | 오라클이 성공(`status=0`, out 기록)·실패(`status=1`, out 미변경) **두 경로** assert; export 집합 불변 | #14(SPEC) / #15 |
| **W9** | **지역 변수 `let`** — 블록 스코프·불변·타입 추론 | `SPEC-let-locals.md` | 재선언/섀도잉/미정의/예약어/`out_value` 부정 케이스 전부 거부; 오라클 로드·호출 통과; **ABI 불변** | #25(SPEC) / #26 |
| **W10** | **`i32` 타입** — 정수 리터럴·산술·비교, `int32_t`/`Integer` 매핑 | `SPEC-i32.md` | 혼합 연산·`i32 /` 거부; 오라클 로드·호출 통과; ABI 불변 | #29(SPEC) / #31 |

### 하드닝·인프라 (슬라이스 아님, 각 1 PR)

| 내용 | PR |
|------|----|
| 파라미터명이 대상 언어(Rust/C/Pascal) 예약어면 typecheck에서 거부 | #10 |
| 함수명 중복 거부(대소문자 무시) · 파라미터명 중복 거부(대소문자 무시) | #16 / #22 |
| f64 무한대로 넘치는 숫자 리터럴 거부 | #17 |
| 모듈 ABI 버전 단일 소스화 | #18 |
| 테스트 빌드 workdir을 프로세스별로 격리 | #19 |
| "모든 경로 return"을 codegen이 아니라 **typeck**에서 강제 | #20 |
| 파서 `peek()`/`err()` 경계 안전 | #21 |
| PE 리더의 손상 입력 가드에 부정 테스트 추가 | #23 |
| CI(`windows-latest`, fmt+clippy+test) · Rust 툴체인 1.97.1 핀 | #12 / #24 |
| W1 fixture 크레이트(`examples/fixture`) + `loads_fixture` 테스트 삭제 — W5의 컴파일러 산출물이 대체(두 번째 ABI 소스 제거, 매 `cargo test`의 불필요한 cdylib 빌드 제거). 테스트 67→66 | #35 |

## 게이트/블로커

- **BLOCKED (툴체인):** 수용 D(Delphi/C 호스트 실제 로드)는 이 머신에 `dcc64`/`cl`/`gcc`가 없어 실행 불가. W7은 **산출물 생성**까지, 실제 로드 검증은 별도.
  - 해소안 — **결정됨(2026-08-29): (b) MSVC Build Tools.** 기각: (a) Delphi/BDS CLI(`dcc64`) — 플래그십 호스트지만 설치 비용이 크고 C ABI 기준 검증이 먼저다, (c) MinGW/LLVM — rustc host triple이 `x86_64-pc-windows-msvc`라 CRT/링커 계열이 갈린다. **이 선택은 D22를 바꾸지 않는다**(D22 = 모듈 포맷에 "msvc"를 못박지 않음 / 이것 = Gate-D 검증용 호스트 툴체인).
  - 설치 후 할 일: `hosts/`에 실제 C 호스트를 두고 산출 `.h`+`.dll`로 로드·호출(수용 D), `dumpbin /exports`로 자체 PE 리더 측정치를 교차 확인. 설치 전까지는 skip-게이트(툴체인 있을 때만 컴파일)로 준비만 한다.
- W6 export 측정 도구가 없으면(dumpbin 미설치) llvm-objdump 또는 Rust PE 리더로 대체 — W0에서 확정.
- **W4 필수(Grok 지적) — 처리됨(강화):** 초기에는 codegen이 마지막 문이 `return`이 아니면 거부했으나(`block_always_returns`), 이후 **typeck로 올려** "모든 경로 return"을 프런트엔드에서 강제한다(PR #20) — 진단이 소스에 가까워짐.
- **크로스 타깃 식별자 하드닝 — 처리됨:** 파라미터명이 대상 언어(Rust/C/Pascal) 예약어와 겹치면 typecheck가 명확한 에러로 거부한다(`compiler/reserved.rs`; 프런트엔드 단일 검사 → 모든 백엔드 보호, Pascal은 대소문자 무시). 함수명은 `mlx_` 접두어라 안전.
- **잔여 하드닝(버그 아님):**
  - `mlc build`(`emit_artifacts`)는 호출마다 고유 임시 빌드 트리(`mlc-build-<pid>-<seq>`)를 쓰고 성공·실패와 무관하게 정리한다 → CLI 경로 경합/임시폴더 누수 해소(STEP1, Grok verify 반영). `codegen::build_cdylib`의 직접 소비자(테스트 `end_to_end`/`protection`)는 고정 `workdir`명 유지(직렬 실행이라 미실현 경합).
  - `mlc build` 산출은 원자적이지 않다: `.dll` 기록 후 `.h`/`.pas` 기록이 실패하면 부분 산출 가능(후속: staging→rename).
  - 모듈명은 입력 파일 stem에서 유도하며 식별자 검증이 없다: 예약어/하이픈/선행숫자 stem은 cargo/Delphi **빌드 에러**로 표면화(무음 오작동 아님). 후속: `reserved.rs` 규칙을 모듈명에도 확장.
  - ~~후속 변수(local) 도입 시 예약어 검사를 변수명에도 확장.~~ → **처리됨**: `let` 슬라이스(PR #26)가 지역 변수명에도 `reserved.rs`와 `out_value` 검사를 적용한다.

## 범위 밖 (후속 슬라이스, 별도 SPEC)

- ~~D17 정수 status + out-param 에러 경로~~ → **완료**(W8, PR #15)
- 가변 지역 변수 `let mut` + 대입 — **SPEC 초안 열림**(`SPEC-let-mut.md`, PR #32, 사용자 확인 대기)
- D16 caller-allocates 반환 / context handle 상태
- 문자열/구조체 마샬링, 콜백
- 두 번째 호스트(C#) — ROADMAP Phase 4

> 다음에 무엇을 할지는 **`docs/STATUS.md` §3**이 정본이다(문서 정합 ✅ → fixture 제거 ✅ →
> emit·진단 → 이후 슬라이스). STATUS §3d의 "하지 말 것"(닫히기 전 D16/문자열/콜백/두 번째 호스트 착수 금지,
> `packager/`·빈 `backend/` 크레이트 생성 금지, D22 미개시)도 이 WBS에 그대로 적용된다.

# Phase 1 WBS — Work Breakdown Structure

의존 순서. **각 작업 = 1 PR**, 측정 가능한 완료 기준(DoD). DP1~DP4 확인 후 W0부터 착수.
방법론은 SDD+WBS+TDD (CLAUDE.md). 각 코드 작업은 실패 테스트 → 구현 → 통과 → `grok_build_verify`.

> 진행(2026-08-29 실측): **W0~W7 ✅** · **STEP1 CLI ✅** · **W8~W10 ✅**(D17 에러 경로 · `let` 지역 변수 · `i32`).
> **수용 A/B/C 완료 + D는 C 호스트로 통과**(2026-08-29). **Delphi(`dcc64`)만 미검증**.
> 테스트 수·CI 구성의 정본은 `docs/STATUS.md`다(여기 숫자를 복제하면 곧 낡는다 — 실제로 두 번 낡았다).
> 잔여 작업 목록의 정본은 `docs/STATUS.md` §3.
>
> **STEP1(Gate-D prep) ✅**: `mlc build <f.mls> -o <dir>` CLI가 `.dll`+`.h`+`.pas` 3종을 디스크로 산출한다(라이브러리 `emit::emit_artifacts`, bin은 argv만). 실측 E2(STEP1 당시): `cargo test` 30 그린(현재 값은 `docs/STATUS.md`), 오라클이 **산출 dll**을 로드해 `mlx_discount(100,true)=90`/`abi_version=1`·export 2개 통과, 실 CLI 실행이 `discount.dll(9,728 B)`+`.h`+`.pas` 생성. (당시) `.h`/`.pas`의 실제 로드는 미검증이었다 — **`.h`는 이후 W12에서 해소**, `.pas`는 여전히 DRAFT.
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
| **W7** | D14 산출물 | C 헤더(`.h`) + Delphi import unit(`.pas`) 생성기 | 헤더/유닛 생성 확인. 실제 로드 게이트는 별도 — **`.h`는 W12에서 통과**, `.pas`는 미검증 | W5 |

## W7 이후 — 실제로 수행된 작업 (기록)

W0~W7은 원래 SPEC의 계획이었다. 아래는 그 뒤에 **별도 SPEC + 사용자 확인**을 거쳐 머지된 슬라이스다.
각 슬라이스의 설계 근거는 해당 SPEC 문서에 있다(전부 `상태: 확정 · 구현 완료`).

| ID | 작업 | SPEC | 완료 기준 (측정, E2) | PR |
|----|------|------|----------------------|----|
| **STEP1** | `mlc build <f.mls> -o <dir>` CLI — `.dll`+`.h`+`.pas`를 디스크로 산출 | (W7 연장) | 실 CLI 실행이 `discount.dll`(9,728 B)+`.h`+`.pas` 생성, 오라클이 그 산출 dll을 로드 | #11 |
| **W8** | **D17 에러 경로** — 실패 가능 함수 `-> T!`, `error NAME = N`, `fail NAME` → i32 status + out-param | [`SPEC-D17-error-abi.md`](../slices/SPEC-D17-error-abi.md) | 오라클이 성공(`status=0`, out 기록)·실패(`status=1`, out 미변경) **두 경로** assert; export 집합 불변 | #14(SPEC) / #15 |
| **W9** | **지역 변수 `let`** — 블록 스코프·불변·타입 추론 | [`SPEC-let-locals.md`](../slices/SPEC-let-locals.md) | 재선언/섀도잉/미정의/예약어/`out_value` 부정 케이스 전부 거부; 오라클 로드·호출 통과; **ABI 불변** | #25(SPEC) / #26 |
| **W10** | **`i32` 타입** — 정수 리터럴·산술·비교, `int32_t`/`Integer` 매핑 | [`SPEC-i32.md`](../slices/SPEC-i32.md) | 혼합 연산·`i32 /` 거부; 오라클 로드·호출 통과; ABI 불변 | #29(SPEC) / #31 |
| **W11** | **가변 지역 변수 `let mut` + 대입문** — `if`가 문이므로 분기 결과를 모으는 수단 | [`SPEC-let-mut.md`](../slices/SPEC-let-mut.md) | 불변 `let`/파라미터/미선언 대입·타입 불일치·대입으로 끝나는 블록·대입식 전부 거부; 오라클 로드·호출 통과; **ABI·크기 불변**(9,728 B 동일) | #32(SPEC) / #39 |
| **W14** | **단항 연산자 `-` · `!`** — 그전엔 음수를 쓸 수 없었다 | [`SPEC-unary.md`](../slices/SPEC-unary.md) | `return -5` 컴파일(이전 파스 에러); `-b`·`!x` 거부; `- -x`/`!!b` 허용; `-a * b` = `((-a) * b)`; 오라클과 **실제 C 호스트** 모두 통과; **`-i32::MIN == i32::MIN`(wrap) 측정**; export 불변 | #51(SPEC) / #53 |
| **W13** | **`while` 루프** — `let mut`가 먹여 살리려던 반복 | [`SPEC-while.md`](../slices/SPEC-while.md) | 조건 non-bool·`while`로 끝나는 블록·`while`을 식으로 쓰기·본문 지역 변수 누출 전부 거부; 오라클과 **실제 C 호스트** 모두에서 `sum_to(10)=55`/`(0)=0`; export 불변. **새 위험**(호스트 스레드 정지)은 `HOST_ABI.md`에 계약으로 명문화 | #48(SPEC) / #49 |
| **W12** | **수용 D — 실제 C 호스트** (`hosts/c-host/host.c`, MSVC `cl`) | (SPEC §3-D) | C11 호스트가 생성 `.h`를 컴파일(`/W4 /WX`)하고 LoadLibrary/GetProcAddress로 스칼라·에러 경로 8개 assert 통과; `dumpbin /exports`가 자체 PE 리더와 일치. **개발 머신 + GitHub `windows-latest` 러너 두 곳에서 통과.** MSVC 부재 시 `GATE_D_SKIPPED`, **CI는 `MATHLESS_GATE_D=require`로 실패 처리** | #43 |

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

- ~~**BLOCKED (툴체인)**~~ → **C 쪽 해소(2026-08-29).** 실제 원인은 툴체인 부재가 아니라 **PATH 미설정**이었다: MSVC Build Tools 2022가 이미 설치돼 있었고(`cl` 19.44 / `dumpbin` / `link`) `vcvars64.bat`으로 잡으면 동작한다. 수용 D는 C 호스트로 닫혔다(`hosts/c-host/host.c`, W12). **Delphi(`dcc64`)는 여전히 미확보** — D14의 플래그십이므로 호스트 서사는 절반이다.
  - 해소안 — **결정됨(2026-08-29): (b) MSVC Build Tools.** 기각: (a) Delphi/BDS CLI(`dcc64`) — 플래그십 호스트지만 설치 비용이 크고 C ABI 기준 검증이 먼저다, (c) MinGW/LLVM — rustc host triple이 `x86_64-pc-windows-msvc`라 CRT/링커 계열이 갈린다. **이 선택은 D22를 바꾸지 않는다**(D22 = 모듈 포맷에 "msvc"를 못박지 않음 / 이것 = Gate-D 검증용 호스트 툴체인).
  - 설치 후 할 일: `hosts/`에 실제 C 호스트를 두고 산출 `.h`+`.dll`로 로드·호출(수용 D), `dumpbin /exports`로 자체 PE 리더 측정치를 교차 확인. 설치 전까지는 skip-게이트(툴체인 있을 때만 컴파일)로 준비만 한다.
- W6 export 측정 도구가 없으면(dumpbin 미설치) llvm-objdump 또는 Rust PE 리더로 대체 — W0에서 확정.
- **W4 필수(Grok 지적) — 처리됨(강화):** 초기에는 codegen이 마지막 문이 `return`이 아니면 거부했으나(`block_always_returns`), 이후 **typeck로 올려** "모든 경로 return"을 프런트엔드에서 강제한다(PR #20) — 진단이 소스에 가까워짐.
- **크로스 타깃 식별자 하드닝 — 처리됨:** 파라미터명이 대상 언어(Rust/C/Pascal) 예약어와 겹치면 typecheck가 명확한 에러로 거부한다(`compiler/reserved.rs`; 프런트엔드 단일 검사 → 모든 백엔드 보호, Pascal은 대소문자 무시). 함수명은 `mlx_` 접두어라 안전.
- **잔여 하드닝(버그 아님):**
  - `mlc build`(`emit_artifacts`)는 호출마다 고유 임시 빌드 트리(`mlc-build-<pid>-<seq>`)를 쓰고 성공·실패와 무관하게 정리한다 → CLI 경로 경합/임시폴더 누수 해소(STEP1, Grok verify 반영). `codegen::build_cdylib`의 직접 소비자(테스트 `end_to_end`/`protection`)는 고정 `workdir`명 유지(직렬 실행이라 미실현 경합).
  - ~~`mlc build` 산출은 원자적이지 않다~~ → **처리됨**(#42): 세 산출물을 `out_dir` 안의 스테이지에 모두 만든 뒤 이동한다. 이동이 실패하면 이번 호출이 놓은 것을 걷어내고 밀어냈던 기존 파일을 되돌린다. 남는 실패 창은 이동 그 자체뿐이며, **롤백 도중 크래시**까지는 보장하지 않는다(저널이 필요한 범위 — 하지 않는다).
  - ~~모듈명에 식별자 검증이 없다~~ → **처리됨**(#42): `emit_artifacts`가 진입 즉시 모듈명을 검사한다 — 식별자 + `reserved.rs` 전 대상 + Windows 예약 장치명(`nul`/`con`/`com1`…). 이름은 생성 `Cargo.toml`의 `name = "…"`, C 헤더 가드, Delphi unit 이름, 그리고 **파일명**에 그대로 들어간다. 같은 검토에서 `reserved.rs`의 PASCAL 목록에 **`at`·`on` 누락**을 발견해 함께 채웠다 — 이는 파라미터·지역 변수명에도 영향이 있었다(E1: Delphi 문서상 예약어, dcc64 없어 컴파일 확인 불가).
  - ~~후속 변수(local) 도입 시 예약어 검사를 변수명에도 확장.~~ → **처리됨**: `let` 슬라이스(PR #26)가 지역 변수명에도 `reserved.rs`와 `out_value` 검사를 적용한다.

## 범위 밖 (후속 슬라이스, 별도 SPEC)

- ~~D17 정수 status + out-param 에러 경로~~ → **완료**(W8, PR #15)
- ~~가변 지역 변수 `let mut` + 대입~~ → **완료**(W11, SPEC #32 / 구현 #39)
- D16 caller-allocates 반환 / context handle 상태
- 문자열/구조체 마샬링, 콜백
- 두 번째 호스트(C#) — ROADMAP Phase 4

> 다음에 무엇을 할지는 **`docs/STATUS.md` §3**이 정본이다(문서 정합 ✅ → fixture 제거 ✅ →
> emit·진단 → 이후 슬라이스). STATUS §3d의 "하지 말 것"(닫히기 전 D16/문자열/콜백/두 번째 호스트 착수 금지,
> `packager/`·빈 `backend/` 크레이트 생성 금지, D22 미개시)도 이 WBS에 그대로 적용된다.

# Phase 1 WBS — Work Breakdown Structure

의존 순서. **각 작업 = 1 PR**, 측정 가능한 완료 기준(DoD). DP1~DP4 확인 후 W0부터 착수.
방법론은 SDD+WBS+TDD (CLAUDE.md). 각 코드 작업은 실패 테스트 → 구현 → 통과 → `grok_build_verify`.

| ID | 작업 | 산출물 | 완료 기준 (측정) | 의존 |
|----|------|--------|------------------|------|
| **W0** | DP1~DP4 확인 → `DECISIONS.md` D19~D22 반영 + export 측정 도구 확보 | DECISIONS 갱신, export 덤프 방법(dumpbin 또는 llvm-objdump/PE 리더) | `discount.dll`의 export 목록을 실제로 출력해 첨부 | SPEC 확인 |
| **W1** | 리포 스캐폴드 | `compiler/`(Rust), `runtime/`(예약 심볼 규약 문서+헤더), `hosts/rust-oracle/`(kernel32 로더), `examples/discount.mls` | `cargo test` 그린: 오라클이 **손수 만든 fixture DLL**을 로드해 §3-B 3개 assert 통과 | W0 |
| **W2** | 렉서·파서 (TDD) | `compiler`의 lexer/parser | `discount.mls` → AST 스냅샷 테스트 통과; 잘못된 입력은 명확한 에러 | W1 |
| **W3** | 타입체크 + 독립 IR (TDD) | typecheck(f64/bool), 비-Rust IR | AST → typed IR 테스트 통과; 타입 오류 케이스 실패 처리 | W2 |
| **W4** | 코드젠 (TDD) | IR → `no_std`+`extern "C"`+`repr(C)` Rust emit → `cargo cdylib` 호출 | 생성 Rust가 빌드되어 DLL 산출; 유닛 테스트 그린 | W3 |
| **W5** | 통합 (수용 A+B) | `mlc build examples/discount.mls` | 오라클이 **컴파일러 산출 DLL**(fixture 아님)로 §3-B 통과 | W4 |
| **W6** | 보호 측정 (수용 C) | strip/no_std 빌드 설정 | export 덤프 = `mlx_discount` + `ml_module_abi_version`만; 소스/디버그/패닉 문자열 최소임을 수치로 첨부 | W5 |
| **W7** | D14 산출물 | C 헤더(`.h`) + Delphi import unit(`.pas`) 생성기 | 헤더/유닛 생성 확인. **로드 게이트(수용 D)는 dcc64/cl/gcc 확보 후** — BLOCKED 표기 유지 | W5 |

## 게이트/블로커

- **BLOCKED (툴체인):** 수용 D(Delphi/C 호스트 실제 로드)는 이 머신에 `dcc64`/`cl`/`gcc`가 없어 실행 불가. W7은 **산출물 생성**까지, 실제 로드 검증은 별도.
  - 해소안(사용자 판단 필요): (a) Delphi/BDS CLI(`dcc64`)를 PATH에 추가, (b) MSVC Build Tools 또는 (c) MinGW/LLVM 설치. 어떤 것을 준비할지 확인 요청 예정.
- W6 export 측정 도구가 없으면(dumpbin 미설치) llvm-objdump 또는 Rust PE 리더로 대체 — W0에서 확정.

## 범위 밖 (후속 슬라이스, 별도 SPEC)

- D17 정수 status + out-param 에러 경로
- D16 caller-allocates 반환 / context handle 상태
- 문자열/구조체 마샬링, 콜백
- 두 번째 호스트(C#) — ROADMAP Phase 4

# ROADMAP

구현은 문서의 열린 질문 중 MVP에 필요한 것만 닫은 뒤에 시작한다 — **그 게이트는 2026-08-28에 닫혔고(Q1~Q5 → D14~D18) Phase 1 구현이 진행 중이다.** 현재 상태의 정본은 [docs/STATUS.md](STATUS.md)다.

## Phase 0 — 설계 고정 (완료)

- 비전/결정/아키텍처 문서화
- 표면 문법 계열 선택
- C ABI 초안 확정
- MVP 언어 범위 확정

완료 조건: `OPEN_QUESTIONS.md`의 Q1~Q5가 닫힘. → **닫힘(2026-08-28, D14~D18)**. Phase 1 툴체인 결정 D19~D22 확정.

## Phase 1 — 수직 슬라이스 (진행 중)

목표: 한 호스트에서 모듈 로드 → 함수 호출.

- 초소형 표면 문법 ✅ — **정본은 `docs/LANGUAGE.md`의 "현재 구현된 표면"이다**(여기 나열하면 슬라이스마다
  낡는다). 기둥만: `f64`/`bool`/`i32`, `if`/`return`/`while`, `let`·`let mut`, 내부 `fn`과 호출,
  단항·`&&`/`||`, `as`, 실패 가능 함수 `-> T!`
- 타입체크 ✅ (모든 경로 return·혼합 타입·예약어·중복 식별자 거부 포함)
- 네이티브 출력 (IR → `no_std`/`extern "C"` Rust → `cargo` cdylib) ✅
- C ABI 로더 (Rust kernel32 오라클) ✅
- `mlc build` CLI → `.dll` + `.h`(C 헤더) + `.pas`(Delphi unit) + `.lib`(MSVC 임포트 라이브러리) **네 가지** 산출 ✅
- **C 데모 호스트 ✅**(MSVC `cl`, `hosts/c-host/host.c` — 수용 D 통과) · Delphi 데모 앱 **✅ 게이트**(2026-09-09, §9-20 — `bds.exe -b`로 IDE 빌드를 불러 `MATHLESS_GATE_DELPHI`가 통과한다. **CI는 아직 아니다**)

완료 조건: `discount(price, vip)` 같은 함수를 모듈에서 호출.
→ 수용 A/B/C/**D 완료**(컴파일 · 오라클 로드·호출 · export/크기 보호 프록시 · **실제 C 호스트 로드**).

**자동 게이트는 CI에 둘이다**: C(`MATHLESS_GATE_D`)와 **Object Pascal**(`MATHLESS_GATE_FPC_HOST` —
Free Pascal로 빌드한 호스트가 x64 모듈을 로드·호출한다). **Delphi 자신은 로컬 게이트까지** —
`MATHLESS_GATE_DELPHI`가 2026-09-09부터 **반복 가능하게** 통과한다(에디션이 명령줄 빌드를 거부하면
`bds.exe -b`로 IDE 빌드를 부른다, §9-20). **남은 한 칸은 Delphi의 CI 게이트뿐이고, 그것은 여기서
닫을 수 없다** — 러너에 Delphi도 대화형 세션도 없다. `-Mdelphi`는 방언 에뮬레이션이라 대신하지 못한다.

> **그래서 Phase 1이 아직 "진행 중"인가.** 완료 조건(위 한 줄)은 **충족됐다.** 열어 두는 이유는
> 하나뿐이다 — D14가 지목한 플래그십 호스트의 **CI 게이트**. 그 한 칸은 이 저장소 밖의 조건
> (러너에 Delphi)이 풀려야 닫힌다. 언어 표면은 그동안 계속 넓어지고 있고, 그것은 Phase 1의
> 완료 조건이 아니다.

세부는 `docs/phase1/WBS.md`, 현재 상태·잔여 작업은 `docs/STATUS.md`.

## Phase 2 — 상태와 계약

- struct
- 호스트 → 모듈 import
- 인터페이스 정의 파일
- 개발/배포 빌드 구분 (strip)

## Phase 3 — 쓸 만한 DX

- 에러 메시지
- 기본 LSP (진단만이라도)
- 예제 2~3개 (비즈니스 룰, 계산, 상태 머신) — 셋 다 `examples/`에 있다(상태 머신은 `order.mls`)

## Phase 4 — 두 번째 호스트

- C# P/Invoke ⏳ 또는 C++ 헤더 — **C++는 헤더 컴파일까지 ✅**: 생성 헤더가 `cl /TP`로 컴파일된다
  (`hosts/rust-oracle/tests/c_host.rs`). 모듈을 부르는 C++ 호스트는 없다
- ABI 버전 정책 ✅ — Phase 1에서 앞당겨 구현했다(Windows 참조 호스트의 거부까지 — 서드파티 호스트에는
  계약이다). 정본은 [`HOST_ABI.md`](HOST_ABI.md) "버전"

## Phase 5 — 보호 강화 / 선택적 WASM

수요가 있을 때만.

## 하지 않는 것 (Phase 3까지)

- 패키지 레지스트리
- 웹 프레임워크
- 자체 JIT VM
- 완전한 OOP + 제네릭
- 상용 난독화 내재화

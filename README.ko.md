# Mathless

> **상태:** 개념 설계 단계 (구현 전) · Phase 0 결정 확정 (D14–D18)
> **언어:** 한국어 (이 파일) · [English → README.md](README.md)

**Mathless**는 Object Pascal/Delphi의 타입 안정성과 네이티브 성능을 계승하면서,
더 폭넓은 개발자가 쓰는 표면 문법으로 작성하고, 소스가 아닌 **보호된 네이티브
라이브러리**로 배포하는, **C ABI**로 로드되는 정적 타입 확장 언어이다.

태그라인 후보:

- *Mathless – Pascal for those who dropped math*
- *Mathless – 수학포기자를 위한 현대적 Pascal*

## 한 줄 정의

현대적 표면 문법 + 강한 정적 타입 + 컴파일 타임 변환 + 네이티브 바이너리 배포 + C ABI 호스트 연동.

## 왜 만드는가

Delphi/Object Pascal은 타입 안정성·네이티브 성능·구조적 문법에서 강하지만, 생태계가 닫혀
있고 동적 확장·도구·AI 경험이 뒤처진다. 기존 스크립트 언어(Python, Lua, JS)는 유연하지만
타입과 배포 산출물 보호가 약하다. Mathless는 Delphi의 강점을 **밖으로** 끌어낸다: 익숙한
표면, 그 아래의 네이티브 실행 모델, 소스 없는 네이티브 모듈 배포, 그리고 **호스트 재컴파일
없는** 연동.

## 현재 확정된 핵심

1. 이름은 **Mathless**.
2. 실행 모델은 **순수 네이티브** — 기본 VM/바이트코드 런타임 없음.
3. 배포는 **네이티브 공유 라이브러리**(DLL/`.so`) — **소스 미배포**.
4. 보호 목표는 **분석·변조 비용을 매우 높이는 것** — 리버싱 "불가능" 주장이 아니다.
5. 호스트는 **Delphi 한정 아님**. 1급 경계는 **C ABI**.
6. 표현력은 **복잡한 로직과 상태** — 단순 룰 테이블이 아니다.
7. 표면 문법은 Delphi 고유 문법보다 넓다. 내부 모델은 Delphi/네이티브.
8. 중간 계층은 **컴파일 타임 변환만** — 런타임 해석 없음.
9. 웹(브라우저)은 1차 목표가 아님 (WASM은 이후 별도 타깃으로 검토).
10. 강점 하나부터 — 여러 토끼를 동시에 잡지 않음.

## Phase 0 결정 (Q1–Q5 → D14–D18)

| # | 결정 |
|---|------|
| D14 | 주력 호스트 = **Delphi**(플래그십 데모/1급 호스트) + **C**(ABI 기준 경계) |
| D15 | 표면 문법 계열 = **중괄호·정적·값타입 우선 C 계열** (MVP 부분집합 = struct + 자유 함수 + 모듈) |
| D16 | 메모리 = **소유권 3층 분리** — 인자=빌림(호출 기간만) / 반환=caller-allocates / 장기 상태=명시적 context handle |
| D17 | 에러 = **ABI는 정수 상태코드 + out-param**, 표면 `Result`는 sugar, 예외는 ABI 미통과 |
| D18 | 모듈 포맷 = **표준 DLL/SO + ABI 버전 export 심볼 + 모듈 전용 export 접두어**, 암호화/서명 컨테이너는 P1 이연 |

세부·기각 대안: [docs/DECISIONS.md](docs/DECISIONS.md).

## 문서 지도 (이 순서로 읽을 것)

| 순서 | 파일 | 내용 |
|------|------|------|
| 0 | [CLAUDE.md](CLAUDE.md) | 에이전트 작업 규칙, 하지 말 것, 우선순위 |
| 1 | [docs/VISION.md](docs/VISION.md) | 왜 만드는가, 성공 조건 |
| 2 | [docs/DECISIONS.md](docs/DECISIONS.md) | 확정된 결정 / 기각된 대안 |
| 3 | [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) | 컴파일·배포·실행 파이프라인 |
| 4 | [docs/LANGUAGE.md](docs/LANGUAGE.md) | 표면 문법 vs 내부 모델 |
| 5 | [docs/HOST_ABI.md](docs/HOST_ABI.md) | C ABI, 호스트 연동 |
| 6 | [docs/SECURITY.md](docs/SECURITY.md) | 보호 목표와 단계 |
| 7 | [docs/ROADMAP.md](docs/ROADMAP.md) | MVP → 확장 |
| 8 | [docs/OPEN_QUESTIONS.md](docs/OPEN_QUESTIONS.md) | 아직 닫히지 않은 질문 |
| 9 | [docs/COMPETITIVE.md](docs/COMPETITIVE.md) | 기존 사례와 차별점 |
| 10 | [docs/GLOSSARY.md](docs/GLOSSARY.md) | 용어 |

## 성공 기준 (초기)

호스트를 **재컴파일하지 않고** Mathless로 컴파일한 네이티브 모듈을 로드해, 타입이 있는
함수(예: `discount(price, vip)`)를 호출할 수 있으면 1차 성공이다.

## 기여 / 워크플로

모든 변경은 **Pull Request**로만 반영한다 — `main`에 직접 커밋 금지.
자세한 규칙은 [CONTRIBUTING.md](CONTRIBUTING.md) 참고.

## 태그

**영문 (GitHub 토픽):** `programming-language` · `compiler` · `native` · `c-abi` · `delphi` ·
`object-pascal` · `dll` · `shared-library` · `plugin-system` · `static-typing` ·
`code-protection` · `extension-language` · `compile-time` · `design-docs`

**한글 태그:** 프로그래밍언어 · 컴파일러 · 네이티브 · C-ABI · 델파이 · 오브젝트파스칼 · DLL ·
공유라이브러리 · 플러그인시스템 · 정적타입 · 코드보호 · 확장언어 · 컴파일타임 · 설계문서

> GitHub 토픽은 소문자 ASCII + 하이픈만 허용하므로, 한글 태그는 저장소 토픽 대신 여기에
> 문서화한다.

## 프로젝트 상태

이 저장소는 아직 구현 저장소가 아니라 **설계 기준선**이다. Phase 0 결정(D14–D18)이 확정됐고,
다음 단계는 Phase 1 수직 슬라이스(C ABI 초안 → 최소 컴파일러 파이프라인 → Delphi 호스트 1개
로드)이다. [docs/ROADMAP.md](docs/ROADMAP.md) 참고.

## 라이선스

아직 미정 (TBD).

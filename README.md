# Mathless

> 상태: 개념 설계 단계 (구현 전)  
> 최종 정리: 2026-08-27  
> 목적: Claude 등 AI 에이전트와 협업해 구현을 시작하기 위한 기준 문서

**Mathless**는 Object Pascal/Delphi의 타입 안정성과 네이티브 성능을 계승하면서,  
더 폭넓은 개발자가 쓰는 표면 문법으로 작성하고,  
소스가 아닌 **보호된 네이티브 라이브러리**로 배포하는 동적 확장 언어이다.

태그라인 후보:

- `Mathless – Pascal for those who dropped math`
- `Mathless – 수학포기자를 위한 현대적 Pascal`

## 한 줄 정의

현대적 표면 문법 + 강한 정적 타입 + 컴파일 타임 변환 + 네이티브 바이너리 배포 + C ABI 호스트 연동.

## 문서 지도 (Claude는 이 순서로 읽을 것)

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

## 현재 확정된 핵심

1. 이름은 **Mathless**.
2. 실행 모델은 **순수 네이티브** (비용 효율). VM/바이트코드 런타임은 1차 목표가 아님.
3. 배포는 **소스 금지, 네이티브 공유 라이브러리**(DLL/.so, BPL 유사).
4. 보호 목표는 리버싱 **불가능이 아니라 분석 비용을 매우 높게**.
5. 호스트는 Delphi 한정 아님. 경계는 **C ABI**.
6. 표현력은 단순 룰이 아니라 **복잡한 로직과 상태**.
7. 표면 문법은 Delphi 고유 문법보다 **넓은 사용자층 문법**. 내부는 Delphi/네이티브 모델.
8. 중간 변환은 **런타임 해석이 아니라 컴파일 타임**.
9. 웹(브라우저)은 1차 목표가 아님. 이후 WASM 타깃으로 검토.
10. 전략은 강점 하나부터. 여러 토끼를 동시에 잡지 않음.

## 프로젝트 성격

아직 구현 저장소가 아니다.  
이 폴더는 **설계 기준 문서**이다. 구현을 시작할 때 이 문서를 복사해 실제 저장소의 루트로 옮긴다.

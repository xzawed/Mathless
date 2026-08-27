# COMPETITIVE

## 직접 참고

| 대상 | 배울 점 | 따라하지 않을 점 |
|------|---------|------------------|
| RemObjects Pascal Script | 임베딩, Inno Setup 검증, 바이트코드 배포 | 기본이 인터프리터. 회사 주력은 미들웨어(Remoting SDK)와 별개 |
| Inno Setup `[Code]` | 호스트 재빌드 없이 복잡한 설치 로직 | 설치 도구 특화, 웹/다중 호스트 아님 |
| DWScript / Lape | 현대 Object Pascal 스크립트 | 네이티브 배포·안티리버싱이 1목표가 아님 |
| AngelScript | C++-like 정적 타입 스크립트, 상용 게임 사용 | 대중 언어 아님. 웹은 실험적 |
| Lua | 임베딩의 왕, 작은 런타임 | 약한 타입, 보호 모델 다름 |
| Emscripten/Cheerp | 네이티브를 웹으로 | 스크립트 언어가 아니라 이식 툴체인 |
| AssemblyScript | 타입 + WASM | TS 계열, Delphi 탈출 서사와 다름 |

## RemObjects에 대한 정리

RemObjects는 주력으로 Remoting SDK / Data Abstract 같은 미들웨어를 판다.  
Pascal Script는 그 회사의 **오픈소스 스크립트 엔진**이며 Inno Setup이 사용한다.  
미들웨어와 스크립트 엔진을 같은 제품으로 보면 오해다.

## Mathless가 이기려는 한 가지 (1차)

기존 Pascal Script 대비:

- 표면 문법이 더 보편적
- 산출물이 네이티브 모듈
- 호스트가 Delphi만이 아님
- 배포 보호가 설계 목표

Lua/AngelScript 대비:

- 더 강한 타입과 AI 친화 계약 파일
- Delphi 제품군과의 친연성

이 한 가지가 흔들리면 프로젝트 정체성이 흔들린다.

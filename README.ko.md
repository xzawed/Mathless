# Mathless

> 타입 있는 로직을 **네이티브 모듈**로 컴파일한다. 호스트는 그것을 **C ABI**로 로드한다 —
> 소스 미배포, 호스트 재빌드 없음.

[![CI](https://github.com/xzawed/Mathless/actions/workflows/ci.yml/badge.svg)](https://github.com/xzawed/Mathless/actions/workflows/ci.yml)
![Rust](https://img.shields.io/badge/rust-1.97.1-orange?logo=rust&logoColor=white)
![Status](https://img.shields.io/badge/status-Phase%201%20complete%20except%20Delphi-blue)
![Target](https://img.shields.io/badge/target-Windows%20x64-informational)
![License](https://img.shields.io/badge/license-Apache--2.0%20OR%20MIT-blue)

**Mathless**는 다시 컴파일할 수 없는 — 또는 하고 싶지 않은 — 애플리케이션에 로직을 넣기 위한
정적 타입 확장 언어다. 작은 표면 문법으로 로직을 쓰면 컴파일러가 **컴파일 타임에** 그것을
네이티브 모듈(`.dll` / `.so`)로 바꾸고, 애플리케이션은 이를 **C ABI**로 로드한다. 배포되는 것은
바이너리이지 소스가 아니다.

네이티브 호스트(Delphi · C · C++ · C#)를 유지보수하면서, 고객·도메인별 규칙을 타입 있고 빠르게,
그리고 현장에서 쉽게 읽히지 않게 두고 싶은 사람을 위한 언어다.

English → **[README.md](README.md)**

## 동작 방식

```
.mls  →  파싱 / 타입체크  →  타입 IR  →  네이티브 codegen  →  모듈 (.dll/.so)  →  [ C ABI ]  →  호스트
```

이 전부가 컴파일 타임에 일어난다. 런타임 인터프리터도, 바이트코드 VM도 없다. 호스트가 로드하는
것은 C ABI 뒤의 네이티브 코드다.

## 왜 만드는가

Delphi와 Object Pascal은 강한 정적 타입과 네이티브 성능을 주지만, 생태계가 닫혀 있고 확장 경험이
뒤처진다. 스크립트 언어는 유연하지만 타입과 **배포물 보호**가 약하다. Mathless는 타입 있고
네이티브인 쪽을 골라, 그것을 **로드 가능하게** 만든다.

전 과정에서 지키는 네 가지이며, 아래의 모든 내용은 이 기준으로 측정된다:

- **네이티브 전용** — 기본 VM도, 바이트코드 런타임도 없다.
- **소스 미배포** — 배포물은 네이티브 공유 라이브러리다.
- **C ABI가 유일한 1급 경계** — 언어별 바인딩은 그 위의 얇은 래퍼이며, 그래서 이것은 Delphi 전용
  도구가 아니다.
- **보호는 비용으로 보고하지, 불가능으로 말하지 않는다** — export 심볼 개수, strip된 바이너리 크기,
  산출물 내 소스 부재. 결코 "리버싱 난이도"로 환산하지 않는다.

## 현재 상태

Phase 1의 수용 기준은 **C 쪽으로는 완료**됐다. `main` 실측(Windows x64, 툴체인 핀):

- **컴파일러 `mlc`** — lex → parse → typecheck → 백엔드 독립 IR → codegen(IR → `no_std`,
  `extern "C"` Rust → `cargo` cdylib).
- **현재 언어** — `f64` · `bool` · `i32`, 산술(`+ - *`, 나눗셈 `/`는 `f64`만)과 비교, 두 수치 타입
  사이의 명시적 `as` 변환; `if`(**아직 `else` 없음**) · `while` · `return`; `let`과 `let mut` + 대입;
  단항 `-`·`!`; `&&`·`||`; 실패 가능 함수 — `-> T!`와 `error NAME = N` · `fail NAME`, 정수 status와
  out-param으로 내려간다; 그리고 내부 `fn` 선언과 호출(**재귀는 컴파일 타임에 거부**).
  정본 목록은 [docs/LANGUAGE.md](docs/LANGUAGE.md).
- **CLI** — `mlc build <file.mls> -o <dir>`가 `<name>.dll`, `<name>.h`(C 헤더), `<name>.pas`
  (Delphi import unit)를 만든다.
- **실측된 호스트 경로 둘** — Rust `kernel32` 오라클, 그리고 생성된 헤더를 컴파일해
  `LoadLibrary` / `GetProcAddress`로 모듈을 호출하는 **MSVC로 빌드한 실제 C 호스트**.

수용 **A**(컴파일된다) · **B**(오라클이 로드·호출한다) · **C**(export·크기 프록시) ·
**D**(실제 C 호스트가 동일 모듈을 로드한다) 전부 통과. strip된 `no_std` 모듈은 약 9.7 KB이고
의도한 심볼 **정확히 두 개**만 export하며, 이 수치는 `dumpbin /exports`와 교차 확인했다 — 우리
PE 리더 하나에만 기대지 않는다.

**Delphi는 검증되지 않았다.** D는 C 쪽만 닫았다. 빌드 머신에 `dcc64`가 없어 생성된 `.pas`는 아직
아무도 컴파일한 적이 없고 DRAFT 표기를 유지한다. D14가 Delphi를 플래그십 호스트로 두므로, 호스트
서사의 절반은 여전히 증명되지 않았다.

현재 수치·열린 결정·다음 슬라이스는 [docs/STATUS.md](docs/STATUS.md)에 있다.

## 예제

```text
export fn discount(price: f64, vip: bool) -> f64 {
  let rate = 0.9
  if vip { return price * rate }
  return price
}
```

```sh
mlc build discount.mls -o out/
#  out/discount.dll   네이티브 모듈 — export: mlx_discount + ml_module_abi_version
#  out/discount.h     C 헤더
#  out/discount.pas   Delphi import unit
```

C 호스트는 `discount.dll`을 C ABI로 로드해 `mlx_discount(100.0, true) == 90.0`을 호출한다 —
호스트 자체를 다시 빌드하지 않고.

## 정직한 한계

- 지금은 **Windows x64**(`.dll`)다. Linux `.so` 타깃은 나중의 **명시적 결정**이지 누락이 아니다.
- 표면은 의도적으로 작고, 위 요약보다 크지 않다. 컴파일되는 것과 아직 안 되는 것의 **정확한 목록**은
  [docs/LANGUAGE.md](docs/LANGUAGE.md)가 정본이다.
- **모듈 호출은 돌아오지 않을 수 있다.** `while`은 영원히 돌 수 있고, C ABI 너머에서 그것을
  중단시키거나 타임아웃할 수단이 없다. 모듈은 신뢰되는 코드로 취급하며, 호스트가 응답성을
  유지해야 한다면 워커 스레드에서 호출하라.
- 확장자(`.mls`, `.mll`)와 C API 이름은 **가칭**이다.

## 문서

| 문서 | 내용 |
|------|------|
| [docs/STATUS.md](docs/STATUS.md) | `main`의 실측 상태, 열린 결정, 다음 슬라이스 |
| [docs/VISION.md](docs/VISION.md) | 왜 만드는가, 성공의 모습 |
| [docs/LANGUAGE.md](docs/LANGUAGE.md) | 표면 문법 vs 내부 모델 |
| [docs/HOST_ABI.md](docs/HOST_ABI.md) | C ABI, 호스트 연동, 생존성 계약 |
| [docs/SECURITY.md](docs/SECURITY.md) | 보호 목표와 단계, 측정된 프록시 |
| [docs/ROADMAP.md](docs/ROADMAP.md) | MVP → 확장 |
| [docs/DECISIONS.md](docs/DECISIONS.md) | 확정 결정(D01–D22)과 기각된 대안 |
| [docs/slices/](docs/slices/README.md) | 기능 슬라이스 SPEC — 작업의 단위 — 과 그 상태 |

## 기여

PR 우선 — `main`에 직접 커밋하지 않는다. 모든 변경은 테스트 주도로 진행되며, CI가 `fmt`·`clippy`·
테스트를 **두 잡**에서 돌린다: 수용 테스트가 실제로 실행되는 `windows-latest`, 그리고 플랫폼 독립
프런트엔드를 컴파일하는 보험인 `ubuntu-latest`. [CONTRIBUTING.md](CONTRIBUTING.md) 참고.

## 라이선스

다음 중 **하나를 선택**해 사용할 수 있다:

- Apache License 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT ([LICENSE-MIT](LICENSE-MIT))

Rust 생태계의 관례적 조합이다. MIT는 단순하고 GPLv2와 호환되며, Apache-2.0은 명시적 특허 허여를
더한다.

명시적으로 달리 밝히지 않는 한, 이 저장소에 제출한 기여는 Apache-2.0이 정의하는 바에 따라 위와
동일하게 이중 라이선스되며, 추가 조건은 붙지 않는다.

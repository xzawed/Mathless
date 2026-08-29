# Mathless

> 타입 있는 로직을 **네이티브 모듈**로 컴파일한다. 호스트는 그것을 **C ABI**로 로드한다 — 소스 미배포, 호스트 재빌드 없음.

[![CI](https://github.com/xzawed/Mathless/actions/workflows/ci.yml/badge.svg)](https://github.com/xzawed/Mathless/actions/workflows/ci.yml)
![Rust](https://img.shields.io/badge/rust-1.97.1-orange?logo=rust&logoColor=white)
![Phase](https://img.shields.io/badge/phase-1_·_vertical_slice-blue)
![Target](https://img.shields.io/badge/target-Windows_x64-informational)

**Mathless**는 정적 타입 확장 언어다. 작은 표면 문법으로 타입 있는 로직을 작성하면, 컴파일러가
그것을 **컴파일 타임에** **보호된 네이티브 모듈**(`.dll` / `.so`)로 바꾼다. 기존 애플리케이션은
이 모듈을 **C ABI**로 로드한다. 배포되는 것은 소스가 **아니라** 네이티브 바이너리다.

*네이티브 호스트(Delphi · C · C++ · C# …)에 타입 있고 보호된 로직을, **호스트 재컴파일 없이**
붙이고 싶은 개발자를 위한 언어.*

English → **[README.md](README.md)**

## 동작 방식 — 전부 컴파일 타임

```
.mls  →  파싱 / 타입체크  →  타입 IR  →  네이티브 codegen  →  모듈 (.dll/.so)  →  [ C ABI ]  →  호스트
```

런타임 인터프리터도, 바이트코드 VM도 없다. 모듈은 C ABI 뒤의 네이티브 코드다.

## 왜 만드는가

Delphi / Object Pascal은 강한 정적 타입과 네이티브 성능을 주지만, 생태계가 닫혀 있고 확장·도구
경험이 뒤처진다. 스크립트 언어(Python, Lua, JS)는 유연하지만 타입과 **배포물 보호**가 약하다.
Mathless는 둘 다 노린다: 익숙한 타입 표면, 그 아래의 네이티브 모듈, 소스 없는 배포, 그리고
**호스트 재컴파일 없는** 로드.

전 과정에서 지켜지는 네 가지:

- **네이티브 전용** — 기본 VM·바이트코드 런타임 없음.
- **소스 미배포** — 배포는 네이티브 공유 라이브러리.
- **C ABI가 유일한 1급 경계** — 언어별 바인딩은 그 위의 얇은 래퍼.
- **보호는 분석·변조 비용을 높이는 것** — 리버싱이 *불가능*하다는 주장이 아니다.

## 현재 상태 — Phase 1 (수직 슬라이스)

Windows에서 실측(`cargo test --workspace` = **154 그린**; CI는 `windows-latest`(정본 — 수용 A/B/C/D가 실제로 실행되는 곳)와 `ubuntu-latest`(프런트엔드) 두 잡, 툴체인 핀):

- **컴파일러 `mlc`** — lex → parse → typecheck → 백엔드 독립 IR → codegen (IR → `no_std`
  `extern "C"` Rust → `cargo` cdylib).
- **현재 언어** — `f64` / `bool` / `i32`, `if` / `return`, **실패 가능 함수**(`-> T!` = 정수 status +
  out-param), **지역 변수**(`let` / `let mut` + 대입), **`while` 루프**, **단항 `-`·`!`**, **`&&`·`||`**, **내부 `fn`과 호출**.
- **CLI** — `mlc build <file.mls> -o <dir>`가 `<name>.dll` + `<name>.h`(C 헤더) +
  `<name>.pas`(Delphi import unit)를 생성.
- **로드·호출** — Rust `kernel32` *오라클*이 컴파일된 모듈을 로드해 타입 함수를 호출한다. strip된
  `no_std` 모듈은 **의도한 심볼만** export(~9.7 KB).

수용 **A**(컴파일) · **B**(오라클 로드·호출) · **C**(export/크기 보호 프록시) · **D**(실제 **C
호스트**가 동일 모듈을 로드) 전부 통과. D는 Windows x64 + MSVC `cl` 기준이다:
[`hosts/c-host/host.c`](hosts/c-host/host.c)가 생성된 `.h`를 컴파일하고 `LoadLibrary`/
`GetProcAddress`로 export를 찾아 스칼라 경로와 에러 경로를 모두 호출한다. 같은 실행에서 우리
export 측정을 `dumpbin /exports`와 교차 확인한다.

**Delphi는 여전히 미검증이다.** D는 C 쪽만 닫았고, 이 머신에 `dcc64`가 없어 생성된 `.pas`는 아직
아무도 컴파일한 적이 없다 — DRAFT 표기를 유지한다.

## 예제

```rust
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

C 호스트는 `discount.dll`을 C ABI로 로드해 `mlx_discount(100.0, true) == 90.0`을 호출한다 — 호스트
재빌드 없이.

## 호스트

1급 경계는 **C ABI**이고, 언어별 바인딩은 그 위의 얇은 래퍼다. Phase 1은 **Delphi**(플래그십 데모
호스트)와 **C**(ABI 기준)를 대상으로 한다. **Delphi 전용이 아니다.**

## 정직한 한계

- Phase 1은 **Windows x64**(`.dll`) 대상이다. Linux / `.so` 빌드는 이후 목표.
- Phase 1은 수직 슬라이스다: 작은 표면과 **두 개**의 실측된 호스트 경로(Rust 오라클, MSVC로 빌드한
  C 호스트). **Delphi는 아직 아니다**(`dcc64` 없음) — D14가 Delphi를 플래그십으로 두므로 호스트
  서사는 절반만 증명됐다.
- 보호는 **프록시**로만 보고한다 — export 심볼 개수, strip된 바이너리 크기, 산출물 내 소스 부재 —
  결코 "리버싱 난이도"로 환산하지 않는다.
- 확장자(`.mls`, `.mll`)와 C API 이름은 **가칭**이다.

## 문서

| 문서 | 내용 |
|------|------|
| [docs/VISION.md](docs/VISION.md) | 왜 만드는가, 성공의 모습 |
| [docs/LANGUAGE.md](docs/LANGUAGE.md) | 표면 문법 vs 내부 모델 |
| [docs/HOST_ABI.md](docs/HOST_ABI.md) | C ABI와 호스트 연동 |
| [docs/SECURITY.md](docs/SECURITY.md) | 보호 목표와 단계 |
| [docs/ROADMAP.md](docs/ROADMAP.md) | MVP → 확장 |
| [docs/DECISIONS.md](docs/DECISIONS.md) | 확정 결정(D14–D22)과 기각된 대안 |
| [docs/STATUS.md](docs/STATUS.md) | `main`의 실측 상태와 잔여 작업 (세션 핸드오프) |
| [docs/slices/](docs/slices/README.md) | 기능 슬라이스 SPEC(= SDD 단위)과 그 상태 |

## 기여

PR 우선 — `main`에 직접 커밋 금지. 모든 변경은 테스트 주도로 진행되며 CI(`fmt` + `clippy` +
`windows-latest` 테스트)가 게이트한다. [CONTRIBUTING.md](CONTRIBUTING.md) 참고.

## 라이선스

다음 중 **하나를 선택**해 사용할 수 있다:

- Apache License 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT ([LICENSE-MIT](LICENSE-MIT))

Rust 생태계의 관례적 이중 라이선스다. MIT는 단순하고 GPLv2와 호환되며, Apache-2.0은 명시적
특허 허여를 더한다. 편한 쪽을 고르면 된다.

명시적으로 달리 밝히지 않는 한, 이 저장소에 제출된 기여는 Apache-2.0이 정의하는 바에 따라
위와 동일하게 이중 라이선스된다.

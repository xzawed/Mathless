# Mathless

> 타입 있는 로직을 **네이티브 모듈**로 컴파일합니다. 호스트는 그것을 **C ABI**로 로드합니다.
> 소스는 배포하지 않고, 호스트는 다시 빌드하지 않습니다.

[![CI](https://github.com/xzawed/Mathless/actions/workflows/ci.yml/badge.svg)](https://github.com/xzawed/Mathless/actions/workflows/ci.yml)
![Rust](https://img.shields.io/badge/rust-1.97.1-orange?logo=rust&logoColor=white)
![Status](https://img.shields.io/badge/status-Phase%201%20complete%20except%20Delphi-blue)
![Target](https://img.shields.io/badge/target-Windows%20x64-informational)
![License](https://img.shields.io/badge/license-Apache--2.0%20OR%20MIT-blue)

네이티브 애플리케이션을 운영하다 보면 자주 바뀌는 규칙이 있습니다. 가격, 할인, 자격 조건처럼
고객마다 조금씩 다른 것들입니다.

그때마다 호스트 전체를 다시 컴파일하기는 어렵습니다. 그렇다고 그 규칙을 읽을 수 있는 소스로
배포할 수도 없습니다.

**Mathless**는 바로 그 틈을 위한 작은 정적 타입 언어입니다. 규칙을 이 언어의 표면 문법으로 쓰면,
컴파일러가 **컴파일 타임에** 그것을 네이티브 모듈로 바꿉니다. 애플리케이션은 그 모듈을
**C ABI**로 로드합니다. 배포되는 것은 바이너리입니다.

Delphi, C, C++, C# 같은 네이티브 호스트를 위해 만들었습니다. 다만 지금까지 끝에서 끝까지 증명된
경로는 C 하나입니다. 자세한 것은 아래 "현재 상태"에 있습니다.

English → **[README.md](README.md)**

## 동작 방식

```
.mls  →  파싱 / 타입체크  →  타입 IR  →  네이티브 codegen  →  모듈 (.dll)  →  [ C ABI ]  →  호스트
```

이 모든 과정이 컴파일 타임에 끝납니다. 지금 만들어지는 모듈은 Windows `.dll`이고, Linux `.so`는
아직 착수하지 않은 목표입니다. 런타임에 무언가를 해석하지 않고, 바이트코드 VM도 없습니다.
호스트가 로드하는 것은 네이티브 코드이며, 다른 라이브러리와 똑같이 호출합니다.

## 왜 만드는가

Delphi와 Object Pascal은 강한 정적 타입과 네이티브 성능을 줍니다. 다만 생태계가 닫혀 있고 확장과
도구 이야기는 뒤처져 있습니다.

스크립트 언어는 유연성 문제를 풀어 줍니다. 그러나 타입이 약하고, 고객에게 건네는 것을 보호하는
데에는 더 약합니다.

Mathless는 타입 있고 네이티브인 쪽을 택해서, 그것을 **로드 가능하게** 만듭니다. 아래 네 가지가 전
과정에서 지켜지며, 이 문서의 나머지는 그 기준으로 측정됩니다.

- **네이티브 전용.** 기본 VM도, 바이트코드 런타임도 없습니다.
- **소스 미배포.** 배포되는 것은 네이티브 공유 라이브러리입니다.
- **C ABI가 유일한 1급 경계.** 언어별 바인딩은 그 위의 얇은 래퍼입니다. 그래서 이것은 Delphi 전용
  도구가 아닙니다.
- **보호는 비용으로 보고하고, 불가능이라 말하지 않습니다.** export 심볼 개수, strip된 바이너리
  크기, 산출물에 소스가 없다는 사실을 측정합니다. 이를 "리버싱 난이도"로 환산하지 않습니다.

## 현재 상태

Phase 1의 수용 기준은 C 쪽으로는 완료됐습니다. 아래는 모두 `main`에서 측정한 값이며, 환경은
Windows x64에 툴체인을 고정한 상태입니다.

**컴파일러.** `mlc`는 lex, parse, typecheck, 백엔드 독립 IR, codegen 순으로 동작합니다. 백엔드는
`no_std` + `extern "C"` Rust를 내보내고 이를 `cargo` cdylib으로 빌드합니다.

**현재 언어.** 타입은 `f64`, `bool`, `i32`, `string` 네 가지입니다. 수치 타입에는 산술(`+`, `-`,
`*`, `/`, 그리고 `i32`의 `%`)과 비교, 그리고 명시적 `as` 변환이 있습니다. `i32`의 나눗셈은
전역(total)이라 `x / 0`은 트랩이 아니라 `0`입니다. `string`은 **파라미터**나 **`-> string!` 반환**에
쓸 수 있고 연산은 `==`·`!=`(바이트 비교)뿐입니다. 반환은 **호스트가 준 버퍼**에 씁니다 — 모듈은
할당하지 않습니다. 제어 흐름은 `if`, `while`, `return`이며 `else`는 아직 없습니다. 지역 변수는
`let`과 `let mut`이고 대입을 지원합니다. 연산자로는 단항 `-`와 `!`, 그리고 `&&`와 `||`가 있습니다.
내장 함수는 `floor`·`ceil`·`round`·`trunc` 넷이고 C의 `<math.h>`와 정확히 같습니다. 함수는 실패
가능하게 선언할 수 있습니다. `-> T!`에 `error NAME = N`과 `fail NAME`을 쓰면 정수 status와
out-param으로 내려가며, 값을 여러 개 돌려주려면 `out` 파라미터를 더 선언할 수 있습니다. 내부
`fn`끼리 서로 호출할 수 있지만, 재귀는 컴파일 타임에 거부합니다. 정본 목록은
[docs/LANGUAGE.md](docs/LANGUAGE.md)에 있습니다.

**CLI.** `mlc build <file.mls> -o <dir>`가 세 파일을 나란히 만듭니다. `.dll` 모듈, `.h` C 헤더,
그리고 `.pas` Delphi import unit입니다.

**측정된 호스트 경로 두 개.** Rust `kernel32` 오라클이 모듈을 로드해 호출합니다. MSVC로 빌드한 실제
C 호스트도 마찬가지입니다. 이 호스트는 생성된 헤더를 컴파일하고 `LoadLibrary`와 `GetProcAddress`로
export를 찾습니다.

수용 A, B, C, D가 모두 통과합니다. 컴파일되고, 오라클이 호출하고, export·크기 프록시가 유지되며,
실제 C 호스트가 같은 모듈을 로드합니다. strip된 `no_std` 빌드는 약 9.7 KB이고 의도한 심볼 세 개
(`mlx_discount` + 예약 심볼 `ml_module_abi_version`·`ml_iface_hash`)만 export합니다. 이 개수는
`dumpbin /exports`와 교차 확인했습니다. 우리 PE 리더 하나에만 기대지 않습니다.

**Delphi는 검증되지 않았습니다.** 수용 D는 C 쪽만 닫았습니다. 빌드 머신에 `dcc64`가 없어서 생성된
`.pas`는 아직 아무도 컴파일한 적이 없고, DRAFT 표기를 그대로 두었습니다. D14가 Delphi를 플래그십
호스트로 두므로, 호스트 이야기의 절반은 아직 증명되지 않았습니다.

현재 수치와 열린 결정, 다음 작업은 [docs/STATUS.md](docs/STATUS.md)에 있습니다.

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
#  out/discount.dll   네이티브 모듈 — export: mlx_discount + ml_module_abi_version + ml_iface_hash
#  out/discount.h     C 헤더
#  out/discount.pas   Delphi import unit
```

C 호스트가 `discount.dll`을 로드해 `mlx_discount(100.0, true)`를 호출하면 `90.0`을 받습니다.
호스트 자체는 다시 빌드하지 않습니다.

## 정직한 한계

- **지금은 Windows x64입니다.** Linux `.so` 타깃은 아직 내리지 않은 결정이지, 빠뜨린 것이 아닙니다.
- **표면은 의도적으로 작습니다.** 오늘 무엇이 컴파일되고 무엇이 안 되는지는
  [docs/LANGUAGE.md](docs/LANGUAGE.md)가 정확히 적고 있습니다.
- **모듈 호출은 돌아오지 않을 수 있습니다.** `while` 루프는 영원히 돌 수 있고, C ABI 너머에서 그것을
  중단하거나 타임아웃할 방법이 없습니다. 모듈은 신뢰되는 코드로 다루시고, 호스트가 응답성을
  유지해야 한다면 워커 스레드에서 호출하시기 바랍니다.
- **이름은 아직 가칭입니다.** 확장자 `.mls`, `.mll`과 C API 이름은 바뀔 수 있습니다.

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

모든 변경은 PR로 들어오고, `main`에는 직접 커밋하지 않습니다. 작업은 테스트 주도로 진행하며, CI가
`fmt`와 `clippy`, 테스트를 두 개의 잡에서 돌립니다.

중요한 쪽은 `windows-latest`입니다. 수용 테스트가 실제로 실행되는 곳이기 때문입니다.
`ubuntu-latest`는 플랫폼 독립 프런트엔드를 컴파일하는 보험입니다. 나머지는
[CONTRIBUTING.md](CONTRIBUTING.md)에 있습니다.

## 라이선스

다음 중 하나를 선택해 사용하실 수 있습니다.

- Apache License 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT ([LICENSE-MIT](LICENSE-MIT))

Rust 생태계에서 흔히 쓰는 조합입니다. MIT는 단순하고 GPLv2와 호환되며, Apache-2.0은 명시적 특허
허여를 더해 줍니다.

**`mlc`가 여러분의 소스로부터 만들어 낸 것은 여러분의 것입니다** —
[LICENSE-OUTPUT-EXCEPTION](LICENSE-OUTPUT-EXCEPTION)을 보십시오. 생성된 `.dll`·`.h`·`.pas`에는
위 라이선스의 의무가 붙지 않습니다. 생성 헤더의 대부분이 저희 템플릿 문장인데도 그렇습니다
(실측: `discount.h` 39줄 중 여러분의 소스에서 온 것은 1줄, 저희 템플릿에서 온 것이 38줄입니다 —
그 비율이 이 예외를 가정하지 않고 문서로 적어 둔 이유입니다). 이 예외는 **산출물에만** 적용되며,
컴파일러 자체는 그대로 Apache-2.0 OR MIT입니다. 법률 조언이 아닙니다.

명시적으로 달리 밝히지 않는 한, 이 저장소에 제출하신 기여는 Apache-2.0이 정의하는 바에 따라 위와
동일하게 이중 라이선스되며 추가 조건은 붙지 않습니다.

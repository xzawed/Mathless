# Phase 1 SPEC — Vertical Slice (SDD)

> **상태: 확정(accepted) · 구현 완료(shipped)** — 2026-08-29 기준.
> §4의 DP1~DP4는 **2026-08-28 사용자 승인으로 닫혔고**, `DECISIONS.md` **D19~D22**로 반영되었다(PR #3).
> 구현: **W0~W7 = PR #4~#9**(+#10 하드닝, #11 `mlc build` CLI). **수용 A/B/C 통과**,
> **수용 D(실제 Delphi/C 호스트 로드)만 BLOCKED** — 빌드 머신에 `cl`/`gcc`/`dcc64` 없음.
> 이 문서는 이제 **설계 기록(design record)**이다. 여기서 닫힌 제안을 다시 열지 않는다 —
> 범위를 바꾸려면 **새 SPEC**을 쓴다.
> **근거 수준:** "실측 검증됨(E2)"로 표기한 것만 측정 완료. `main`의 최신 실측은 `docs/STATUS.md`.
> **작성 절차:** SDD. 이 SPEC 확인 후 `WBS.md`의 작업을 TDD로 진행했다.

## 1. 목표 (ROADMAP Phase 1)

한 호스트에서 Mathless로 컴파일한 네이티브 모듈을 **호스트 재컴파일 없이** 로드해 타입 있는 함수를 호출한다.

기준 함수:

```text
export fn discount(price: f64, vip: bool) -> f64 {
  if vip { return price * 0.9 }
  return price
}
```

## 2. 계약 (Contracts)

### 2.1 표면 소스 `.mls` — MVP 부분집합 (D15)

- 선언: `export fn NAME(params) -> TYPE { ... }`
- 타입: `f64`, `bool` (이 슬라이스 한정)
- 문: `if cond { ... }`, `return expr`
- 식: 리터럴, 파라미터 참조, 사칙연산(`* + - /`), 비교
- 이 슬라이스는 위 한 함수를 컴파일할 수 있으면 충분하다.

### 2.2 모듈 ABI (D18)

- 산출물: 플랫폼 표준 DLL (Windows x64 우선; §4 DP4).
- 사용자 export 접두어: `mlx_` (예: `mlx_discount`). 런타임 예약 `ml_*`와 분리.
- 예약 심볼: `ml_module_abi_version() -> u32`. 호스트가 major 불일치 시 로드 거부.
- 경계: cdecl / C ABI, `extern "C"`, `repr(C)`.
- **실측 검증됨(E2):** 위 export를 가진 cdylib를 kernel32 `LoadLibraryW`/`GetProcAddress`로 로드·호출 성공 (스모크 테스트, 아래 §3-B 수치).

### 2.3 에러/메모리 — 이 슬라이스 범위 밖 (명시)

- 이 슬라이스는 **스칼라 in/out만** 쓴다(에러 경로 없음).
- D17(정수 status + out-param)과 D16(context handle, caller-allocates 반환)은 **이 슬라이스에서 검증하지 않는다.** 후속 슬라이스에서 별도 SPEC으로 다룬다. (Grok 지적 반영: 미검증 명시)

## 3. 수용 기준 (측정 가능)

- **A. 컴파일:** `examples/discount.mls` → 컴파일러 → `discount.dll` 산출(수동 개입 없이).
- **B. 로드·호출 (CI 오라클, Rust kernel32 호스트):**
  - `mlx_discount(100.0, true)  == 90.0`
  - `mlx_discount(100.0, false) == 100.0`
  - `ml_module_abi_version()    == 1`
  - *현재 손수 작성한 fixture로 위 3개 통과 실측 완료(E2). 남은 것은 fixture를 컴파일러 산출물로 대체하는 것.*
- **C. 보호 측정 (D04/D05):** `discount.dll`의 export 목록을 도구로 덤프해 **`mlx_*` + `ml_module_abi_version`만** 노출되고, 소스/디버그 심볼/패닉 문자열이 최소임을 확인한다. `no_std` + strip 지향. **rustc가 자동 보장하지 않으므로 반드시 측정한다.** (Grok 지적)
- **D. D14 게이트 (별도, 현재 BLOCKED):** 동일 `discount.dll`을 **Delphi(플래그십) 또는 C 호스트**에서 로드·호출. 이 머신에 `dcc64`/`cl`/`gcc` 없음 → **툴체인 확보 전까지 BLOCKED**로 표기. 슬라이스는 C 헤더(`.h`)와 Delphi import unit(`.pas`)을 **산출**하되, 실제 로드 검증은 툴체인 확보 후.

> Phase 1 "완료"는 A+B+C이며, **D는 D14 정직성을 위해 반드시 별도 게이트**로 남긴다. CI 오라클(Rust) 그린만으로 "Delphi에서 됐다"고 말하지 않는다.

## 4. 결정 제안 (DP1~DP4 — **확인됨 2026-08-28 → D19~D22**)

측정 근거: rustc/cargo 1.97 설치·동작 / clang·gcc·cl(MSVC)·fpc·dcc·cmake·make 미설치 / cdylib 빌드 스모크·kernel32 로드 스모크 PASS(§2.2). ※ 이 스모크는 손수 쓴 Rust 기준이며, 수용 기준 §3-A(컴파일러 파이프라인)와 다르다.

### DP1 — 코드젠 경로 (Q6 관련: "닫음"이 아니라 잠정 3번째 경로)

| 옵션 | 장점 | 단점 | 판정 |
|---|---|---|---|
| **잠정 rustc lowering**: 비-Rust IR → `no_std`+`extern "C"`+`repr(C)` Rust → `cargo build --crate-type cdylib` | 설치된 유일 네이티브 툴, E2 검증됨, C 컴파일러 불필요 | Q6의 3번째 경로(닫지 않음). rustc/libstd 누출 위험(→ 수용 C로 측정). 모듈 작성자에 Rust 빌드 의존 | **MVP 잠정 채택, C-emit 슬롯 유지** |
| C-emit → clang/MSVC | 전통적, C 헤더 자연스러움 | C 컴파일러 미설치(현재 불가) | 보류(툴체인 확보 시 재검토) |
| LLVM 직접(inkwell/llvm-sys) | 최고 제어 | LLVM 라이브러리 필요, 무거움 | 이연 |

- **IR은 Rust가 아니라 독립 IR로 유지**한다(Q11 불변). Rust는 backend lowering 대상일 뿐.

### DP2 — 컴파일러 구현 언어

| 옵션 | 장점 | 단점 | 판정 |
|---|---|---|---|
| **Rust** | 설치·검증됨, 컴파일러 작성에 강함, cdylib+FFI 직결 | 러닝 커브 | **채택** |
| C++ | 네이티브 | 미설치, 빌드 무거움 | 기각 |
| Python(프로토) | 최속 프로토 | 별도 런타임, 타입 약함 | 기각 |

### DP3 — 호스트(2계층)

| 계층 | 무엇 | 지금 가능? |
|---|---|---|
| CI 오라클 | Rust kernel32 로더 호스트 | ✅ E2 검증됨 |
| **D14 done-gate** | Delphi(플래그십)/C 호스트가 동일 DLL 로드 | ❌ dcc64/cl/gcc 미설치 → BLOCKED |
| 산출물 | C 헤더(`.h`) + Delphi import unit(`.pas`) | ✅ 생성 가능(로드 검증만 BLOCKED) |

- Rust 오라클은 **테스트 오라클일 뿐** done-gate가 아니다. (Grok 지적)

### DP4 — 타깃

- **Windows x64 우선.** 단, **"msvc"를 D18에 못박지 않는다.** target은 빌드 설정으로 두고, export 집합·CRT/unwind 동작은 측정한다.

> **확인 완료(2026-08-28).** DP1~DP4는 `DECISIONS.md`에 **D19~D22**로 반영되었다(PR #3). DP1은 Q6를 "잠정 해결"로만 표기하고 완전 종결하지 않는다 — C-emit 슬롯은 열려 있다.

## 5. 명시적 미검증 / 가정 (Grok 교차검토) — 작성 시점 2026-08-28, **해소 현황 표기**

> 아래는 이 SPEC을 쓸 당시의 미검증 목록이다. 각 항목의 현재 상태를 덧붙인다(2026-08-29 실측).
> 최신 상태의 정본은 `docs/STATUS.md`.

- `.mls → parse → typecheck → emit` 파이프라인 **아직 없음**(스모크는 손수 쓴 Rust).
  → **해소.** W2~W5로 구현, `mlc build` CLI까지(PR #5~#7, #11). `cargo test --workspace` 67 그린.
- C 헤더 소비 및 Delphi/C 호스트 로드 **미검증**.
  → **여전히 미검증(수용 D, BLOCKED).** `.h`/`.pas`는 생성되지만(PR #9) `cl`/`gcc`/`dcc64` 미확보.
- D16(handle) / D17(status+out-param) **미검증**(범위 밖).
  → **D17만 해소**: 에러-경로 슬라이스로 구현·실측(SPEC `SPEC-D17-error-abi.md`, PR #14/#15).
  **D16은 여전히 범위 밖**(후속 슬라이스, SPEC 미작성).
- target triple·CRT/unwind·정확한 export 집합(`dumpbin /exports` 등) **미측정** → 수용 C에서 측정.
  → **해소.** 자체 PE 리더로 export 집합 측정(PR #8): `mlx_*` + `ml_module_abi_version`만.
  `dumpbin`은 여전히 미설치 — 교차 확인은 툴체인 확보 후.
- 이 문서의 어떤 항목도 측정 전까지 "확정"으로 서술하지 않는다.

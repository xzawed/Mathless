# Phase 1 SPEC — Error-Path Slice (D17: integer status + out-param)

> **상태:** 초안 — **사용자 확인 대기**. §5의 DP-E1~E3(특히 표면 문법)은 **제안**이다.
> 확인 전까지 구현 코드 착수 및 `DECISIONS.md` 변경 금지 (SDD 게이트).
> **근거 수준:** E0(문서 정합) + 기존 Phase 1 스칼라 슬라이스(E2)와의 정합. 새 측정값은 구현 후 E2.
> **선행:** Phase 1 스칼라 happy-path 슬라이스(수용 A/B/C 완료), 결정 **D17**, **Q13 = 평탄 i32**
> (사용자 확정 2026-08-28). 표면 문법 계열은 D15(중괄호·정적·값타입 C 계열).

## 1. 목표

Phase 1 스칼라 슬라이스는 **실패 없는** 함수만 다뤘다(기존 SPEC §2.3에서 에러 경로를 명시적으로 범위 밖으로 둠). 이 슬라이스는 **실패 가능 함수**를 추가한다:

1. 표면에서 "이 함수는 실패할 수 있다"와 "지금 실패한다"를 표현하고,
2. ABI는 D17대로 **정수 status(반환) + out-param(값)** 으로 lowering하며,
3. 오라클이 **성공 경로와 실패 경로를 모두** 호출·검증한다(E2).

기준 함수:

```text
error DIV_BY_ZERO = 1        // 모듈 스코프, i32 양수 도메인 에러

export fn safe_div(a: f64, b: f64) -> f64 {
  if b == 0.0 { fail DIV_BY_ZERO }
  return a / b
}
```

## 2. 계약 (Contracts)

### 2.1 표면 `.mls` (DP-E1 제안 — §5, 확인 대기)

- **실패 가능 함수**: 본문에 `fail <CODE>` 문을 하나 이상 포함하는 함수(별도 표식 문법은 DP-E1).
- **`fail <IDENT>`**: 선언된 도메인 에러 코드로 즉시 실패 반환. 값 out은 기록하지 않는다.
- **`return expr`**: 성공. 값 out = `expr`, status = 0.
- **에러 코드 선언**: `error <IDENT> = <정수>` (모듈 스코프, **i32 양수**). 이름은 대상 언어 예약어 검사(`reserved.rs`)를 함수 파라미터와 동일하게 통과해야 한다.
- 타입체크는 `fail`을 **종결 문**으로 취급한다("모든 경로 return" 분석에서 `fail`로 끝나는 경로는 값 반환을 요구하지 않는다 — 현재 codegen의 `block_always_returns`와 정합, Grok 지적).
- 이 슬라이스는 위 한 함수를 컴파일할 수 있으면 충분하다(happy-path 슬라이스와 동일한 범위 원칙).

### 2.2 모듈 ABI (D17 + Q13 확정)

실패 가능 함수 `f(params) -> T` 는 다음으로 lowering된다:

```c
int32_t mlx_f(<params...>, T* out);
```

- **반환값 = 평탄 i32 status (Q13 확정):**
  - `0` = 성공
  - **양수** = 모듈이 정의한 도메인 에러 코드
  - **음수** = 예약(런타임/ABI 레벨 에러). *이 슬라이스는 음수를 방출하지 않는다*(예약만).
- **성공 시:** `*out = value; return 0;`
- **실패 시:** `*out`는 **미변경**(호스트는 `status != 0`이면 out을 읽지 않는다는 계약, DP-E3). `return <code>;`
- **순수(무오류) 함수**는 Phase 1대로 **값을 직접 반환**한다(변경 없음). status/out 규약은 실패 가능 함수에만 적용.
- 예약 심볼 `ml_module_abi_version`은 그대로. 에러 코드는 **상수**로 헤더/유닛에 방출(§2.4)하며 export 심볼이 **아니다**.
- 경계: cdecl / C ABI, `extern "C"`, `repr(C)` — 기존과 동일. 예외는 ABI를 가로지르지 않는다(D17).

### 2.3 에러/메모리 경계 (명시)

- out-param은 **스칼라 T 한 개**를 포인터로 기록할 뿐이며 **caller-allocates 버퍼가 아니다.** D16(caller-allocates 반환 버퍼 / context handle)은 **이 슬라이스 범위 밖**.
- 문자열/구조체 out, 콜백은 후속(마샬링 슬라이스).

### 2.4 바인딩 산출물

- **C 헤더(`.h`)**: `int32_t mlx_safe_div(double a, double b, double* out);` + 에러 상수 `#define ML_ERR_DIV_BY_ZERO 1` (접두어는 DP-E2). DRAFT/BLOCKED 코멘트 유지.
- **Delphi 유닛(`.pas`)**: `function mlx_safe_div(a: Double; b: Double; out out_: Double): Integer; cdecl; external ML_MODULE;` (또는 `var out_`) + `const ML_ERR_DIV_BY_ZERO = 1;`. DRAFT/BLOCKED 유지.
- 실제 C/Delphi 호스트 로드는 **D14 게이트 — BLOCKED**(생성만, 검증은 툴체인 확보 후).

## 3. 수용 기준 (측정 가능)

- **A. 컴파일:** `examples/safe_div.mls` → `mlc build -o <dir>` → `safe_div.dll` + `safe_div.h` + `safe_div.pas`.
- **B. 로드·호출 (Rust kernel32 오라클):**
  - **성공:** `let mut out = SENTINEL; let s = mlx_safe_div(6.0, 2.0, &mut out);` → `s == 0 && out == 3.0`.
  - **실패:** `let mut out = SENTINEL; let s = mlx_safe_div(1.0, 0.0, &mut out);` → `s == 1 (DIV_BY_ZERO, 양수) && out == SENTINEL` (미변경 계약 검증).
  - **SENTINEL은 유한값**(예: `-999.0`)을 쓴다 — `NaN`은 `NaN == NaN`이 `false`라 "미변경" 검증이 불가능(Grok 지적).
- **C. 보호 (D04/D05):** export = **정확히** `mlx_safe_div` + `ml_module_abi_version` (에러 코드는 상수 → export 아님). strip/no_std 유지, 소스/파일명 비유출. 프록시만 측정.
- **D. D14 게이트 (별도, BLOCKED):** 동일 DLL을 실제 Delphi/C 호스트에서 status+out으로 호출. 툴체인(MSVC 예정) 확보 전까지 **BLOCKED** — 바인딩은 산출하되 실로드는 미검증.

> Phase 1 스칼라 슬라이스와 동일한 정직성: 오라클 그린을 "Delphi/C에서 됐다"로 말하지 않는다.

## 4. Q13 확정 반영 (문서 절차)

Q13(status 체계) = **평탄 i32: 0=OK / 양수=모듈 도메인 에러 / 음수=예약(런타임·ABI)** — 사용자 확정 2026-08-28.
이 확정을 `OPEN_QUESTIONS.md`(Q13 close)와 `DECISIONS.md`(D17 세부)에 반영하는 것은 **규칙 8**에 따라 **사용자 확인 후 별도 문서 PR**로 처리한다(이 SPEC은 제안·설계 문서일 뿐 DECISIONS를 바꾸지 않는다).

## 5. 설계 제안 (DP — 확인 대기)

### DP-E1 표면 실패 문법

| 옵션 | 예 | 장점 | 단점 | 판정 |
|---|---|---|---|---|
| **`fail CODE` 문 + `error NAME = N` 선언** | `if b==0.0 { fail DIV_BY_ZERO }` | 명시적, C/Delphi status와 직결, 표면 타입시스템 확장 최소 | 새 키워드 2개(`fail`,`error`) | **MVP 권장** |
| `Result<T>` 표면 타입 + `?` 전파 | `-> Result<f64>` … `let x = g()?` | 익숙(Rust/Swift), 합성 용이 | 제네릭·표면 타입시스템 확장 큼(MVP 규칙상 제네릭 배제) | 이연 |
| 별도 문법 없이 음수 반환 관례 | `return -1.0` | 문법 0 | 값/에러 혼동, 타입 불명확, D17 status와 불일치 | 기각 |

> **DP-E1 세부(Grok 지적):** 실패 가능 여부를 본문 `fail` 유무로 **추론**하면, 나중에 `fail`을 추가하는 순간 시그니처가 `T 반환` ↔ `int32 status + out`으로 바뀌어 **ABI 파괴**가 된다. 이를 피하려면 실패 가능성을 **명시적 표식**(예: `-> f64 fails` 또는 `-> f64!`)으로 **시그니처에 고정**하는 방안을 권장 옵션과 함께 확정한다(표식 유무는 사용자 확인 대상).

### DP-E2 에러 코드 이름공간

- 모듈별 `error` 선언, i32 양수. 헤더/유닛 상수는 `ML_ERR_<NAME>`(또는 `ML_ERR_<MODULE>_<NAME>`) 접두 — Q14(export 접두어) 확정과 정합시킨다.
- 음수 예약 범위(런타임/ABI 에러 코드 집합)는 후속에서 `runtime/ml_abi.h`에 표준화.

### DP-E3 성공 경로 out 계약

- `status != 0`이면 호스트는 out을 **판독하지 않는다**(문서화 계약). out 초기값은 호스트 책임.
- 대안(항상 out을 정의값으로 세팅)은 실패 시 불필요한 쓰기·정보 유출 → 채택하지 않음.

## 6. 범위 밖 / 미검증 (명시)

- D16 caller-allocates 버퍼 / context handle 상태.
- 문자열·구조체 out, 1단계 콜백(별도 마샬링 슬라이스).
- 표면 `Result` 완전 합성(`?` 전파) — DP-E1에서 `Result` 옵션 채택 시에만 후속.
- 실제 **Delphi/C 호스트** 로드(수용 **D**) — **D14 BLOCKED**. (오라클은 Rust로 C-ABI를 로드·검증하는 수용 B/C 수단일 뿐 D14 done-gate가 아니다.)
- 어떤 항목도 측정 전까지 "확정/동작"으로 서술하지 않는다.

## 7. WBS (이 슬라이스 — **사용자 확인 후** 착수, 각 = 1 PR, TDD)

| ID | 작업 | 완료 기준(측정) | 의존 |
|----|------|------------------|------|
| **WE1** | 렉서·파서: `fail`, `error NAME = N` | `safe_div.mls` → AST 스냅샷 통과; 잘못된 사용 명확한 에러 | SPEC 확인 |
| **WE2** | 타입체크·IR: 실패 가능 함수 표식 + 에러 코드 테이블 | 미선언 코드 `fail` 거부; 순수/실패 함수 구분 IR | WE1 |
| **WE3** | codegen: i32 status + out-param lowering(`fail`→`return <code>`, `return`→`*out=…;return 0`) | 생성 Rust 빌드→DLL; 유닛 테스트 그린 | WE2 |
| **WE4** | header/pas: status 반환형 + out-param + 에러 상수 | `.h`/`.pas` 계약 스냅샷 통과 | WE3 |
| **WE5** | 오라클: 성공/실패 두 경로 assert(E2) | §3-B 두 경로 통과, §3-C export/strip 통과 | WE3 |

> 절차: **이 SPEC 확인 → WBS 확정 → WE1..WE5 각 [실패 테스트 → 구현 → 통과 → Grok 검증] → PR → merge.**

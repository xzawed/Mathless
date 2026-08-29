# Phase 1 SPEC — Mutable Locals (`let mut`) + Assignment Slice

> **상태: 확정(accepted) — 2026-08-29 사용자 승인.** §4의 DP-M1~M4는 권장안 그대로 닫혔다
> (`let mut` 키워드 / `let mut` 지역만 대입 대상 / 대입은 문 / 복합 대입 제외).
> **구현은 WM1~WM5 TDD로 진행**하며, 완료되면 이 배너를 `구현 완료(shipped)`로 바꾸고 PR 번호를 적는다.
> `DECISIONS.md`는 바꾸지 않는다.
> **근거 수준:** E0(문서 정합) + 기존 슬라이스(E2)와의 정합. 새 측정값은 구현 후 E2.
> **선행:** Phase 1(스칼라 f64/bool/i32 + D17 + `let` 지역 변수). 설계 교차검토: Grok.

## 1. 목표

**가변 지역 변수** `let mut NAME = EXPR`와 **대입문** `NAME = EXPR`를 추가한다. 내부 전용(ABI
불변). 아직 반복문(`while`)이 없지만, **`if`가 문(式이 아님)** 이므로 **가변 변수가 분기 결과를 하나로
모으는 자연스러운 수단**이다(Grok). `while`은 이 대입을 그대로 재사용한다.

기준 함수(가변 sink + `if` 분기 대입):

```text
export fn discount3(price: f64, vip: bool) -> f64 {
  let mut result = price
  if vip { result = price * 0.9 }
  return result
}
```

## 2. 계약 (Contracts)

### 2.1 표면 `.mls`

- **`let mut NAME = EXPR`** — 가변 지역 변수(새 키워드 `mut`). 타입은 EXPR에서 추론.
- **대입문 `NAME = EXPR`** — 식별자로 **시작하는** 유일한 문. 가변 지역 변수를 재대입(RHS 타입이
  변수 타입과 같아야 함).
- 대입은 **문일 뿐 식이 아니다**(DP-M3): `a = b = c` 불가, 대입은 값을 만들지 않는다.
- 복합 대입(`+= -= *=`)은 **범위 밖**(DP-M4).

### 2.2 대입 대상 규칙 (DP-M2)

- 대상은 **스코프에 있는 `let mut` 지역 변수**여야 한다.
- **불변 `let` 재대입 → 에러.** **파라미터 재대입 → 에러**(D16: 인자=빌림/불변). **미선언 이름 → 에러**(미정의 변수).
- 타입 불일치(예: `i32` 변수에 `f64` 대입) → 에러.

### 2.3 스코프·규칙 (Grok 교차검토 반영)

- `let mut` 이름도 **예약어 검사 + 실패 가능 함수의 `out_value` 예약 + 무-섀도잉**(`let`과 동일).
- **블록 스코프**: `if` 본문에서 **바깥 `let mut`에 대입 가능**(블록 복제가 바깥 바인딩+타입+가변성을
  유지, codegen `x = e;`가 바깥 Rust `let mut`을 변경 — §1 `discount3`가 이 경로).
- `let mut`/대입은 **종결 문이 아니다**: 블록이 이들로 끝나면 기존 "모든 경로 return" 검사가 거부.

## 3. 수용 기준 (측정 가능)

- **A. 컴파일:** `examples/discount3.mls` → `mlc build` → `discount3.dll`.
- **B. 로드·호출 (오라클):** `mlx_discount3(100,true)==90`(if에서 result 대입), `(100,false)==100`
  (result 유지), `abi==1`.
- **C. 보호:** export = 정확히 `mlx_discount3` + `ml_module_abi_version`(가변 변수 비유출). strip 유지.
- **부정(타입체크) 케이스:**
  - 불변 재대입: `export fn f() -> i32 { let x = 1  x = 2  return x }` → 에러(불변)
  - 파라미터 재대입: `export fn f(a: i32) -> i32 { a = 1  return a }` → 에러(파라미터 불변)
  - 미선언 대입: `export fn f() -> i32 { y = 1  return 0 }` → 에러(미정의)
  - 타입 불일치: `export fn f() -> i32 { let mut x = 1  x = 1.0  return x }` → 에러(f64→i32)
  - 대입으로 끝나는 블록: `export fn f() -> i32 { let mut x = 0  x = 1 }` → 에러("모든 경로 return" — 대입은 종결 아님)
  - 대입은 식이 아님(M3): `export fn f() -> i32 { let mut x = 0  return x = 1 }` → 파싱 에러(`=`가 식에 없음)
  - `let mut` 이름: 예약어/`out_value`(fallible)/섀도잉은 `let`과 동일하게 거부
- **D. D14 게이트:** 가변 변수는 ABI 무관 → 수용 D와 별개(BLOCKED).

## 4. 설계 제안 (DP — **확인됨 2026-08-29**; Grok 권장안 그대로 채택)

| DP | 선택지 | 권장 | 근거 |
|----|--------|------|------|
| **M1 문법** | `let mut NAME` vs `var NAME` | **`let mut NAME`** | 선언 키워드 하나 유지; `var`는 C# 추론과 충돌·표면 분열 |
| **M2 대상** | `let mut` 지역만 vs 파라미터도 | **`let mut` 지역만** | 파라미터=빌림/불변(D16), 불변 `let`도 거부 |
| **M3 대입 형태** | 문만 vs 식(체이닝) | **문만** | 단순·안전; `a=b=c` 불가 |
| **M4 복합 대입** | 포함 vs 제외 | **제외** | sugar; `=`만 우선 |

## 5. 범위 밖 / 미검증

- 복합 대입(`+= -= *=`), 반복문(`while` — 이 대입을 재사용), 가변 파라미터/참조, 대입식(값 산출).
- 어떤 항목도 측정 전까지 "확정/동작"으로 서술하지 않는다.

## 6. WBS (이 슬라이스 — 확인 완료, WM1~WM5 TDD 착수)

| ID | 작업 | 완료 기준(측정) | 의존 |
|----|------|------------------|------|
| **WM1** | 렉서: `mut` 키워드 | 토큰 테스트 | SPEC 확인 |
| **WM2** | ast/파서: `let mut`(가변 플래그) + 대입문(Ident then `=` 1-토큰 lookahead) | `discount3.mls` → AST 통과; 잘못된 사용 명확한 에러 | WM1 |
| **WM3** | 타입체크·IR: 스코프 `HashMap<String,(IrType,is_mut)>`, `IrStmt::Assign`, 대입 규칙(가변만·타입일치·§3 부정) | §3 부정 케이스 거부; 정상 통과 | WM2 |
| **WM4** | codegen: `let mut x = e;` / `x = e;` 방출 | 생성 Rust 빌드→DLL | WM3 |
| **WM5** | 오라클: §3-A/B/C 실측(E2) | 로드·호출·export 통과 | WM4 |

> 절차: **이 SPEC 확인 → WBS 확정 → WM1..WM5 각 [실패 테스트 → 구현 → 통과 → Grok 검증] → PR → merge.**

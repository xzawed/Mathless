# LANGUAGE

## 이중 계층

| 계층 | 목표 |
|------|------|
| Surface | 폭넓은 개발자가 읽고 쓰기 쉬운 문법. Delphi `begin/end` 강요 안 함 |
| Meaning | 강한 정적 타입, 예측 가능한 실행, 네이티브로 내리기 쉬운 의미 |

표면은 마케팅/유입, 의미는 성능/보호/호스트 연동을 지탱한다.

## 표면 문법 (방향만 확정, 세부는 미정)

확정:

- Delphi 고유 문법을 그대로 쓰지 않음
- 타입을 숨기지 않음 (점진적 타입 포기 금지. 추론은 가능)
- AI가 생성하기 쉽도록 문법이 일관적이어야 함

검토한 후보 (계열은 D15로 확정 = 1번, 세부 문법만 미정):

1. C# / Java 계열 (중괄호, class, 익숙한 키워드)
2. TypeScript 계열 (더 넓은 웹/AI 훈련 데이터)
3. 간결 독자 문법 (진입은 신선, 학습 자료·AI 품질은 불리할 수 있음)

확정 방향(D15, 2026-08-28): **중괄호 기반·정적·값타입 우선의 C 계열 표면.** MVP 부분집합은 struct + 자유 함수 + 모듈이며, 아래 예시(`export fn … -> f64`)에 가깝다.  
"C#-like"는 개발자층·친숙도를 가리키는 **좁은 라벨**일 뿐, class 상속·GC·제네릭·async를 약속하지 않는다(전부 MVP 밖, `Q7`).  
이유: 값 타입/모듈이 네이티브 lowering에 직결, 넓은 친숙도와 AI 데이터, TS의 구조적 타이핑·웹 정체성 회피.

## MVP 언어 범위 (제안 — **목표**, 오늘 컴파일되는 것과 다름)

> 아래 두 목록은 MVP의 **목표 범위(제안)** 이다. **지금 실제로 컴파일되는 표면**은 다음 절
> "현재 구현된 표면"을 본다. 최신 실측의 정본은 `docs/STATUS.md`.

포함:

- 정수/실수/불리언/문자열
- 함수, 지역 변수, 상수
- if / while / for
- struct/record 수준 복합 타입
- null 안전 또는 option (둘 중 하나만)
- 모듈 단위 export/import (호스트 함수 import, 모듈 함수 export)
- 명시적 에러 처리 (예외 또는 Result. MVP는 하나만)

제외 (MVP 밖):

- 제네릭
- 본격 클래스 상속 체계 (필요 시 단순 struct + 함수부터)
- async/await
- 매크로, eval
- 동적 타입, Any 남용
- 리플렉션으로 임의 호스트 멤버 탐색

복잡한 로직·상태는 **구조체 + 함수 + 모듈 전역/컨텍스트 객체**로도 MVP에서 가능하다.

## 현재 구현된 표면 (2026-08-29 실측, E2)

`cargo test --workspace` = 66 pass / 0 fail 기준. **이 목록에 없는 것은 아직 컴파일되지 않는다.**

구현됨:

- 타입: `f64`, `bool`, `i32`
- `export fn NAME(params) -> T { … }` — 자유 함수, 모듈 export
- **실패 가능 함수**: `-> T!` 표식 + `error NAME = N` 선언 + `fail NAME` 문
  (D17 lowering = i32 status 반환 + out-param, 실패 시 out 미변경)
- `if cond { … }` — **`else` 없음**
- `return expr`
- **지역 변수** `let NAME = EXPR` — 블록 스코프, **불변**, 타입 추론
- 식: 리터럴, 파라미터·지역 변수 참조, `+ - * /`(단 `i32 /`는 미지원), 비교

아직 아님 (위 "포함" 목록 중 미구현):

- 문자열, struct/record, null 안전 또는 option
- `while` / `for`, `else`
- 상수 선언, **호스트 함수 import**(현재는 모듈 export 단방향)
- 가변 지역 변수 `let mut`·대입 — SPEC 초안 열림(PR #32, 확인 대기)
- `i32` ↔ `f64` 캐스트, `i32` 나눗셈·나머지, 체크드 오버플로

## 내부 의미 모델 (Delphi 방식의 의미)

“내부가 Delphi” = Delphi 컴파일러를 반드시 쓴다는 뜻이 아니다.

의미적으로 닮을 점:

- 강한 정적 타입
- 값 타입과 참조 타입 구분 가능성
- 숨은 동적 프로토타입 없음
- 컴파일 시점에 대부분의 계약 확정
- 호스트 호출은 명시적 import

닮지 않아도 되는 점:

- VCL
- unit/uses 그대로
- 문자열 내부 표현을 Delphi `UnicodeString`에 고정 (ABI에서 결정)

## 타입과 AI

호스트 API는 기계가 읽는 인터페이스 파일로 공개한다.  
예: `host.mls.d` / `host.abi.json` 같은 계약 파일.  
AI와 컴파일러가 같은 계약을 본다.

## 예시 (세부 문법은 여전히 미확정 — 확장자·이름은 가칭)

아래는 **오늘 실제로 컴파일된다**(`mlc build`, 오라클 로드·호출로 E2 검증):

```text
export fn discount(price: f64, vip: bool) -> f64 {
  let rate = 0.9
  if vip { return price * rate }
  return price
}

error DIV_BY_ZERO = 1                       // i32 양수 도메인 에러

export fn safe_div(a: f64, b: f64) -> f64! {   // `!` = 실패 가능
  if b == 0.0 { fail DIV_BY_ZERO }
  return a / b
}
```

내부에서는 동일 시그니처의 네이티브 함수로 내려가고, 호스트는 C ABI로 호출한다.
실패 가능 함수는 D17대로 `int32_t mlx_safe_div(double a, double b, double* out_value)`로 내려간다
(성공=0 + out 기록, 실패=양수 코드 + out 미변경).

# SPEC — 스팬 비교 `byte_slice(s, a, b) == "…"`

> **기록 문서다.** 이 SPEC은 닫힌 슬라이스의 설계 기록이다 — 본문은 닫힌 날의 사실이고 다시 고치지 않는다.
> 현재 상태는 [`docs/STATUS.md`](../STATUS.md)가 말한다.

- **상태: 확정 · 구현 완료 (2026-09-13).** **DP-C1~C5를 사용자가 확인했다** — 다섯 다 권고대로다.
  - 갈리는 둘을 따로 물었다: **DP-C1 스팬 비교**(`starts_with` 내장이 아니라), **DP-C2 `-2` → `!`**.
    나머지 셋(`ByteSlice`만 · `ml_subeq` 신규 · 스팬 대 스팬 제외)은 살아 있는 대안이 없어 함께 확인됐다.
  - **수용 A~L 전부 닫혔다.** 어느 테스트가 어느 기준을 닫았는지 §3에 적었다.

  > **📌 §5.2의 첫 줄을 정정한다 — 위험이 둘로 갈린다(실측).** 방출기를 되돌려 **두 가지 방식으로**
  > 깨 봤다:
  >
  > | 무엇을 뺐나 | 결과 |
  > |---|---|
  > | 비교 lowering만 | **`unreachable!` 패닉** — 조용하지 않다. `ByteSlice`를 식으로 낼 길이 없다 |
  > | 비교 lowering **+** `ByteSlice` 팔에 맨 포인터 | **조용한 오답 `(0, false)`** (진실은 `(0, true)`) |
  >
  > **즉 두 보호가 겹쳐야만 안전하다.** `unreachable!` 하나만으로는 충분하고, 그것을 채우는
  > 순간 수용 C가 유일한 방어가 된다. 검증이 짚은 그대로다.
- 선행: `SPEC-string-slice.md`(#229/#230 — `byte_slice`) · `SPEC-string-input.md`(**DP-S2 불투명 바이트** ·
  **DP-S3 연산 범위** · `==`의 의미) · `SPEC-array-input.md`(**인덱싱이 함수를 `!`로 만든다**)
- 관련 결정: **D16**(할당자 없음) · **D17**(i32 status) · **DP-S2** · **DP-S3**
- 발단: **2026-09-13 §7 재실행**(`STATUS.md` §9-46) — 새 표본의 R2

---

## 0. 왜 — 측정부터

### 0.1 방금 넣은 기능의 가장 자연스러운 용법이 거부된다 (E2)

§9-46이 보험 청구 심사 규칙 여섯을 쓰고 **값을 쟀다**. R2가 이것이다:

```mls
export fn valid_claim(code: string) -> bool {
  if byte_len(code) != 8 { return false }
  return byte_slice(code, 0, 2) == "AB"
}
```

```
→ a built string cannot be used on the left of a comparison
```

**세 철자를 다 쟀고 전부 거부된다** — 좌변·우변·`!=`, 그리고 `byte_len(byte_slice(...))`도.
즉 **스팬은 반환만 되고 검사할 수 없다.**

`byte_slice`(#230)와 `==`(#89)가 **둘 다 있는데 합성이 안 된다.** 읽는 사람은 예상하지 못한다.

### 0.2 거부 자체는 옳다 — 고칠 것은 *방법*이다

스팬은 `to` 자리에 **NUL이 없다.** 그것을 C 문자열 비교에 넘기면 스팬 밖을 읽는다.
`is_built_string`이 `ByteSlice`에 `true`를 답하는 이유이고(`SPEC-string-slice` §2.6),
그 판단은 **유지된다.**

### 0.3 무엇이 남는가 — 우회는 하나뿐이고 호스트로 간다

| 우회 | 어디서 |
|---|---|
| 모듈이 스팬을 **반환**하고 호스트가 비교한다 | **호스트** |
| 전체 문자열을 유한 집합과 비교한다 | 접미가 가변이면 불가능 |

**모듈 안 우회가 없다.** §9-38의 잣대(*"우회가 호스트로 일을 밀어내는가"*)로 ❌다.

### 0.4 정직하게: 이 슬라이스는 R2를 **절반만** 푼다

R2는 *"8바이트, 앞 2자리 `AB`, **뒤 4자리는 숫자**"* 였다. 앞 2자리는 이 슬라이스가 푼다.
**뒤 4자리가 숫자인지는 여전히 못 쓴다** — 바이트 하나를 범위 비교할 수단이 없다(`s[i]`도
문자 분류도 없다). 그것은 별도 결정이고 **이 문서는 그것을 열지 않는다.**

### 0.5 사전 검증이 설계의 중심을 정했다 (E2)

기둥 셋을 SPEC 전에 검증에 걸었고 **3/3 CONFIRMED**였다. 그리고 *묻지 않은 것*이 이렇게 답했다:

> *"`ByteSlice` is `unreachable!` in `emit_expr` today, and the `ml_sublen` bounds check lives
> only in `emit_concat_return` — lowering a span to `s.add(from)` would let every NUL-walker
> over-read the host past `to`."*

**코드로 재확인했다. 맞다.** §2.5가 그것이고, 이 문서에서 가장 중요한 제약이다.

---

## 1. 목표

스팬을 **모듈 안에서** 비교한다. 접두·접미·중간 어디든.

```mls
export fn valid_claim(code: string) -> bool! {
  if byte_len(code) != 8 { return false }
  return byte_slice(code, 0, 2) == "AB"
}
```

**하지 않는 것**: 순서 비교(`<`), 대소문자 무시, 문자 분류, 탐색(`index_of`), 스팬 대 스팬.

---

## 2. 계약 (Contracts)

### 2.1 무엇이 비교 가능해지는가 — **`ByteSlice`만** (DP-C3)

`is_built_string`은 셋에 `true`를 답한다. **그 셋은 같지 않다**(검증 확인):

| 종류 | 비교 시점에 바이트가 | 판정 |
|---|---|---|
| `Concat(…)` | **아직 없다** — `return`에서 호스트 버퍼에 써야 생긴다 | 그대로 **거부** |
| `Cast { .. }`(`i32 as string`) | **아직 없다** — 같은 이유 | 그대로 **거부** |
| **`ByteSlice`** | **이미 있다** — 빌린 `s` 안을 가리킨다 | **허용** |

**`ByteSlice`만 푸는 것이 규칙을 약화시키지 않는 이유가 이 표다.** 나머지 둘의 거부 사유는
*"바이트가 없다"* 이고, 그것은 변하지 않는다.

### 2.2 상대편은 **빌린 문자열**이다 (DP-C1)

`byte_slice(...) == <리터럴 | string 파라미터>`, 양방향, `!=` 포함.

**스팬 == 스팬은 이번에 하지 않는다**(DP-C5). `SPEC-string-return` §0.2가 정한 규율 그대로 —
**프로토콜을 여는 슬라이스는 가장 작은 것으로 고른다.**

### 2.3 길이가 다르면 **같지 않다**

`byte_slice(c, 0, 2) == "ABC"` → **false**. 스팬은 2바이트, 리터럴은 3바이트다.

**이것이 구현을 가르는 자리다.** `ml_streq`를 재사용하면 스팬에 NUL이 없어 **소스의 나머지를 계속
읽고**, `"AB"` 와 `"ABZZZZ"` 를 비교하게 된다 — 방향이 정반대인 오답이다(수용 C).

### 2.4 범위 밖 — **실패한다, `false`가 아니다** (DP-C2)

스팬이 문자열 밖이면 `ML_ST_INDEX_OUT_OF_RANGE`(`-2`). **`false`로 답하지 않는다** — 그것은
clamp와 같은 조용한 오답이고, D17이 음수 status를 둔 이유다.

**따라서 스팬을 비교하는 함수는 `!`여야 한다.** 이것은 **새 규칙이 아니라 기존 규칙**이다 —
배열 인덱싱이 이미 그렇고, 진단 모양까지 있다(실측):

```
function 'f' indexes 'xs', so it can fail — declare it `-> bool!`.
An index outside 0..len is reported as a reserved negative status,
and D17 only gives a function a status when its signature says `!`
```

같은 문장 구조로 스팬을 말한다.

> ⚠ **대가를 적는다.** 오늘 `-> bool` 술어를 쓰던 저자가 `-> bool!`로 바꿔야 하고, 호스트는
> out-param 한 칸을 더 받는다(D17). `byte_len`만 쓰는 검사는 여전히 `-> bool`이므로,
> **스팬 비교를 쓰는 순간** 시그니처가 바뀐다.

### 2.5 **스팬은 식 자리에서 절대 맨 포인터가 되지 않는다** — 이 슬라이스의 진짜 위험

오늘 `emit_expr`의 `ByteSlice` 팔은 **`unreachable!`** 이고, 범위 검사는 **`emit_concat_return`
안에만** 있다. 스팬을 식에서 쓸 수 있게 만들면서 그 팔을 *"`s.add(from)`을 내놓는다"* 로 채우면,
**모든 NUL 보행자가 `to` 너머를 읽는다** — 검증이 묻지 않은 것에서 짚었고 코드로 확인했다.

**그래서 `ByteSlice` 팔은 계속 `unreachable!`로 둔다.** 비교 **전체**를 하나로 낮춘다:

```rust
// Binary { op: Eq|Ne, lhs: ByteSlice { s, from, to }, rhs }
{ let __f = <from>; let __t = <to>;
  let __l = ml_sublen(<s>, __f, __t);
  if __l < 0 { <bail> }                  // 배열 인덱싱과 같은 블록식·같은 탈출
  ml_subeq(<s>, __f, __l, <rhs>) }
```

배열 인덱싱의 `IrExprKind::Index` 팔이 **이미 같은 모양**이다(블록식 + 이른 `return`). 그것을
베낀다 — 새 기계장치가 아니다.

> 🔴 **`<bail>`은 반환 ABI마다 다르다. 이것을 상수로 적으면 안 된다.**
> 초안은 여기에 `return -2;`를 적었는데 **그것은 `StringOut` 형태**다. 이 슬라이스의 동기가 되는
> 예제는 `-> bool!`, 즉 `RetAbi::Fallible`이고 그 본문은 `Result<bool, i32>`를 돌려주므로
> **`return Err(-2);`** 여야 한다. 그대로 베끼면 **생성 Rust가 컴파일되지 않는다.**
>
> | ABI | bail |
> |---|---|
> | `Fallible` (`-> bool!` — 이 슬라이스의 주 경로) | `return Err(-2);` |
> | `StringOut` · `ArrayOut` | `return -2;` |
> | `Plain` | 도달 불가 — §2.4가 `!`를 강제한다 |
>
> `Index` 팔이 이미 이 세 갈래를 `match abi`로 하고 있다. **구현은 그 match를 베끼고, 위 한 줄을
> 베끼지 않는다.** (사전 검증이 *묻지 않은 것*에서 짚었다 — SPEC 자신의 조각이 틀려 있었다.)

### 2.6 헬퍼 — `ml_subeq` (DP-C4)

```rust
fn ml_subeq(a: *const u8, from: i32, n: i32, b: *const u8) -> bool {
    // n바이트를 비교하고, b가 정확히 n바이트인지까지 본다.
    // b 쪽 NUL을 루프 안에서 보는 것은 "b가 더 짧다"를 잡기 위함이고,
    // 루프 뒤의 검사는 "b가 더 길다"를 잡는다. 둘 다 없으면 §2.3이 깨진다.
    let mut i: i32 = 0;
    while i < n {
        let (x, y) = unsafe { (*a.add((from + i) as usize), *b.add(i as usize)) };
        if y == 0 || x != y { return false; }
        i += 1;
    }
    unsafe { *b.add(n as usize) == 0 }
}
```

`ml_streq`는 **그대로 둔다** — 빌린 문자열끼리의 비교는 바뀌지 않는다.

### 2.7 인코딩 — 아무것도 열지 않는다

바이트 동등성이다(DP-S2 그대로). 대소문자를 구분하고 정규화하지 않는다. **DP-S2를 열지 않는다.**
다만 §9-46의 R6이 적은 대가는 그대로다 — 리터럴은 ASCII뿐이므로(DP-S4) 비교 대상도 ASCII다.

---

## 3. 수용 기준 (측정 가능)

**모두 실제 로드된 모듈에서 잰다(E2).** 어느 테스트가 어느 기준을 닫는지 머지 시점에 이 표에 적는다.

- **A. 값.** `"AB123456"` → true, `"AX123456"` → false.
- **B. 길이가 다르면 false.** `byte_slice(c,0,2) == "ABC"` → **false** (스팬 2 vs 리터럴 3),
  그리고 `byte_slice(c,0,3) == "AB"` → **false** (반대 방향).
- **C. 접미사가 아니다.** 소스가 스팬보다 **길다**: `"ABZZZZZZ"` 에서 `byte_slice(c,0,2) == "AB"` 가
  **true**. `ml_streq` 재사용은 여기서 **false**를 낸다 — 오답의 방향이 §2.3과 정반대다.
- **D. 범위 밖은 `-2`.** `byte_slice(c,0,2)` 를 1바이트짜리 `c`에 쓰면 `-2`, out-param 미변경.
- **E. `!` 강제.** `-> bool`로 쓰면 **컴파일 거부**, 진단이 `-> bool!`를 댄다.
- **F. 나머지 둘은 그대로 거부.** `(a + b) == "x"`, `(n as string) == "1"` 둘 다 거부.
- **G. 양방향·`!=`.** 우변 스팬, `!=` 둘 다 값이 맞다.
- **H. 빈 스팬.** `byte_slice(c,2,2) == ""` → **true**.
- **I. 파라미터 상대.** `byte_slice(c,0,2) == prefix` (둘 다 `string` 파라미터).
- **J. 보호 프록시 무변경.** export 3개, import 집합이 대조 모듈과 **동일**. **테스트로 고정한다.**
- **K. 호스트 두 곳.** 오라클 + **실제 C 호스트**(수용 D 게이트).
- **L. `ByteSlice`가 식에서 맨 포인터가 되지 않는다.** `emit_expr`의 팔이 여전히 `unreachable!`이고,
  생성 Rust에 `ml_subeq` 없이 스팬 포인터가 나가는 자리가 없다.

> **닫힌 자리 (2026-09-13 실측).**
>
> | 수용 | 닫은 것 |
> |---|---|
> | A·B·**C**·G·H | `a_span_compares_by_length_not_by_nul` — **소스가 스팬보다 길다** |
> | D·I | `an_out_of_range_span_comparison_is_a_status_not_false` |
> | E | `a_span_comparison_needs_the_bang` (컴파일러) |
> | F | `the_other_built_strings_are_still_refused_in_a_comparison` |
> | J | `a_span_comparison_adds_no_import_over_a_comparing_baseline` — **테스트로 고정** |
> | **K** | **`hosts/c-host/host.c`의 `is_kookmin` 블록** — §7의 R2 규칙 자체 |
> | L | 위 📌의 두 갈래 파괴 실측 + `emit_expr` 팔이 `unreachable!` 그대로 |
>
> **덤으로 하나 더 닫았다**: `the_span_helpers_are_only_emitted_where_they_are_called`.
> 첫 구현이 스팬을 비교하는 모든 모듈에 **`ml_streq`를 죽은 채로** 넣고 있었다 —
> `compares_strings`가 스팬 비교까지 세고 있었기 때문이다. `ml_wsub`도 비교만 하는 모듈에서
> 죽어 있었다. 셋을 각자 게이트로 갈랐고(§9-40이 `emit_slen_helper`를 가른 것과 같은 이유),
> 골든 diff가 **+40줄에서 +30줄로** 줄었다.

### 3.1 반드시 거부되는 것

| 소스 | 이유 |
|---|---|
| `-> bool` 안의 스팬 비교 | §2.4 — 진단이 `-> bool!`를 댄다 |
| `(a + b) == "x"` · `(n as string) == "1"` | §2.1 — 바이트가 아직 없다 |
| `byte_slice(a,0,1) == byte_slice(b,0,1)` | DP-C5 — 이번 범위 밖, 진단이 그렇게 말한다 |
| `byte_slice(s,0,2) < "AB"` | 순서 비교는 없다(DP-S3) |
| `byte_len(byte_slice(s,0,2))` | 그대로 거부 — 이 슬라이스는 **비교만** 연다 |

---

## 4. 설계 결정 (DP — **확정, 2026-09-13 사용자 확인 — DP-C1~C5 전부 권고대로**)

| # | 질문 | 권고 | 대가 |
|---|---|---|---|
| **DP-C1** | 모양 | **`byte_slice(...) == <빌린 문자열>`** | 대안은 `starts_with(s, lit)` 내장 — **실패할 수 없어 `!`가 불필요**하고 더 싸다. 그러나 **중간 스팬을 못 다루고 `byte_slice`와 합성도 안 된다** — §0.1의 문제를 그대로 남긴다 |
| **DP-C2** | 범위 밖 | **`-2` 실패 → 함수가 `!`** | `-> bool` 술어가 `-> bool!`로 바뀐다. 대안(`false`로 답하기)은 clamp와 같은 조용한 오답 |
| **DP-C3** | 무엇을 푸는가 | **`ByteSlice`만** | 없다. `Concat`/`Cast`의 거부 사유는 변하지 않는다(§2.1) |
| **DP-C4** | 헬퍼 | **`ml_subeq` 신규** | `ml_streq` 재사용은 §2.3을 정반대로 깬다 |
| **DP-C5** | 스팬 == 스팬 | **이번에 안 한다** | 두 스팬의 범위 검사가 둘로 늘고, 작은 쪽에서 먼저 결함을 찾는 규율(`SPEC-string-return` §0.2)을 따른다 |

---

## 5. 범위 밖 / 위험

### 5.1 이 슬라이스가 하지 않는 것

- **R2의 나머지 절반** — 바이트가 숫자인지(§0.4). 별도 결정이다.
- **순서 비교·대소문자·탐색** — DP-S3는 **한 번에 한 연산씩** 연다.
- **스팬 == 스팬** — DP-C5.
- **비ASCII 리터럴** — DP-S4 그대로(§9-46 R6의 대가는 남는다).

### 5.2 이 슬라이스가 만드는 새 위험

| 위험 | 막는 것 |
|---|---|
| **스팬이 맨 포인터가 되어 NUL 보행자가 밖을 읽는다** | §2.5 — `emit_expr` 팔은 `unreachable!` 유지 + **수용 L** |
| **bail을 ABI별로 고르지 않는다** | §2.5의 🔴 표. `-> bool!`에 `return -2`를 내면 **생성 Rust가 컴파일되지 않는다** — 시끄러운 실패이고, 수용 A가 즉시 잡는다 |
| **접미사 오답** — `ml_streq` 재사용 | §2.6 + **수용 C**(소스가 스팬보다 길 것) |
| **길이 불일치를 같다고 답한다** | `ml_subeq`의 **루프 안·루프 뒤 두 검사** + **수용 B 양방향** |
| **`!` 강제가 기존 코드를 깬다** | 코퍼스에 스팬 비교가 0개다(오늘 컴파일되지 않으므로). **새 대가는 저자에게만** |

---

## 6. WBS (확인 후 착수, 각 1 PR)

| # | 작업 | 완료 기준 |
|---|---|---|
| **E1** | 타입체커: 비교 위치에서 `ByteSlice`만 허용, `!` 강제, §3.1의 거부 5종 | 수용 E·F, 진단이 **맞는 이름**을 댄다 |
| **E2** | codegen: `Binary{Eq/Ne}` 팔에서 스팬을 **통째로** 낮춘다 + `ml_subeq`. ⚠ **`emit_expr`의 `ByteSlice` 팔은 `unreachable!`로 둔다**, ⚠ **bail은 `Index` 팔의 `match abi`를 베낀다**(`Fallible`은 `Err(-2)` — §2.5) | 수용 A·B·**C**·D·G·H·I·**L** |
| **E3** | 오라클 값 + C 호스트 + import 가드 | 수용 J·K |
| **E4** | 문서: `LANGUAGE.md` · `language_gaps.rs` · `HOST_ABI.md` · 슬라이스 색인 | §2.4의 대가가 적힌다 |

---

## 7. 정직한 반론 (기록)

- **§7이 이것을 강제하지 않았다.** §9-46에서 **결합** 잣대로 열리는 것이지 능력 잣대가 아니다.
  같은 표본의 R6(한국어 라벨)도 결합이고, 그쪽은 **기록된 결정**(DP-S4)이라 이 문서가 다루지 않는다.
- **DP-S3를 네 번째로 여는 것이다**(연결 #108 · 길이 #224 · 부분문자열 #230 · 그리고 비교).
  여전히 **한 번에 한 연산씩**이다.
- **더 싼 대안이 있다.** `starts_with`는 실패할 수 없어 `!`를 요구하지 않는다. **그것을 고르면
  이 문서의 §2.4·§2.5가 통째로 사라진다** — 대신 중간 스팬과 합성을 포기한다. DP-C1이 그 선택이다.

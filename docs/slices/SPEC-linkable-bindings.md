# SPEC — 링크 가능한 바인딩 (import library + C++ 컴파일)

**상태: 초안 · 사용자 확인 대기.** 확인 전에는 구현하지 않는다(`CLAUDE.md` 개발 방법론).
**결정 근거:** `STATUS.md` §9 D1 — 사용자 결정(2026-09-03): 다음 슬라이스는 배열도 struct도 아닌
**ABI 정합**이다.

---

## 0. 왜 이것인가 (실측)

§9-1은 남은 두 후보(배열 IN · 고정 레이아웃 struct)가 **어느 쪽도 실측상 강제되지 않는다**고
결론냈다. 그 자리에서 2026-09-03 감사가 **제품에 더 가까운 공백**을 찾았다.

### 0.1 생성 `.h`는 링크 가능해 보이지만 아니다 (실측)

```
grep -n "dllimport|declspec|\.lib" compiler/src/header.rs compiler/src/emit.rs compiler/src/codegen.rs
  -> 0건
compiler/src/emit.rs:287  let names = ["{m}.dll", "{m}.h", "{m}.pas"]      <- .lib이 없다
```

생성 헤더는 `double mlx_discount(double, bool);` 같은 **평범한 프로토타입**을 내놓는다. 그런데
함께 배달되는 import library가 없으므로, 헤더를 include하고 평범하게 호출하는 C 호스트는
**링크 단계에서 깨진다**. 참조 호스트가 그것을 겪지 않는 이유는 선언을 `_Generic` 타입 오라클로만
쓰고 실제 호출은 `GetProcAddress`로 하기 때문이다(`hosts/c-host/host.c`).
`compiler/src/header.rs:76-79`의 주석이 이미 *"Not verified: … link-time binding via an import
library"* 라고 적는다 — 알고 있었고, 닫히지 않았다.

> **이것이 D06(호스트 비종속)·D14(공식 쌍 = Delphi + C)의 서사에 직접 닿는다.** 오늘 "C 호스트가
> 붙는다"의 증명은 **동적 로딩 한 방식뿐**이고, C/C++ 세계에서 더 흔한 방식(헤더 + `.lib` 링크)은
> 한 번도 성립한 적이 없다.

### 0.2 그런데 import library는 **이미 만들어지고 있다** (실측, 이 슬라이스가 작은 이유)

```
target/release/discount_fixture.dll.lib      2,152 B
target/{debug,release}/deps/*.dll.lib
```

`cargo build --crate-type cdylib`이 `x86_64-pc-windows-msvc`에서 `<crate>.dll.lib`을 함께 낸다.
`emit_artifacts`는 그것을 **복사하지 않을 뿐**이다. 따라서 이 슬라이스의 큰 절반은 codegen이
아니라 **패키징**이다.

> ⚠ 위 수치는 워크스페이스에 남아 있던 **fixture** 산출물에서 잰 것이다(2026-08-28). 구현 시
> **오늘의 파이프라인이 같은 파일을 내는지 먼저 확인**하고, 그 결과를 §3-A의 수용 기준으로 삼는다.
> 이름(`<crate>.dll.lib` vs `<crate>.lib`)도 그때 고정한다.

### 0.3 C++는 한 번도 컴파일된 적이 없다 (실측)

```
compiler/src/header.rs:178-180, 209-212   ->  #ifdef __cplusplus / extern "C" 방출
grep -rn "g\+\+|clang\+\+|/TP" (트리 전체)  ->  0건
docs/HOST_ABI.md:208-209                   ->  우선순위 표에서 C / C++ 는 공동 1위
```

헤더가 C++ 가드를 방출하는데 그 경로를 컴파일한 적이 없다. 공동 1위 호스트의 **절반이 미검증**이다.

---

## 1. 범위

**포함**

1. `mlc build`가 **네 번째 산출물** import library를 낸다.
2. 생성 `.h`가 **링크 타임 바인딩에 유효**함을 실제 링크로 증명한다 — `GetProcAddress` 없이
   호출하는 C 호스트.
3. 생성 `.h`가 **C++로도 컴파일**됨을 증명한다(`cl /TP /W4 /WX`).

**미포함 (명시)**

- `__declspec(dllimport)` 방출 — §4 DP-L2에서 **넣지 않기로** 판단한다(근거 아래).
- `.so`/ELF 쪽 대응(D22 미개시, `STATUS.md` §4-7).
- MSVC 외 C 컴파일러(clang-cl·MinGW)와 `g++`/`clang++` — §5로 보낸다. 이 슬라이스는
  **이미 CI에 있는 툴체인**만 쓴다.
- Delphi 링크 방식(생성 `.pas`는 `external ML_MODULE` 암시적 임포트로 이미 링크 타임 바인딩이다 —
  그러나 `dcc64`가 없어 여전히 미검증, `STATUS.md` X1).

---

## 2. 계약

### 2.1 산출물

`mlc build <f.mls> -o <dir>` →

| 파일 | 무엇 | 상태 |
|---|---|---|
| `<module>.dll` | 네이티브 모듈 | 기존 |
| `<module>.h` | C 헤더 | 기존 |
| `<module>.pas` | Delphi import unit | 기존 (DRAFT) |
| **`<module>.lib`** | **MSVC import library** | **신규** |

**원자성은 기존 규약을 따른다**(`emit.rs`): 넷을 스테이지에 다 만든 뒤 옮기고, 이동이 실패하면
되돌린다. **부분 집합이 `out_dir`에 남지 않는다** — 오늘 셋에 적용되는 불변식이 넷으로 넓어질 뿐이다.

### 2.2 호스트가 고를 수 있는 두 결합 방식

| 방식 | 무엇이 필요한가 | 모듈 교체 시 |
|---|---|---|
| **동적**(오늘) | `.dll` + `.h` | 호스트 재빌드 불필요. 지문 게이트가 드리프트를 **거부**한다(#105) |
| **정적 링크**(신규) | `.dll` + `.h` + `.lib` | 호스트 재빌드 불필요(같은 `.lib` 계약이면). **그러나 게이트를 부르지 않으면 아무도 거부하지 않는다** |

> ⚠ **정적 링크는 보호를 약화시키는 방향이 아니라, 게이트를 건너뛰기 쉬운 방향이다.**
> `SPEC-iface-hash.md` §5.1이 이미 적은 문제("검사하지 않는 호스트는 보호되지 않는다")가
> 이 방식에서 **기본값**이 된다 — 링크된 심볼은 그냥 해석되기 때문이다. 그래서 §3-D가
> **링크 호스트도 게이트를 부르는 예제**를 요구한다. 대외 문구는 규칙 6을 따른다.

---

## 3. 수용 기준 (측정 가능)

- **A. 산출물.** `mlc build examples/discount.mls -o <dir>` 후 `<dir>`에 `.dll`·`.h`·`.pas`·`.lib`
  **정확히 4개**가 있다. `.lib`의 크기 > 0이고, `dumpbin /linkermember` 또는 `/symbols`가
  `mlx_discount`를 포함한다(자체 PE 리더가 아니라 **외부 도구 교차 확인**).
- **B. 링크 호스트가 값을 낸다.** `GetProcAddress`를 한 번도 부르지 않는 새 C 호스트가
  `#include "discount.h"` + `.lib` 링크로 `mlx_discount(100.0, true) == 90.0`을 얻는다.
  MSVC `cl … discount.lib`로 빌드하고 **실제로 실행**한다.
- **C. C++ 컴파일.** 예제 18개의 헤더 전부가 `cl /TP /W4 /WX`(C++ 모드)로 컴파일된다 —
  수용 D의 C11 게이트와 **같은 코퍼스**다. 실패 시 어느 헤더인지 이름을 낸다.
- **D. 링크 호스트도 게이트를 부른다.** B의 호스트가 `ml_module_abi_version`·`ml_iface_hash`를
  **링크된 심볼로** 호출해 헤더의 `ML_<MODULE>_IFACE_HASH`와 비교하고, 드리프트 모듈에 대해
  **거부한다**(수용 D의 `pack_drift`와 같은 방식, 값으로 확인).
- **E. 회귀 없음.** export 집합 **불변**(3개), 크기는 D3의 범위 assert 안, 기존 수용 A/B/C/D 전부 통과.
- **F. 게이트가 드리프트를 잡는다.** `doc_claims.rs`가 산출물 목록(`emit.rs`)과 문서(`README` 두 판본·
  `HOST_ABI.md`)의 산출물 개수를 묶는다 — 넷 중 하나가 빠지면 실패한다.

---

## 4. 설계 결정

- **DP-L1 — `.lib`은 `mlc`가 만들지 않고 `cargo`가 만든 것을 옮긴다.** 새 도구를 부르지 않는다
  (`lib.exe` 호출 없음). 이유: 그것이 이미 존재하고(§0.2), 링커가 만든 것이 정본이며,
  D19의 "rustc lowering" 경로를 벗어나지 않는다.
- **DP-L2 — `__declspec(dllimport)`를 방출하지 않는다.** 함수 심볼에는 선택 사항이고(없으면 링커가
  thunk를 하나 끼운다), 넣는 순간 **같은 헤더가 모듈 내부 빌드에서는 틀리게 된다**(export 쪽은
  `dllexport`여야 한다). 오늘 헤더는 호스트 전용이므로 굳이 나눌 이유가 없고, 데이터 export는
  존재하지 않는다(export는 함수 3개뿐 — 실측). 성능 차이는 이 슬라이스에서 **주장하지 않는다**.
- **DP-L3 — C++ 검증은 `cl /TP`로 한다.** `g++`/`clang++`는 CI에 없다. 없는 툴체인을 요구하면
  게이트가 빨개지기만 하고(§4-8이 Delphi에서 배운 것), `/TP`는 **이미 있는 컴파일러로 같은 헤더를
  C++ 규칙으로 읽게 하는** 가장 싼 방법이다. 다른 C++ 컴파일러는 §5다.
- **DP-L4 — 새 호스트는 `hosts/c-host`를 고치지 않고 옆에 둔다.** 수용 D의 119개 체크는
  **동적 결합**의 증거다. 그것을 링크 방식으로 바꾸면 증거가 사라진다. 새 파일
  (`hosts/c-host-link/host.c` 가칭)을 두고 두 결합 방식을 **둘 다** 유지한다.

---

## 5. 범위 밖 / 미검증 (명시)

- `clang-cl`·MinGW·`g++`·`clang++` — 실제 다른 컴파일러는 여전히 미검증이다. §3-C는
  **MSVC의 C++ 모드**만 닫는다. `header.rs:76-79`의 "Not verified: other C compilers"는 그대로 남는다.
- `.so`/ELF와 `dlsym`(D22 미개시).
- Delphi 링크(`dcc64` 부재, X1).
- **성능**: 링크 바인딩이 `GetProcAddress`보다 빠르다고 **적지 않는다**(규칙 7 — 측정 없이 쓰지 않는다).
- `ML_ERR_*`의 모듈 접두어 부재(§5-5.4)는 **여기서 닫지 않는다** — Q14 결정이 선행한다(D2).
  다만 §3-C가 18개 헤더를 한 TU에서 C++로도 읽으므로, 충돌이 생기는 날 **더 빨리** 드러난다.

---

## 6. 열린 질문 (구현 전 확인)

1. `.lib` 파일 이름을 `<module>.lib`으로 할지 `<module>.dll.lib`(cargo가 내는 이름)으로 둘지.
   전자가 사용자에게 자연스럽고 후자가 링커 관례에 가깝다. **권고: `<module>.lib`** — 산출물 이름을
   `<module>.<ext>`로 통일한다.
2. `.lib`이 D23(산출물 라이선스 예외) 범위에 들어간다는 것을 `LICENSE-OUTPUT-EXCEPTION`에
   한 줄로 명시할지. **권고: 명시한다** — 그 문서가 산출물을 열거하고 있다.

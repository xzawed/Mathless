# STATUS — 세션 핸드오프 (현재 상태 · 다음 작업)

> 새 세션은 이 문서를 **먼저** 읽는다. **착수 지점은 §9의 "▶ 여기서 시작한다" 블록이다.**
> 측정값은 그 시점 `main` 기준이며, `git log`·`docs/slices/README.md`·각 SPEC이 정본이다.
>
> 이 문서는 **현재와 다음**만 담는다. 닫힌 항목은 한 줄로 남고, 그 서사와 날짜 붙은 기록은
> `docs/history/status-<절>.md`(2026-09-25에 이 문서에서 옮긴 원문)와 세션 기록 §9-N(색인은
> [`HISTORY.md`](HISTORY.md), 원문은 `docs/history/9-N.md`)에 있다. **Read 한 번에 들어가야 한다** —
> `doc_claims.rs`의 `status_fits_in_one_read`가 크기를 잰다. 넘치면 지우지 말고, 닫힌 항목을 한 줄로 접어
> 서사를 옮긴다.
>
> **가장 최근에 잰 값은 `HISTORY.md` 맨 위 스텁이 가리키는 기록에 있다.** 이 문서와 다르면 그쪽이 최신이다 —
> §9-N은 날짜 붙은 기록이라 나중에 고쳐 쓰지 않는다. 이 머리말의 이전 판은
> [`history/status-1.md`](history/status-1.md).

## 1. 현재 상태 (실측, `main`)

- **저장소는 공개다 (2026-08-30).** `https://github.com/xzawed/Mathless` — 익명 접근 `HTTP 200`,
  API `"visibility": "public"`. 공개 전 보안·개인정보 감사를 실측으로 마쳤다(§5c). **이제 커밋 메시지와
  PR 본문은 쓰는 즉시 공개된다.**
- **테스트:** `MATHLESS_GATE_D=require MATHLESS_GATE_FPC=require MATHLESS_GATE_FPC_HOST=require cargo test --workspace --locked` — **0 fail / 0 ignored**,
  `clippy -D warnings`·`fmt`도 종료 코드 0.
  > **개수·줄 수·체크 수는 여기 적지 않는다** — 원천은 실행 결과다(전역 규칙의 SSOT). 여기 적어 둔
  > 값은 슬라이스마다 낡았다. 날짜 붙은 값은 §9-N 기록에 두고, 오늘 값은 아래 명령을 **파이프 없이**
  > 돌려 `$?`를 본다. 낡아 온 경위는 [`history/status-1.md`](history/status-1.md).
  > ⚠ **`fmt`·`clippy`는 종료 코드로 확인한다.** `cargo fmt --all --check | tail -1 && echo clean`은
  > **파이프라인의 종료 코드(=`tail`의 0)** 를 보므로 언제나 "clean"을 찍는다 — 실제로 그렇게 통과시킨
  > 커밋이 CI에서 잡혔다(2026-09-02). 실패할 수 없는 가드는 아무것도 증명하지 않는다.
- **CI 두 잡:** `windows-latest`가 **정본**(수용 A/B/C/D 실행, `MATHLESS_GATE_D=require`로 skip 금지),
  `ubuntu-latest`는 프런트엔드 보험(수용 테스트는 `cfg(windows)`라 거기선 컴파일되지 않는다).
  툴체인 핀 `rust-toolchain.toml` = 1.97.1.
- **코드:** 비테스트 Rust(`compiler/src` + `hosts/rust-oracle/src`) + C·Delphi 호스트
  (`c-host` · `c-host-link` · `delphi-host`) + `runtime/ml_abi.h`, 그리고 테스트.
  `src`에 TODO/FIXME 없음. 서드파티 의존성 **0개**(`Cargo.lock`에 로컬 크레이트 둘뿐).
  > **세는 명령을 함께 적는다** — 구성을 안 적으면 다음 사람이 다른 것을 세고 "낡았다"고 판단한다
  > (실제로 그렇게 두 번 낡았다):
  > ```
  > find compiler/src hosts/rust-oracle/src -name '*.rs' | xargs wc -l | tail -1
  > wc -l hosts/c-host/host.c hosts/c-host-link/host.c hosts/delphi-host/host.dpr | tail -1
  > find compiler/tests hosts/rust-oracle/tests -name '*.rs' | xargs wc -l | tail -1
  > ```
- **수용 A/B/C/D 전부 통과.** **CI가 강제하는 호스트 게이트는 C와 Free Pascal 둘이고, Delphi는 로컬 전용이다**
  — MSVC로 빌드한 C11 호스트가 산출 DLL을
  `LoadLibrary`/`GetProcAddress`로 로드·호출한다(`hosts/c-host/host.c`, **로드하는 모듈 전부**에
  지문 게이트가 붙는다 — 개수는 `host.c`가 정본이라 여기 적지 않는다).
  **체크 개수는 여기 적지 않는다(2026-09-11).** 게이트를 돌려 세고, 세는 명령은 아래 블록에 있다.
  > 체크 수는 호스트가 **실제로 찍은 줄**을 센다 — `grep -c '^  ok   '`. 소스의 `grep -c 'check('`는
  > 루프 안의 한 호출·정의부·주석까지 걸려 **다른 값**을 준다. 재현 명령을 적을 때는 그 명령을 실제로
  > 돌려 값을 옮긴다(경위는 [`history/status-1.md`](history/status-1.md)).
  **Delphi는 2026-09-07에 처음 실측됐고**(§9-15 — 한 번, 손으로), **2026-09-09에 로컬 게이트가 됐다**(§9-20):
  `dcc64`가 명령줄 빌드를 거부하면 게이트가 `bds.exe -b`로 IDE 빌드를 부르고, Community Edition이
  그것은 막지 않는다 — `MATHLESS_GATE_DELPHI=require`가 이 머신에서 통과한다.
  **Free Pascal은 CI 게이트다**(§9-21·§9-22): `MATHLESS_GATE_FPC`가 생성 유닛을, `MATHLESS_GATE_FPC_HOST`가
  Pascal 호스트의 로드·호출을 `windows-latest`에서 required로 지킨다. **그러나 `-Mdelphi`는 방언
  에뮬레이션이라 Delphi 게이트를 대신하지 못한다**(§9-23이 그 증거를 가장 선명하게 적는다 — `string`의
  기본 뜻이 갈린다). **그 갈림은 이제 CI에서도 잰다(2026-09-25)** — 같은 호스트가 `-Mdelphiunicode`로 한 번
  더 돌아 Delphi의 답(status 1)을 낸다. **CI에 도는 Delphi 게이트는 없고, 여기서 닫을 수도 없다** — 러너에 Delphi도
  대화형 세션도 없다. 생성 `.pas`의 문구는 그 사실들을 함께 적는다.
- **코퍼스:** **예제 전부**의 헤더가 `cl /W4 /WX`로 컴파일되고(C11 한 번역 단위 + C++ `/TP` 헤더당
  한 TU), 그중 **일부**만 로드까지 되어 게이트를 통과한다. **두 수치가 다른 것은 의도적이다** —
  N1은 "헤더가 유효한 C인가"를 전부로 넓혔고, 동작은 여전히 Rust 오라클이 덮는다.
  > 개수는 적지 않는다 — 게이트가 `examples/`를 열거해 스스로 센다(N1·A5). 세려면
  > `ls examples/*.mls | wc -l` · `ls docs/slices/SPEC-*.md | wc -l`.
  > **`examples/refund.mls`는 일부러 충돌한다** — `shapes.mls`와 같은 `E_NEG`를 다른 값으로 선언한다.
  > 접두어(Q14) 전에는 이 코퍼스가 `C4005`로 빌드에 실패했다. **지우지 말 것**: 그것이 픽스처다.
- **두 번째 소비 경로 (2026-09-03):** `hosts/c-host-link`가 `GetProcAddress` 없이 `.lib`을 링크해
  호출하고, **드리프트 모듈은 exit 3으로 거부**한다. 동적 경로(`hosts/c-host`)와 **둘 다 유지**한다 —
  한쪽으로 바꾸면 다른 쪽의 증거가 사라진다(DP-L4).
- **산출물:** `mlc build <f.mls> -o <dir>`가 쓰는 파일은 `README.md`와 `HOST_ABI.md`가, 모듈
  크기와 export 집합의 측정은 `SECURITY.md`가 정본이다 — 여기 다시 적지 않는다. 크기는 머신마다
  `FileAlignment` 한 블록만큼 달라(핀은 rustc를 덮지 MSVC `link.exe`를 덮지 않는다) 한 값을 프로젝트
  상수처럼 적지 않고, 512 B 단위로 양자화돼 신호가 되지도 못한다(§5-5.1).
- **라이선스:** Apache-2.0 OR MIT 이중.

## 2. 오늘 컴파일되는 언어

**정본은 [`LANGUAGE.md`](LANGUAGE.md)의 "현재 구현된 표면"이다 — 여기 요약하지 않는다.** 여기 있던 요약은
그 정본의 사본이었고, 배열(§9-25·§9-29)과 `byte_len`·`byte_slice`(§9-40·§9-44)가 들어온 뒤에도 그것들을
"없는 것"으로 적고 있었다(2026-09-25 발견). 옛 요약은 [`history/status-1.md`](history/status-1.md).

## 4. 사용자 결정 대기

측정은 끝났고 판단만 남은 것들이다. **혼자 정하지 않았다.** 여기에는 열린 항목만 있다.

**번호는 고정한다** — 닫힌 결정은 같은 번호로 [`history/status-closed-001.md`](history/status-closed-001.md)에
옮기고, "§4-6" 같은 인용은 거기서 풀린다. 결정 과정과 실측의 본문은 [`history/status-4.md`](history/status-4.md).

5. **홈페이지 URL** — 여전히 `null`이고, **그대로 둔다.** 가리킬 것이 없다: 이 프로젝트에는 문서
   사이트가 없고, 저장소 URL을 저장소의 홈페이지로 적는 것은 순환이다. **웹 UI 전용도 아니었다** —
   `gh repo edit --homepage <URL>`로 언제든 된다. 문서 사이트가 생기면 그때 한 줄이다.

## 5. 기록된 부채 — 조용히 사라지면 안 되는 것

> 여기에는 아직 남은 부채만 있다. 갚은 부채는 같은 번호로 [`history/status-closed-001.md`](history/status-closed-001.md)에
> 옮기고, "§5-1" 같은 인용은 거기서 풀린다. 발견·상환의 실측과 경위는
> [`history/status-5.md`](history/status-5.md)(§5-1~§5-5)와 [`history/status-5-6.md`](history/status-5-6.md)(§5-6).

2. **`while`은 호스트 스레드를 정지시킬 수 있다.** MVP가 막을 수 없어 `HOST_ABI.md`의 **생존성 계약**으로
   다뤘다. `SECURITY.md`에는 넣지 않았다(그 문서의 위협 모델은 배포된 알고리즘). **우회 가능한 반쪽
   방어를 넣고 안전하다고 쓰지 말 것.** (SPEC-while §5.1)
3. **재귀는 금지다.** 정적으로 판정 가능하고 결과가 프로세스 종료이기 때문이다. **조용히 완화하지 말 것** —
   푸는 것은 가산적 변경이며 SPEC 갱신이 따라야 한다. (SPEC-calls §5.1)
4. **생성 모듈의 패닉은 프로세스를 죽이지 않고 호출한 스레드를 무한히 돌린다.** ↓ §5-4

### 5-4. 생성 모듈의 패닉은 프로세스를 죽이지 않는다 — 스레드를 무한히 돈다

여러 문서가 `i32 /0`을 두고 "`no_std`+`panic=abort`니까 **호스트를 죽인다**"라고 적어 왔다.
**코드를 읽어 보니 틀렸다.** 생성 크레이트가 실제로 담는 것은:

| 무엇 | 어디 | 값 |
|---|---|---|
| no_std | `compiler/src/codegen.rs:39` | `#![no_std]` |
| 패닉 핸들러 | `compiler/src/codegen.rs:58` | `#[panic_handler] fn ml_panic(_: &core::panic::PanicInfo) -> ! { loop {} }` |
| 패닉 전략 | 생성 `Cargo.toml`(`codegen.rs:286`) | `[profile.release] panic = "abort"` |

`no_std`에서는 **`#[panic_handler]`가 곧 패닉 런타임**이다. `panic = "abort"`는 언와인딩 테이블을
없앨 뿐, `abort()`를 부르는 것이 아니다. 그래서 패닉은 `ml_panic`으로 들어가 **`loop {}`에 갇힌다** —
호출한 호스트 스레드가 그 자리에서 영원히 돈다. 프로세스는 살아 있고, 그 스레드만 죽지도 돌아오지도
않는다.

**둘 다 측정된 적이 없고, 오늘은 측정할 수도 없다.** 지금 언어의 어떤 연산도 패닉에 도달하지 못하기
때문이다: `f64` 0나눗셈은 `inf`, `as`는 포화, 정수 산술은 wrap — `compiler/src/codegen.rs`가 `i32`의
`+ - *`에 `wrapping_add`/`_sub`/`_mul`을 **명시적으로 방출**하므로 프로필 설정과 무관하다
(`-i32::MIN == i32::MIN`으로 **실측**됨 — SPEC-unary, §9-55.4) — 그리고 `i32`의 `/`·`%`는 **가드를 달아**
방출된다(#76 — 제수가 0이면 0을 돌려주고 `i32::MIN / -1`은 `wrapping_div`로 간다).
`#[panic_handler]`는 오늘 `no_std`를 만족시키려고 있을 뿐 **도달 불가능한 코드**다.
따라서 위 서술의 근거 수준은 **E1(소스 판독 + Grok 교차확인)이며 E2가 아니다.**

**이것이 §3a의 논거를 바꾼다.** 결과가 "프로세스 종료"가 아니라 "스레드 정지"이므로, `i32 /0`은
**재귀와 같은 등급이 아니라 `while`(§5-2)과 같은 등급**이다. 그리고 저장소는 `while`을 메커니즘이
아니라 `HOST_ABI.md`의 생존성 계약으로 처리했다. `i32 /0`을 그보다 강하게 다뤄야 한다면 그 근거는
"더 치명적이라서"가 아니라 **"런타임에 판정 가능해서"**여야 한다. 설계 선택지 자체는 그대로다.

## 6. 하지 말 것

- **`packager/`·빈 `backend/` 크레이트 생성 금지** — 아직 조기 경계다.
  (**`hosts/delphi-host`는 2026-09-02에 생겼다** — 크레이트가 아니라 `hosts/c-host`와 같은
  소스 + skip-게이트 하네스이고, 사용자가 Delphi 설치를 결정하면서 미리 세워 둔 것이다. §4-8.)
  `hosts/c-host`처럼
  **실제로 쓰이는 것**만 만든다.
- **D22(`.so`/ELF) 미개시** — 명시적 결정 필요.
- **수용 D를 언어보다 뒤처지게 두지 말 것** — 새 구문을 넣으면 `hosts/c-host/host.c`에도 추가한다.
  지금까지 모든 슬라이스가 그렇게 했다. 체크 개수는 적지 않는다 — 필요하면 게이트를 돌려 센다(§1).
- **`DECISIONS.md`는 사용자 확인 없이 바꾸지 않는다**(규칙 8).
- **생성 산출물은 ASCII로 유지** — 비-ASCII는 MSVC에서 C4819를 내고 `/WX` 빌드를 깬다(실측). 테스트가 고정.
- **아직 없는 것을 현재형으로 쓰지 말 것** — **저장소는 이미 공개다.** 계획은 계획으로 표시한다(§5b).
  이제 커밋 메시지도 PR 본문도 쓰는 즉시 공개된다. 고쳐 쓸 기회가 없다고 생각하고 쓴다.
- **`README.md`와 `README.ko.md`는 한 커밋에서 같이 고친다** — 둘은 번역이 아니라 **같은 주장의 두
  판본**이다. 한쪽만 고치면 다른 쪽이 조용히 낡는다. 뱃지·측정값·언어 기능 표가 특히 그렇다.

## 7. 작업 방식 (효과가 확인된 것)

> 한 줄씩만 적는다. 각 규칙이 나온 사례·수치와 아래 네 복기의 본문은
> [`history/status-7.md`](history/status-7.md)에 그대로 있다(2026-09-25에 옮겼다). **절차와 Grok 사용법의
> 정본은 `CLAUDE.md`다**(개발 방법론 · Grok 협업).

- **다음 슬라이스는 실측으로 고른다.** 후보를 나열하지 말고 **실제 업무 규칙을 오늘의 언어로 써 보고**
  어디서 깨지는지 본다.
- **주장은 측정에 묶는다.** 없는 측정을 테스트로 만들지 않는다(§5-1이 그 예다).
- **"전부"라고 쓰기 전에 저장소 전체를 grep한다 — `*.rs`까지.**
- **문서와 코드를 대조하는 것으로는 부족하다 — 실행해 봐야 한다.**
- **컴파일 여부가 아니라 값을 재라.** 표준 도구는 `mlprobe`다(§9-49) — 스니펫 → 빌드 → 로드 → 호출 →
  **값 출력**까지 한 명령이다:

  ```
  cargo run -p mlc --bin mlprobe -- --src "export fn f(x: f64) -> f64 { return x * 2.0 }"
  ```

- **넓은 변경에는 안전망을 먼저 세우고, 그 안전망을 먼저 적대적으로 검토하라.** 리팩터 뒤에 쓴 테스트는
  그 변경이 안전했다고 말해 줄 수 없다.
- **골든 파일은 "넓은 변경"을 검토 가능하게 만드는 도구다.** 불변량이 아니라 **검토 장치**로 쓴다.
- **텍스트를 고정하는 테스트는 그 텍스트가 유효한지 모른다.** 값이나 빌드로 재는 테스트가 하나는 있어야 한다.
- **팬아웃 실측은 착수 근거를 뒤집을 수 있다 — 그것이 팬아웃의 값이다.**
- **프로토콜을 여는 슬라이스는 가장 작은 것으로 골라라.**
- **보호 프록시는 절대값이 아니라 비교로 측정한다** — 같은 방식으로 빌드한 대조 모듈과의 집합 비교로.
- **가드는 산출물을 지켜라 — 그것을 만드는 코드가 아니라.** 방출기를 호출해 **나온 텍스트**를 본다.
- **조건 뒤에 숨은 검사는 실패하지 않고 건너뛴다.** 게이트가 필요하면 그 게이트 자체를 assert로 만든다.
- **개명은 assert를 조용히 무력화한다.** 넓은 개명 뒤에는 "이 assert가 아직 실패할 수 있나"를 따로 묻는다.
- **바닥값은 코퍼스에서 유도한다.** 손으로 적은 하한은 코퍼스가 늘 때마다 저절로 헐거워진다.
- **가드는 사본의 존재를 요구하지 않는다.** 사실마다 집 한 곳만 "적어야 한다"를 두고, 나머지는 "적었다면
  맞아야 한다"로 검사한다 — 존재를 요구하면 사본을 지우는 올바른 수리가 빨개진다(§9-66, #316).
- **결함을 심기 전에 커밋한다.** `git checkout --`로 되돌리면 커밋하지 않은 수정도 함께 사라진다(§9-66.6).
- **§7은 모듈 안의 값만 재지 않는다 — 경계도 잰다.** 우회가 맞는 값을 내도, 그 약속이 헤더와 지문에 나가는지
  본다. 상태 머신의 벽은 모듈 안이 아니라 거기 있었다(§9-67.2).

### 7-1. 검증 절차 감사 (2026-09-03) — 왜 검증이 반복해서 헛돌았나 → [본문](history/status-7.md)
### 7-3. 한 세션을 실측으로 복기했다 (2026-09-11) — **네 가지가 반복해서 값을 잃었다** → [본문](history/status-7.md)
### 7-2. REVIEW ONLY가 지켜지지 않았다 — 그리고 도구는 그 사실을 보고하지 않았다 (2026-09-05) → [본문](history/status-7.md)

## 8. 이력

- **세션 기록(§9-N):** 색인은 [`docs/HISTORY.md`](HISTORY.md)(한 줄 스텁, 최신이 맨 위), 원문은
  `docs/history/9-N.md`(2026-09-25 분할). 2026-09-22까지 이 파일 §9 안에 있었다.
- **슬라이스 색인:** `docs/slices/README.md` — 각 슬라이스의 상태·내용·SPEC/구현 PR 번호.
- **phase 계획:** `docs/phase1/SPEC.md` + `WBS.md`(W0~W17).
- **2026-08-29 ~ 09-05 세션 요약(PR #34~#144):** [`history/status-8.md`](history/status-8.md).

## 9. 새 세션 시작 절차

1. 이 문서 → `README.md`(문서 지도) → `docs/slices/README.md`(슬라이스 색인) → `CLAUDE.md`(규칙).
   이 줄이 세션 시작 문서의 정본이다 — 각각 Read 한 번, 합계 100 KB(`doc_claims.rs`의
   `the_session_start_documents_fit_in_one_read`가 이 줄에서 목록을 뽑는다). 나머지는 지도에서 필요할 때 연다.
2. 실측부터 한다 — **CI가 요구하는 게이트 셋을 전부 건다.** 하나만 걸면 나머지는 skip되고
   스위트는 그대로 초록이라, Pascal 쪽 회귀를 못 본다(*a skipped gate is not a passed gate*).

   ```powershell
   $env:MATHLESS_GATE_D="require"; $env:MATHLESS_GATE_FPC="require"; $env:MATHLESS_GATE_FPC_HOST="require"
   cargo test --workspace --locked
   ```

   Git Bash에서는 한 줄로:
   `MATHLESS_GATE_D=require MATHLESS_GATE_FPC=require MATHLESS_GATE_FPC_HOST=require cargo test --workspace --locked`.
   목록의 정본은 `.github/workflows/ci.yml`(windows 잡 Test 단계의 `env:`)이다.
3. **바로 아래 "▶ 여기서 시작한다" 블록으로 간다** — 그 한 블록이 오늘의 착수 지점과 추천 순서를
   담고 있다. 그 아래 표에는 **열린 행만** 있다 — 닫힌 행은 같은 번호로
   [`history/status-closed-001.md`](history/status-closed-001.md)에 있고, 경위는 [`history/status-9.md`](history/status-9.md).
4. **언어 슬라이스를 고를 때만** §7대로 업무 규칙을 실제로 써 본다 — 그 방법이 후보 표의 근거를
   **세 번** 뒤집었다(§3a-8 · §3a-9 · §9-1). **표의 근거를 인용만 하고 착수하지 말 것.**

### 다음 세션이 바로 집을 수 있는 것

> ## ▶ 여기서 시작한다
>
> ### 2026-09-26 기준 — **상태 머신 예제가 경계의 공백을 쟀다: 상태 번호는 헤더와 지문 밖에 있다 (§9-67)**
>
> 문서 정합 둘을 닫았고(#322), 상태 머신 예제(`examples/order.mls`, #323)를 쓰면서 §7을 새 도메인으로 돌렸다.
> 모듈 안에서는 전부 동작하지만, 상태 번호를 바꿔도 헤더와 지문이 바이트 단위로 같다 — 옛 번호로 빌드된 호스트가
> 게이트를 통과하고 잘못 읽는다(`HOST_ABI.md` "인터페이스 지문").
>
> **▶ 다음 세션 — 여기서 시작한다.** 순서대로:
>
> 1. **상수 선언 SPEC 확인 — 사용자 결정.** 슬라이스 선택은 확인받았다(2026-09-26, 권장안). 초안은
>    `docs/slices/SPEC-constants.md`이고 §4의 DP-K1~K8이 확인을 기다린다 — 설계 셋(가시성·타입·이름)은 Grok
>    plan 게이트를 거쳤다. 확인되면 §6의 두 PR로 구현한다.
> 2. **작은 항목**: C# 호스트(Phase 4) · LSP(Phase 3, 큰 일).
> 3. **외부**: `X1`·`X2` — Delphi가 있는 CI 러너. 이 저장소 안에서는 닫히지 않는다.
>
> ⚠ **이 머신의 전체 게이트는 6회 중 2회 실패한다**(§9-63.5) — 실패를 보면 먼저 원인을 읽는다.
> 강한 신호는 CI 두 잡이다.
>
> 📌 **본문은 §9-67이다**(`docs/history/9-67.md`). 그 앞의 2026-09-25 (2) 항목(근본 원인 수리)은
> 바이트 그대로 옮겼다 — 지금은 `docs/history/start-block-048.md`다.
>
>
> ### 그 앞의 항목들 — **본문은 `docs/history/start-block-NNN.md`로 옮긴다 (2026-09-23부터)**
>
> 세션마다 가장 오래된 항목을 **다음 번호의 새 파일** `start-block-NNN.md`로 **바이트 그대로** 옮기고,
> 색인 [`history/start-block-index.md`](history/start-block-index.md)에 한 줄을 더한다(날짜 · 무엇을 했나 ·
> §9-N). 이미 있는 기록 파일은 다시 편집하지 않는다(2026-09-25까지는 `HISTORY.md`의 한 절이었다).
> 그 아래의 착수 대기 표는 날짜 기록이 아니라 **상시 목록**이라 옮기지 않는다 — 닫힌 행만 등록부
> (`history/status-closed-001.md`)로 간다. 이 색인이 생긴 경위(날짜 항목 45개 · 759줄)는 [`history/status-9.md`](history/status-9.md).
>
**아래 목록은 착수 대기다. 결정 대기의 정본은 §4다** — 여기 다시 세지 않는다.

> **번호는 고정한다** — 닫힌 행은 같은 번호로 [`history/status-closed-001.md`](history/status-closed-001.md)에
> 옮긴다(다른 문서와 PR이 "N1"처럼 번호로 가리킨다). 닫힌 행의 원래 서술(근거·실측)은
> [`history/status-9.md`](history/status-9.md).

> **읽는 법.** "바로"는 *사용자 결정도 SPEC도 없이 오늘 시작할 수 있다*는 뜻이다.
> 크기는 이 저장소의 기존 PR에 견준 것이다(#80 = 하루치 슬라이스, #113 = 반나절 수정).

#### 외부 조건이 선행하는 것

| # | 무엇 | 조건 |
|---|---|---|
| **X1** | **Delphi 검증 — D14의 나머지 절반** | **남은 것은 CI뿐이다** — 러너에 Delphi도 대화형 세션도 없다. 실체는 2026-09-07에 측정됐고(§9-15), 로컬 게이트도 닫혔다(§9-20: `dcc64`가 명령줄 빌드를 거부하면 `bds.exe -b`로 IDE 빌드, `MATHLESS_GATE_DELPHI=require` 초록). **이 개발 PC를 자체 호스팅 러너로 쓰는 안은 쓰지 않는다**(2026-09-24, Grok 동의·사용자 확인 — 공개 저장소라 포크 PR이 그 러너에서 코드를 실행할 수 있다). **CI가 볼 수 있는 몫은 넓혔다**(2026-09-25, #306): FPC 호스트 게이트가 `-Mdelphiunicode`로 한 번 더 돌아 Delphi의 `string` 함정(§9-23)을 잰다 — 그래도 Free Pascal이므로 **X1은 닫히지 않는다**. 경위 → [`history/status-9.md`](history/status-9.md) |
| **X2** | **생성 `.pas`의 Delphi 하류 게이트** | **X1과 같은 조건이다.** 생성 유닛은 FPC가 CI에서 매 푸시 컴파일하고(§9-11), Delphi는 로컬 게이트에서 반복 가능하게 컴파일한다(§9-15·§9-20). **남은 것은 CI뿐이고, 여기서 못 닫는다.** 경위 → [`history/status-9.md`](history/status-9.md) |

## 9-N 세션 기록 — [`HISTORY.md`](HISTORY.md)로 옮겼다 (2026-09-22)

날짜 붙은 세션 기록은 [`docs/HISTORY.md`](HISTORY.md)가 색인이고(한 줄 스텁, 최신이 맨 위) 원문은
`docs/history/9-N.md`다(2026-09-25 분할) — 날짜 붙은 최신 측정값은 맨 위 스텁이 가리키는 기록이다.
**§9-N을 인용하는 문장은 그대로 유효하다**: 인용은 스텁 표제로 떨어지고 `doc_claims.rs`의
`every_cited_history_entry_has_a_heading`이, 스텁과 파일의 짝은 `the_history_archive_is_indexed_and_numbered`가
강제한다. 옮긴 경위는 [`history/status-8.md`](history/status-8.md).

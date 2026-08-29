# Contributing / 기여 가이드

> English first, 한국어 아래. This project works **PR-first**: `main` is never committed to directly.

## Workflow (English)

1. **Never commit to `main` directly.** Every change lands via a Pull Request.
2. Branch from `main` using a typed prefix:
   - `docs/*` — documentation / design docs
   - `feat/*` — new implementation
   - `fix/*` — bug fixes
   - `chore/*` — tooling, meta, housekeeping
3. Keep a PR scoped to one concern. Reference the decision/question it touches
   (e.g. `D16`, `Q12`).
4. **Do not overturn a decision in `docs/DECISIONS.md`** without first writing the rationale
   and trade-off into that file (see [CLAUDE.md](CLAUDE.md)).
5. **Evidence level** must match the phase (per CLAUDE.md):
   - Phase 0 (docs) → **E0** (doc consistency) / **E1** (cited external facts). No fabricated
     measurements.
   - Once code exists → **E2** (real build/run artifacts) is required before "done".
6. **Second-order verification via Grok** is required for implementation / diagnosis / code
   review before a PR is marked complete. If a Grok tool fails, report it and ask — do not
   silently self-substitute.
7. Squash-merge into `main`. Delete the branch after merge.
8. **CI** (GitHub Actions, `windows-latest`) runs `cargo fmt --check` + `clippy -D warnings`
   + `cargo test --workspace` on every PR — the authoritative gate that actually executes the
   `#![cfg(windows)]` acceptance tests. Keep it green before merge. (Cross-platform SO/ELF CI
   is deferred with D22.)

## Methodology: SDD + WBS + TDD (mandatory from Phase 1)

1. **SDD (spec-first):** write the spec before code — `docs/slices/SPEC-<name>.md` with inputs, outputs,
   contracts, and measurable acceptance criteria. Get user confirmation before implementing.
2. **WBS:** break the spec into PR-sized tasks in `docs/phaseN/WBS.md` (the phase plan, alongside `docs/phaseN/SPEC.md`), in dependency order; each
   task = one PR with a measurable done-criterion.
3. **TDD:** write the failing test first (Red → Green → Refactor). Test results are the E2 evidence.
4. **Grok + measured data (both doing and reviewing):** perform and verify each task with Grok,
   grounded in real data (test results, build artifacts, export dumps, run logs). `grok_build_verify`
   is required before a PR is marked complete.

Order: **SPEC → (user confirm) → WBS → per task [failing test → implement → pass → Grok verify] → PR → merge.**
Never write "it works / it's fast / it's protected" without a measurement.

## Positioning rule

Never describe the protection as "impossible to reverse". The honest phrasing is
**"raises the cost of analysis and tampering"** (see `docs/SECURITY.md`, decision D05).

---

## 워크플로 (한국어)

1. **`main`에 직접 커밋 금지.** 모든 변경은 Pull Request로만 반영한다.
2. `main`에서 분기하고, 접두어로 종류를 표시한다:
   - `docs/*` — 문서 / 설계 문서
   - `feat/*` — 신규 구현
   - `fix/*` — 버그 수정
   - `chore/*` — 도구·메타·정리
3. PR은 한 가지 관심사로 좁힌다. 관련 결정/질문 번호(예: `D16`, `Q12`)를 명시한다.
4. `docs/DECISIONS.md`의 **결정을 뒤집으려면**, 먼저 그 파일에 근거와 트레이드오프를 쓴다
   (자세한 규칙은 [CLAUDE.md](CLAUDE.md)).
5. **근거 수준**은 단계에 맞춘다 (CLAUDE.md 규칙):
   - Phase 0(문서) → **E0**(문서 정합) / **E1**(출처 있는 외부 사실). 없는 측정값 날조 금지.
   - 코드가 생기면 → "완료" 선언 전 **E2**(실제 빌드/실행 산출물) 필수.
6. **Grok 2차 검증**은 구현·진단·코드 검토에서 PR 완료 전 필수. Grok 도구가 실패하면
   보고하고 판단을 구한다 — 임의 대체 금지.
7. `main`에는 squash-merge. 머지 후 브랜치 삭제.
8. **CI**(GitHub Actions)가 모든 PR에서 `cargo fmt --check` + `clippy -D warnings` +
   `cargo test --workspace`를 실행한다. **`windows-latest`가 정본 게이트** — `#![cfg(windows)]`
   수용 테스트를 실제로 돌리며, `MATHLESS_GATE_D=require`로 수용 D(실제 C 호스트)가 조용히
   skip되지 않게 한다. **`ubuntu-latest`는 프런트엔드 보험**(Windows 전용 가정 조기 발견)이지
   권위가 아니다 — 수용 테스트는 거기서 컴파일되지 않는다. 머지 전 둘 다 green 유지.
   (`.so`/ELF **타깃**은 여전히 D22와 함께 이연 — Linux 잡은 D22가 아니다.)

## 라이선스와 기여

이 저장소는 **Apache-2.0 또는 MIT** 이중 라이선스다(`LICENSE-APACHE` / `LICENSE-MIT`).
명시적으로 달리 밝히지 않는 한, 제출된 기여는 Apache-2.0이 정의하는 바에 따라 위와 동일하게
이중 라이선스된다.

## 대외 표현 규칙

보호를 "리버싱 불가능"으로 표현하지 않는다. 정직한 문구는 **"분석과 변조 비용을 높인다"**
이다 (`docs/SECURITY.md`, 결정 D05 참고).

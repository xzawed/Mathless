# OPEN QUESTIONS

구현 전에 닫아야 하는 것과, 미뤄도 되는 것을 나눈다.

## MVP 전에 닫을 것 — 닫힘 (2026-08-28 → DECISIONS D14~D18)

Q1~Q5는 사용자 승인으로 닫혔다. 세부·기각 대안은 `DECISIONS.md`의 "확정 상세" 참고.

### Q1. 주력 호스트 2개는 무엇인가? — 닫힘 → D14

Delphi(1급 데모) + C(ABI 기준). 후보였던 Delphi+C#은 Phase 4로 이연.

### Q2. 표면 문법 계열은? — 닫힘 → D15

중괄호·정적·값타입 우선 C 계열(MVP=struct+함수). "C#-like"는 좁은 라벨. 세부 문법은 계속 미정.

### Q3. 메모리 모델은? — 닫힘 → D16

소유권 3층 분리(인자=빌림 / 반환=caller-allocates / 상태=context handle), 힙 분리. 남은 하위 → Q12.

### Q4. 에러 모델은? — 닫힘 → D17

ABI 정수 상태코드+out-param, 표면 Result는 sugar. 남은 하위 → Q13.

### Q5. 모듈 파일 포맷은? — 닫힘 → D18

표준 DLL/SO + 버전 export 심볼 + 모듈 전용 접두어. 커스텀 컨테이너는 P1. 남은 하위 → Q14.

## 미뤄도 되는 것

- Q6. 코드젠 경로 — **잠정 해결 → D19** (MVP는 rustc lowering). C-emit 후 clang/fpc, LLVM 직접은 재검토 여지로 남김.
- Q7. 클래스 상속을 언제 넣을 것인가?
- Q8. WASM 타깃 시기
- Q9. 서명 키 관리
- Q10. 패키지 이름/확장자 확정 (`.mls`, `.mll`은 가칭)
- Q11. 내부 IR을 실제 Object Pascal 소스에 가깝게 생성할 것인가, 독자 IR인가?
- Q12. 반환 값 소유권 규약: out-buffer(caller-allocates) vs 모듈 소유+명시 free vs 스칼라 반환만 — Phase 2(struct)에서 확정 (D16 파생)
- Q13. 에러 코드 체계 — **닫힘(2026-08-28)**: 사용자 확정으로 **평탄 i32**(`0`=OK / 양수=모듈 정의 도메인 에러 / 음수=예약 런타임·ABI). **D17 에러-경로 슬라이스로 구현·실측**(SPEC PR #14, 구현 PR #15; 오라클 로드로 status/out-param 검증). out-param 세부는 D17대로. **`DECISIONS.md` D17 확정 상세에 반영 완료**(규칙 8상 사용자 승인 후 별도 PR #30) — 이 항목은 더 이상 열려 있지 않다. 기각: HRESULT식(무겁·Windows 중심), 0/음수 무구조(의미 비트 부족).
- Q14. ABI 버전 배치(export 심볼 vs PE/ELF section)와 사용자 모듈 export 접두어 확정(`ml_`는 런타임 예약) — (D18 파생)

## Claude에게

Q1~Q5는 닫혔다. 남은 결정(Q6~Q14)도 제안 시 장단점 표를 쓰고, 문서를 고치기 전에 사용자 확인을 받는다.  
확인 없이 DECISIONS.md를 바꾸지 않는다.

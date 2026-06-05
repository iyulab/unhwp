# unhwp Roadmap

## 히스토리 (완료)

- **v0.1–0.2**: 코어 HWP5/HWPX 파서, CLI, streaming API 기초
- **v0.3.x**: streaming 파이프라인 완성, 섹션 마커, MultiFormatWriter, bugfix 시리즈
- **v0.4.0**: WASM 지원 (`unhwp-wasm`), GitHub Pages 플레이그라운드, npm 패키지 (`@iyulab/unhwp`)

---

## v0.6 — HWPX 렌더링 충실도 (실측 결함 기반)

> 출처: 2026-06-05 HWPX 요구사항정의서 품질 평가 — `claudedocs/plans/quality-assessment-2026-06-05-hwpx-reqdoc.md`
> 우선순위 = 평가에서 도출된 가시적 영향 순

**완료 (Cycles 27–28):**
- **D1 (High) — `hp:tab`/leader 렌더** ✅: `parse_run`이 `hp:tab`을 공백으로 렌더 +
  `render_text_run`이 강조 마커 밖으로 공백 분리. 목차 제목·페이지번호 직결 해소.
- **D2 (High) — floating 이미지 block-level 렌더** ✅: `hp:pic`의 `textWrap`
  (IN_FRONT_OF_TEXT/BEHIND_TEXT) 인식 → floating 이미지(도장/서명/워터마크)를 standalone
  블록으로 분리. 전부 보존(무손실), 텍스트·헤딩 오염 제거. suppression 옵션은 폐기(YAGNI).

**Defer (Cycle 29–30, evidence에 수요 없음):**
- **D3 (Med) — EMF 처리** ⏸️ v0.7+: 순수 Rust 완전 EMF 렌더러 부재(미완성 변환기=기술부채).
  EMF 바이트는 이미 무손실 보존. 변환은 viable 접근 확보 시.
- **D5 (Med) — 표 병합 셀** ⏸️: 실측 rowSpan 0, colSpan만. colspan 빈 셀 폴백은 올바른
  출력. rowspan→HTML 폴백은 rowspan 문서 실수요 시(`has_rowspan()` 게이트).
- **D4 (Low) — BinData 디덤** ⏸️: byte-identical 중복은 reader에게 invisible. enhancement,
  value bar 미달.

**신규 (Cycle 30 발견):**
- **D8 (Med) — `trim_text` 공백 손실**: `section.rs:45` 등 `trim_text(true)`가 `hp:t`
  앞뒤 공백 제거(목차 "1. "→"1."). `trim_text(false)` + whitespace 처리, 회귀 위험으로 별도 사이클.

## v0.5 — 품질 & 커버리지 향상 (잔여)

- **HWP5 이미지 추출 완성**: 바이너리 스트림에서 PNG/JPG 실제 추출
- **HWP 3.x 지원**: `hwp3` 피처 구현 완성 (현재 파서 스텁만 존재)
- **WASM 이미지 API**: `toImages()` — base64 인코딩된 이미지 배열 반환
- **성능 벤치마크**: criterion 기반 100MB 문서 처리 기준 수립

## v0.7+ — 장기 방향

- **Async API**: `tokio` 피처 구현 완성
- **Python 바인딩**: PyO3 기반 네이티브 패키지
- **수식(Equation) 변환**: EQEdit → LaTeX/MathML

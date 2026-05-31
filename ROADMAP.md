# unhwp Roadmap

## 히스토리 (완료)

- **v0.1–0.2**: 코어 HWP5/HWPX 파서, CLI, streaming API 기초
- **v0.3.x**: streaming 파이프라인 완성, 섹션 마커, MultiFormatWriter, bugfix 시리즈
- **v0.4.0**: WASM 지원 (`unhwp-wasm`), GitHub Pages 플레이그라운드, npm 패키지 (`@iyulab/unhwp`)

---

## v0.5 — 품질 & 커버리지 향상

> 우선순위 미정 — 수요에 따라 순서 조정

- **HWP5 이미지 추출 완성**: 바이너리 스트림에서 PNG/JPG 실제 추출
- **표 렌더링 개선**: 병합 셀 HTML 폴백
- **HWP 3.x 지원**: `hwp3` 피처 구현 완성 (현재 파서 스텁만 존재)
- **WASM 이미지 API**: `toImages()` — base64 인코딩된 이미지 배열 반환
- **성능 벤치마크**: criterion 기반 100MB 문서 처리 기준 수립

## v0.6+ — 장기 방향

- **Async API**: `tokio` 피처 구현 완성
- **Python 바인딩**: PyO3 기반 네이티브 패키지
- **수식(Equation) 변환**: EQEdit → LaTeX/MathML

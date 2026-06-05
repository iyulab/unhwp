# unhwp — 핸드오프 문서

> **현재 상태:** v0.5.0 구현 완료(미태깅). HWPX 렌더링 충실도 결함 D1·D2 수정을 **미릴리즈 v0.5.0에 흡수**(2026-06-05 결정, 버전 파일 변경 없음).

## 지금 하고 있는 것

v0.5.0(Cycles 17–26) 위에 실측 결함 D1(tab/leader)·D2(floating 이미지) 수정 완료(Cycles 27–28).
v0.5.0이 아직 태그되지 않아 D1/D2는 v0.5.0의 일부로 흡수(버전 범프 없음).
D3/D4/D5는 evidence 부재로 defer, D8(`trim_text` 공백 손실) 신규 발견.

## 다음에 해야 할 것

### 옵션 A: v0.5.0 릴리즈 (수동)

v0.5.0 코드는 모두 커밋 완료(Cycles 17–26 + D1/D2 흡수, 최신 `ef0d256`). **태그·푸시만 남음**:

```
git tag v0.5.0
git push && git push --tags
```

릴리즈 후 GitHub Actions가 자동으로: CI 빌드 검증 / npm 배포(`@iyulab/unhwp@0.5.0`) / GitHub Pages 업데이트.
최초 1회 설정: GitHub Pages "GitHub Actions" 소스 활성화, NPM_TOKEN 시크릿 등록.

### 옵션 B (개발 다음 작업): D8 — `trim_text` 공백 손실 수정

`src/hwpx/section.rs:45`(및 mod.rs:329, header.rs:12, styles.rs:18) `reader.config_mut().trim_text(true)`가
`hp:t` 텍스트의 앞뒤 공백을 제거(목차 "1. "→"1." → `**1.**바코드` 직결). render는 무결(Cycle 30 진단 확정).

- 방향: `trim_text(false)` + 요소 간 들여쓰기 whitespace 무시 로직 검토.
- 리스크: 공백 처리 전반 회귀 → 단위 + 스냅샷 회귀 테스트 동반 필수.
- 검증: 실문서 재추출로 `**1.** 바코드`(공백 보존) 확인 + 기존 표/본문 회귀 없음.

### 결함 사이클 결과 (Cycles 27–30, 2026-06-05)

상세: `claudedocs/cycle-logs/cycle-27~30.md`, 평가: `claudedocs/plans/quality-assessment-2026-06-05-hwpx-reqdoc.md`, 로드맵: ROADMAP.md § v0.6

- **완료**: D1(`hp:tab`/leader 렌더) ✅ C27, D2(floating 이미지 block 렌더) ✅ C28
- **Defer (YAGNI — evidence 수요 없음)**: D3(EMF→v0.7+), D5(rowSpan 0이라 colspan 빈 셀이 정답), D4(byte-identical 중복은 invisible)
- **신규**: D8(`trim_text` 공백 손실) → 위 옵션 B
- **비-결함(검증 완료)**: D7(인접 bold `**A****B**`, 본 문서 미발생), D6(frontmatter 제목 불일치 = 원본 메타 부정확, 라이브러리 버그 아님)

## v0.5 완료 목록 (2026-06-01)

| 작업 | 사이클 | 상태 |
|---|---|---|
| Bold/Italic 비트 스왑 수정 | Cycle 17 | ✅ |
| HWP PUA 불릿 렌더링 추가 | Cycle 18 | ✅ |
| parseWithOptions WASM 연결 | Cycle 19 | ✅ |
| CharShape 단위 테스트 + 회귀 테스트 | Cycle 20 | ✅ |
| 테이블 빈 셀 최적화 검증 | Cycle 21 | ✅ (기존 구현 확인) |
| WASM 플레이그라운드 옵션 UI | Cycle 22 | ✅ |
| parse_file_with_options 추가 + Unhwp 빌더 버그 수정 | Cycle 23 | ✅ |
| async_api 옵션 전달 버그 수정 | Cycle 24 | ✅ |
| v0.5.0 버전 범프 (5개 파일) | Cycle 25 | ✅ |

## 알려진 제약

### wasm-pack test Windows 경로 버그
`wasm-pack test --node` 실행 시 Windows에서 경로 이스케이프 오류 발생.
CI(Ubuntu)에서는 정상 작동. 로컬 개발 시 CI 결과로 검증.

### GitHub Pages / NPM 설정 필요 (최초 릴리즈)
- GitHub Pages 소스: Settings → Pages → "GitHub Actions" 활성화
- NPM_TOKEN 시크릿 등록

## 핵심 컨텍스트

### 공개 API 변경 사항 (v0.5.0)

신규 추가된 공개 함수:
- `parse_file_with_options(path, opts: &ParseOptions) -> Result<Document>`
- `parse_reader_with_options(reader, opts: &ParseOptions) -> Result<Document>`
- `parse_bytes_with_options(data, opts: &ParseOptions) -> Result<Document>`
- `async_api::parse_bytes_with_options(data, opts) -> Result<Document>` (async feature)

`Hwp5Parser::parse_with_options()`, `HwpxParser::parse_with_options()` 추가.

### 버전 동기화 대상 파일 (5개 모두 동시 변경)
```
Cargo.toml                           # 0.5.0
cli/Cargo.toml                       # 0.5.0
unhwp-wasm/Cargo.toml                # 0.5.0
bindings/python/pyproject.toml       # 0.5.0
bindings/csharp/Unhwp/Unhwp.csproj   # 0.5.0
```

### 라이브 URL
- 플레이그라운드: https://iyulab.github.io/unhwp/
- npm: https://www.npmjs.com/package/@iyulab/unhwp (NPM_TOKEN 설정 후)

## 로드맵 앵커

→ `claudedocs/cycle-logs/ROADMAP.md` § 미래 방향 (v0.6+)

# unhwp — 핸드오프 문서

> **현재 상태:** v0.5.0 구현 완료(미태깅). HWPX 충실도 결함 D1·D2·D8 + CLI 출력 결함 D9 수정 완료. 버전 라벨(v0.5.0 흡수 vs v0.5.1 패치) **사용자 결정 대기**.

## 지금 하고 있는 것

v0.5.0(Cycles 17–26) 위에 실측 결함을 순차 수정:
- D1(tab/leader)·D2(floating 이미지) ✅ Cycles 27–28
- D8(`trim_text` `<hp:t>` 공백 손실) ✅ Cycle 31 — `section.rs`만 `trim_text(false)`, 실문서로 `**1.** 바코드` 보존 확인, 표/본문 회귀 0. equation/footnote 가드는 Cycle 33에서 discriminating 단위 테스트로 검증(+6 hwpx::section 테스트)
- D9(CLI update 배너 stdout 오염) ✅ Cycle 31 — `println!`→`eprintln!`(stderr), JSON 출력 청결화
- 스윕 검증(Cycle 32–33): HWP5 10문서 + 복잡 HWPX(kproject, `<hp:tab>` 20개) — 인코딩·glue·hard-break 0
- D3/D4/D5는 evidence 부재로 defer

## 다음에 해야 할 것

### 옵션 A: 릴리즈 (수동) — **버전 라벨 결정 필요**

D1/D2는 미릴리즈 v0.5.0에 흡수 결정됨. D8/D9는 그 위에 추가된 **사용자 가시 수정**(fidelity·출력 정확성).
- 권고: (b) **v0.5.1 패치** 분리 — 버그 수정 단위라 정석. 5개 버전 파일 동시 범프(아래 § 버전 동기화) 후 태그.
- 대안: (a) 아직 미태깅이므로 v0.5.0에 함께 흡수.

태그 후 GitHub Actions 자동: CI 빌드 / npm 배포 / GitHub Pages.
최초 1회 설정: GitHub Pages "GitHub Actions" 소스 활성화, NPM_TOKEN 시크릿 등록.

### 옵션 B (개발 다음 작업): CLI stdout 청결 회귀 테스트

D9 재발 방지용 통합 테스트(`assert_cmd` 등으로 `json` 출력이 유효 JSON임을 검증). 이후 다른 실문서(`test-files/*.hwp`)로 fidelity 스윕.

### 결함 사이클 결과 (Cycles 27–33, 2026-06-05)

상세: `claudedocs/cycle-logs/cycle-27~33.md`, 평가: `claudedocs/plans/quality-assessment-2026-06-05-hwpx-reqdoc.md`, 로드맵: ROADMAP.md § v0.6

- **완료**: D1(`hp:tab`/leader 렌더) ✅ C27, D2(floating 이미지 block 렌더) ✅ C28, D8(`trim_text` `<hp:t>` 공백 보존) ✅ C31, D9(CLI 배너 stdout 오염) ✅ C31
- **Defer (YAGNI — evidence 수요 없음)**: D3(EMF→v0.7+), D5(rowSpan 0이라 colspan 빈 셀이 정답), D4(byte-identical 중복은 invisible)
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

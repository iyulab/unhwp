# unhwp — 핸드오프 문서

> **현재 상태:** v0.5.0 완료. 모든 v0.5 품질 향상 작업 완료. 릴리즈 준비 상태.

## 지금 하고 있는 것

v0.5.0 구현 완료 (Cycles 17–26). 릴리즈 대기 상태.

## 다음에 해야 할 것

### 최우선: 릴리즈 작업 (수동)

```
git commit -m "feat(v0.5): bold/italic fix, parse options, quality tests, async api fix"
git tag v0.5.0
git push && git push --tags
```

릴리즈 후 GitHub Actions가 자동으로:
- CI 빌드 검증
- npm 패키지 배포 (`@iyulab/unhwp@0.5.0`)
- GitHub Pages 업데이트

### 이후 (v0.6+)

→ `claudedocs/cycle-logs/ROADMAP.md` § 미래 방향

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

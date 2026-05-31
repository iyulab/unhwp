# unhwp — 핸드오프 문서

> **현재 상태:** v0.4.0 WASM 구현 완료. 버전 범프 완료, 커밋/태그 대기.

## 지금 하고 있는 것

v0.4.0 릴리즈 준비 — 코드 구현 완료, 커밋 및 GitHub 릴리즈 태그 생성 필요.

## 다음에 해야 할 것

1. **GitHub Pages 활성화** (수동): 리포 Settings → Pages → Source: `GitHub Actions`
2. **NPM_TOKEN 시크릿 설정** (수동): 리포 Settings → Secrets → `NPM_TOKEN`
3. **커밋 & 태그**: `git commit`, `git tag v0.4.0`, `git push --tags`
4. **CI 검증**: `build-wasm` 잡이 Ubuntu에서 `wasm-pack test --node` 통과하는지 확인

## 알려진 제약

### `parseWithOptions` 옵션 연결 미완성
WASM `parseWithOptions(data, opts)`가 현재 옵션을 무시하고 `parse_bytes`를 호출.
`parse_reader_with_options` 코어 API 추가로 해결 가능 — v0.5 후보.

### wasm-pack test Windows 경로 버그
`wasm-pack test --node` 실행 시 Windows에서 경로 이스케이프 오류 발생.
CI(Ubuntu)에서는 정상 작동. 로컬 Windows 환경에서는 cargo check/build로 검증.

## 핵심 컨텍스트

### 파일 구조 (v0.4.0 신규)
```
unhwp-wasm/
  Cargo.toml              # cdylib, wasm-bindgen, version 0.4.0
  README.md               # npm 패키지 문서
  src/
    lib.rs                # parse(), parseWithOptions() 진입점
    document.rs           # HwpDocument wasm-bindgen 바인딩
    options.rs            # ParseOptions wasm-bindgen 바인딩
docs/
  .nojekyll               # Jekyll 비활성화
  .gitignore              # pkg/ 제외
  index.html              # WASM 플레이그라운드 SPA
.github/workflows/
  pages.yml               # GitHub Pages 자동 배포 (신규)
  ci.yml                  # build-wasm 잡 추가됨
  release.yml             # publish-npm 잡 추가됨
```

### 버전 동기화 대상 파일 (5개 모두 동시 변경)
```
Cargo.toml                           # 0.4.0
cli/Cargo.toml                       # 0.4.0
unhwp-wasm/Cargo.toml                # 0.4.0
bindings/python/pyproject.toml       # 0.4.0
bindings/csharp/Unhwp/Unhwp.csproj   # 0.4.0
```

## 로드맵 앵커

→ `ROADMAP.md` § v0.5 — 품질 & 커버리지 향상

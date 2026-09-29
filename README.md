# rust-gui-benchmark

Git/DB 클라이언트용 GUI 프레임워크(Electron+Rust, Tauri, Slint, GPUI) 벤치마크와 크로스 플랫폼 CI.
모든 프로토타입은 같은 Rust 코어(`core/`)를 사용합니다. 결과 해석은 `REPORT.md` 참고 (Linux/Xvfb/GPU 없음 기준).

- `core/` 커밋 그래프(gix/git2) + SQLite 100만 행 코어
- `electron/` `tauri/` `slint-app/` `gpui-app/` UI 프로토타입 (`web/`은 Electron·Tauri 공용 프론트)
- `scripts/xbench.py` 크로스 플랫폼 하니스 (시작 시간, 메모리, CPU, 프레임 간격)
- `.github/workflows/ci.yml` ubuntu / macos / windows × 5개 잡. 결과는 Actions 요약, 어노테이션, artifact로 확인

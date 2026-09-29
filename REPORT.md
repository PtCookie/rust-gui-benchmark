# Git/DB 클라이언트 프레임워크 벤치마크 (Electron / Tauri / Slint / GPUI)

측정일 2026-09-29. Claude Code 클라우드 세션(2 vCPU, 7.8GB RAM, GPU 없음)에서 Xvfb(1280x800) + openbox 위에 실행.

## 무엇을 재었나

네 프로토타입은 **동일한 Rust 코어(`bench-core`)와 동일한 시나리오**를 씁니다. UI 계층만 다릅니다.

| 시나리오 | 데이터 | 동작 |
|---|---|---|
| commits | git/git 저장소 커밋 82,327개 (blobless clone), 그래프 레인 최대 281 | 가상 리스트 + 커밋 그래프 열 |
| grid | SQLite 100만 행 (6컬럼, 97MB) | 가상 리스트, 200행 단위 지연 페이징 |

- 준비 후 0.7초 대기, 이후 프레임마다 90행씩 스크롤(fast fling) 900프레임. **GPUI만 200프레임**(너무 느려서).
- 시작 시간: 프로세스 spawn부터 앱이 `.ready` 마커를 쓸 때까지(외부 측정).
- 메모리: 프로세스 트리 전체 PSS를 200ms마다 샘플링(공유 페이지 중복 집계 방지).
- 프레임 간격: Electron/Tauri는 `requestAnimationFrame` 간격, Slint는 winit `RedrawRequested` 간격, GPUI는 `request_animation_frame` 렌더 콜백 간격.
- CPU ms/frame: 준비 이후 트리 CPU 증가량 / 프레임 수.
- 각 조합 3회 실행 후 중앙값. 전체 벤치마크를 독립적으로 2회 수행했고 결과가 몇 % 이내로 일치.

버전: Electron 38.8.6, Tauri 2.12.0 (wry 0.57.0, WebKitGTK 2.52.6), Slint 1.18.1 (software renderer), gpui 0.2.2 (Vulkan/lavapipe), gix 0.88.0, git2 0.21.0, rusqlite 0.40.2, Rust 1.95.0.

## 코어 성능 (UI와 무관, 신뢰 가능)

| 항목 | 결과 |
|---|---|
| 커밋 82k 순회+디코딩, git2 | 1,555 ms |
| 커밋 82k 순회+디코딩, gix | 1,966 ms |
| 그래프 레인 계산 | 252 ms |
| 로드 후 코어 메모리 | 149 MB |
| 커밋 행 전체 JSON 직렬화 | 44 MB / 183 ms (역직렬화 264 ms) |
| SQLite 100행 페이지 조회 | p50 0.06 ms, p95 0.10 ms |
| SQLite 100만 행 전부 로드 | 478 ms / +165 MB |

## 프레임워크 비교 (소프트웨어 렌더링 환경)

### commits (82k)

| | 준비 (ms) | 코어 로드 | 프레임워크 오버헤드 | 프레임 평균 (ms) | p95 | >33ms 프레임 | CPU ms/frame | PSS 준비 후 (MB) |
|---|---|---|---|---|---|---|---|---|
| Electron | 2,400 | 1,817 | 583 | 18.35 | 33.3 | 10 | 18.2 | 556 |
| Tauri | 2,223 | 1,895 | 329 | 16.33 | 21.0 | 1 | 16.3 | 435 |
| Slint | 1,976 | 1,843 | 133 | 16.70 | 19.6 | 1 | 15.0 | 153 |
| GPUI | 2,266 | 1,797 | 469 | 125.84 (※) | 160.0 | 200 | 146.5 | 259 |

### grid (100만 행)

| | 준비 (ms) | 프레임워크 오버헤드 | 프레임 평균 (ms) | p95 | >33ms 프레임 | CPU ms/frame | PSS 준비 후 (MB) |
|---|---|---|---|---|---|---|---|
| Electron | 583 | 575 | 16.72 | 16.7 | 0 | 14.0 | 402 |
| Tauri | 286 | 280 | 16.21 | 17.0 | 0 | 14.2 | 216 |
| Slint | 89 | 83 | 16.13 | 16.8 | 0 | 11.4 | 34 |
| GPUI | 427 | 422 | 175.60 (※) | 176.9 | 200 | 183.2 | 114 |

(※) GPUI는 GPU 없이 소프트웨어 Vulkan(lavapipe)으로 돌아서 픽셀 처리가 CPU로 에뮬레이션됩니다. **GPUI의 실제 성능이 아닙니다.** 앱 쪽 리스트 구성 비용만 따로 재면 프레임당 0.12 ms(commits) / 0.17 ms(grid)입니다.

모든 프레임워크에서 빠른 스크롤 중 "행이 아직 안 온" 프레임은 0개였습니다. 즉 IPC 페이징(500행/200행 청크, 앞으로 3페이지 프리페치)이 90행/프레임 fling을 따라갔습니다.

### Tauri + WebKitGTK 렌더러 설정 (commits, GPU 없는 환경)

| 설정 | 프레임 평균 | CPU ms/frame | PSS 피크 |
|---|---|---|---|
| 기본값 | 38.3 ms | 56.9 | 665 MB |
| `WEBKIT_DISABLE_DMABUF_RENDERER=1` (벤치마크 기본) | 16.3 ms | 16.1 | 506 MB |
| `WEBKIT_DISABLE_COMPOSITING_MODE=1` | 16.3 ms | 15.9 | 524 MB |

## 이 결과로 말할 수 없는 것

- **실제 GPU 환경의 성능.** 프레임 간격이 60Hz 상한(~16.7ms)에 걸려 있어서 Electron/Tauri/Slint의 프레임 시간 차이는 작게 보입니다. CPU ms/frame과 메모리, 시작 시간이 더 믿을 만한 지표입니다.
- **GPUI의 성능.** GPU가 필요합니다.
- **WebKitGTK의 GPU 경로 문제**(Wayland/NVIDIA/Intel/AMD별 차이). 위 표의 기본값 열화는 소프트웨어 GL에서 나온 것이라 같은 원인으로 단정할 수 없습니다.
- Slint의 "준비" 시간은 `show()` 후 50ms 타이머라서 첫 프레임 시각보다 약간 낙관적입니다.
- Slint 그래프는 단색 선 + 색 있는 점(Path 한계), Electron/Tauri는 레인별 색 선(Canvas)이라 렌더 작업량이 조금 다릅니다.
- macOS/Windows, HiDPI, 한글 IME, 코드 에디터/diff 뷰어 같은 실제 위젯은 측정하지 않았습니다.

## 환경에서 발견한 것

- GPUI(X11)는 **창 관리자가 없으면 프레임 루프가 시작되지 않습니다**(MapNotify 이후에만 시작). 순수 Xvfb에서는 첫 프레임 이후 멈춥니다. CI에서는 openbox 같은 WM을 함께 띄워야 합니다.
- Tauri 릴리스 빌드는 프론트엔드 `dist`를 바이너리에 임베드하므로 프론트를 고치면 Rust도 다시 빌드해야 합니다.
- Tauri 커맨드는 인자를 이름으로 받습니다(객체를 그대로 넘기면 거부).
- gix 0.88은 `sha1` feature를 명시해야 컴파일됩니다.

## 재현

```
cd bench
python3 run.py all commits,grid 3   # electron,tauri,slint,gpui
python3 aggregate.py
```

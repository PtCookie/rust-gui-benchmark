# Git/DB 클라이언트 프레임워크 벤치마크: 실기기 결과 (Electron / Tauri / Slint / GPUI)

측정일 2026-09-30. 실제 GPU가 있는 데스크톱 5개 환경에서 같은 프로토타입을 돌린 결과입니다. 처음에 클라우드 VM(GPU 없음, Xvfb)에서 잰 결과는 판단용으로는 낡아서 이 문서로 대체했습니다(부록에 코어 수치와 당시 한계만 남김). 원본 수치는 `results-real/summary.json`에 있습니다.

## 측정 환경

| 기호 | OS | CPU / GPU | 디스플레이 | 비고 |
|---|---|---|---|---|
| **A** | macOS | Apple M1 Max, 64GB | 2560x1440 @144Hz | Slint는 `BENCH_HZ=144` 필요 |
| **B** | Windows 10.0.26200 | Ryzen 5 5600G (내장 Radeon), 15GB | 1920x1080 @60Hz | |
| **C** | Windows 11 Home 26H2 | Ryzen 7 7800X3D + RX 7700 XT, 63GB | 3840x2160 @1.25x 60Hz | D, E와 **같은 머신** |
| **D** | Linux (CachyOS), Hyprland 0.56 Wayland | 위와 같음 | 3840x2160 @60Hz | 배율 미기록 |
| **E** | Linux, KDE Plasma Wayland | 위와 같음 | 해상도/배율 미기록 | GPUI ready는 5회 재측정 |

버전: Electron 44, Tauri 2.12, Slint 1.18.1 (소프트웨어 렌더러 `slint`, Skia 렌더러 `slint-skia`), gpui 0.2.2, gix 0.88, rusqlite 0.40. 웹뷰는 Windows에서 WebView2, macOS에서 WKWebView, Linux에서 WebKitGTK(기본 설정, DMABUF 우회 없음)입니다.

## 무엇을 재었나

네 프로토타입은 **같은 Rust 코어(`bench-core`)와 같은 시나리오**를 쓰고 UI 계층만 다릅니다.

| 시나리오 | 데이터 | 동작 |
|---|---|---|
| commits | git/git 저장소 커밋 82,327개 (blobless), 그래프 레인 최대 281 | 가상 리스트 + 커밋 그래프 열, 프레임마다 90행씩 빠르게 스크롤 |
| grid | SQLite 100만 행 (6컬럼) | 가상 리스트, 200행 단위 지연 페이징, 같은 방식으로 스크롤 |
| idle | commits 로드 후 방치 | 아무것도 안 할 때의 CPU |

- 시작 시간(`ready`)은 프로세스 spawn부터 앱이 `.ready` 마커를 쓸 때까지입니다.
- CPU는 프로세스 트리 전체(macOS의 WebKit 도우미 프로세스 포함) 기준, 메모리는 Linux만 PSS이고 macOS/Windows는 RSS입니다. **RSS와 PSS는 직접 비교할 수 없습니다** (Linux Electron은 RSS 1012MB, PSS 536MB).
- 각 조합 3회 실행의 중앙값입니다.
- 프레임 간격은 디스플레이 주사율이 상한이라 프레임워크 간 비교 지표가 아닙니다. 그래서 아래 비교는 **프레임당 CPU(ms)** 를 중심으로 합니다. GPU 시간은 포함되지 않습니다.

## 결과

### 1. 스크롤이 버티는가

전 환경, 전 프레임워크가 디스플레이 주사율을 유지했고 33ms를 넘긴 프레임은 거의 없었습니다(commits 기준 0~1개). 스크롤 자체는 어느 프레임워크도 병목이 아닙니다. 다만 예외가 있습니다.

- **A(144Hz)**: Slint 소프트웨어 렌더러는 96fps에서 CPU 한계에 닿았고(93.9% 코어), Tauri(WKWebView)는 72fps로 나왔습니다(원인 미확인). 나머지는 143~144fps를 냈습니다.
- **E(KDE)**: Slint 소프트웨어 렌더러가 50fps(commits)와 46fps(grid)로 60fps를 못 채웠습니다. 같은 머신의 D에서는 60fps였습니다.

### 2. 프레임당 CPU (ms, commits / 낮을수록 좋음)

| | A macOS 144Hz | B Win 5600G | C Win 7800X3D | D Linux Hyprland | E Linux KDE |
|---|---|---|---|---|---|
| Slint (SW) | 9.8 | 10.2 | 8.2 | 14.1 | 17.1 |
| Slint + Skia | **4.0** | 6.0 | 5.5 | 9.3 | 9.4 |
| GPUI | 7.2 | 6.2 | **3.3** | 9.3 | 9.3 |
| Tauri | 15.8 | 12.9 | 6.0 | **7.8** | **7.6** |
| Electron | 13.3 | 17.3 | 9.1 | 8.0 | 8.0 |

grid도 순서는 대체로 같습니다 (A: Skia 3.8 < Slint 6.9 < GPUI 8.3 < Electron 12.1 < Tauri 15.2, C: Skia 4.0 ≈ GPUI 3.9 < Slint 5.5 < Tauri 6.2 < Electron 6.8, D/E: Electron 6.2 < Tauri 7.4 < GPUI 10.1 < Skia 13.8 < Slint 16.1~18.0).

- **macOS와 Windows**에서는 Skia와 GPUI가 웹뷰 계열(Tauri, Electron)보다 프레임당 CPU가 절반 이하입니다.
- **Linux**에서는 순서가 뒤집혀 Tauri와 Electron이 가장 낮고, Skia와 GPUI는 약 20% 높으며, Slint 소프트웨어 렌더러가 가장 높습니다.
- **같은 머신(C 대 D/E)** 에서 GPUI는 3.3ms(Windows) 대 9.3ms(Linux), Skia는 5.5 대 9.3, Slint SW는 8.2 대 14.1~17.1로 Linux에서 1.7~2.8배 무겁습니다. 반면 Tauri는 6.0 대 7.6~7.8, Electron은 9.1 대 8.0으로 차이가 작습니다. 즉 네이티브 렌더러는 Windows 경로가 Linux(Vulkan/Wayland) 경로보다 CPU 효율이 좋았습니다(Linux가 1.7~2.8배). 원인(드라이버, 렌더 경로, 배율)은 확인하지 못했습니다. Windows는 1.25배 배율이었고 Linux 쪽 배율은 기록하지 않았습니다.
- GPUI의 macOS CPU는 프레임당 7.2ms인데 143fps로 돌기 때문에 코어 하나를 넘게(103%) 씁니다. 이 하네스가 프레임마다 애니메이션을 요청하기 때문이고, idle에서는 2.4%로 낮습니다. 실제 앱의 CPU 사용은 앱 구조에 달렸습니다.

### 3. 메모리 (MB, commits, 준비 후)

| | A RSS | B RSS | C RSS | D PSS (RSS) | E PSS (RSS) |
|---|---|---|---|---|---|
| Slint (SW) | 360 | 116 | 116 | 162 (183) | 167 (194) |
| Slint + Skia | 484 | 240 | 230 | 207 (243) | 207 (242) |
| GPUI | 318 | 130 | 131 | 173 (197) | 171 (196) |
| Tauri | 538 | 552 | 532 | 540 (753) | 533 (733) |
| Electron | 723 | 568 | 595 | 536 (1012) | 531 (947) |

Windows와 Linux에서는 네이티브(Slint, GPUI, Skia)가 웹뷰 계열의 절반 이하(약 1/2.3~1/5)입니다. macOS에서는 네이티브도 RSS가 크게 잡혀(Skia 484MB) 차이가 1.1~2배로 줄지만, macOS의 RSS 정의 차이(GPU/공유 메모리 포함 여부) 때문이라 절대값 비교는 조심해야 합니다. Skia는 소프트웨어 렌더러보다 Windows에서 약 +115~125MB, macOS에서 +124MB입니다.

### 4. 시작 시간 (ms)

`ready` 중앙값(commits)과, 여기서 코어 로드 시간을 뺀 **프레임워크 오버헤드**입니다.

| | ready A / B / C / D / E | 오버헤드 A / B / C / D / E |
|---|---|---|
| Slint (SW) | 1400 / 1344 / 1002 / 781 / 783 | 337 / 69 / 68 / 100 / 105 |
| Slint + Skia | 1347 / 1532 / 1270 / 801 / 804 | 296 / 226 / 277 / 115 / 124 |
| GPUI | 1246 / 1547 / 1109 / 791 / 805 (※) | 247 / 228 / 179 / 103 / 110 |
| Tauri | 1735 / 1828 / 1248 / 1942 / 1966 | 726 / 520 / 306 / **1166 / 1200** |
| Electron | 1826 / 1610 / 1135 / 1263 / 1290 | 525 / 309 / 197 / 276 / 295 |

(※) E의 GPUI는 첫 측정에서 화면 보호기 때문에 1506ms가 나와서 5회 재측정한 값(805ms)입니다.

- 전체 `ready`는 코어 로드(커밋 82k 읽기, Linux 약 0.68초, Windows 0.93~1.3초, macOS 1.0~1.3초)가 대부분을 차지해서, Windows/macOS에서는 네이티브와 웹뷰의 차이가 작습니다. Linux에서만 네이티브가 0.8초로 확연히 빠릅니다.
- **Tauri(WebKitGTK)의 오버헤드가 Linux에서 1.2초로 가장 큽니다.** Windows(WebView2)에서는 0.3~0.5초입니다.
- Skia는 Slint 소프트웨어 렌더러보다 Windows에서 시작이 약 160~210ms 더 걸립니다(macOS와 Linux는 차이가 작음).

### 5. 유휴 CPU (%)

| | A | B | C | D Hyprland | E KDE |
|---|---|---|---|---|---|
| Slint (SW) | 0.3 | 0.3 | 0.0 | **45.4** | 0.0 |
| Slint + Skia | 0.6 | 0.2 | 0.2 | **5.0** | 0.0 |
| GPUI | 2.4 | 0.8 | 1.6 | 0.6 | 0.5 |
| Tauri | 0.2 | 4.3 | 1.0 | 3.5 | 4.4 |
| Electron | 1.6 | 0.2 | 0.0 | 0.2 | 0.5 |

같은 머신에서 **Slint의 idle 이상(45%, 5%)은 Hyprland에서만** 나왔고 KDE에서는 0%였습니다. Slint의 문제라기보다 Hyprland의 프레임 콜백과 맞물린 동작으로 보이지만, 원인은 확인하지 않은 추정입니다. Tauri는 Windows 5600G와 Linux KDE에서 4% 안팎으로 idle이 가장 높습니다.

## 위젯과 한글 입력 (Slint, `slint-widgets`)

`winit-skia`로 세 OS와 두 컴포지터에서 직접 확인했습니다. 자세한 로그 해석과 우회 방법은 README의 "Slint 위젯 확인"에 있습니다.

| 항목 | macOS | Windows 11 | Linux Hyprland | Linux KDE |
|---|---|---|---|---|
| 한글 조합, 방향키, 드래그, 한/영 | 정상 | 정상 | 정상 | 정상 |
| Cmd/Ctrl+A,C,V,X,Z (한글 입력 소스) | **동작 안 함** → 우회 적용 후 정상 | 정상 | 정상 | 정상 |
| 조합 중 포커스 상실 | **글자 소실** → 우회 적용 후 정상 | 우회 발동, 문제 없음 (※) | 우회 발동, 정상 | 컴포지터가 먼저 Commit해서 정상 |
| 이모지 입력 | **불가** (winit #3342) | 정상 (Win+.) | 미검증 (피커 없음) | 미검증 |
| 한자 변환 | 미검증 | 정상 | 미검증 | 미검증 |

(※) Windows에서는 포커스를 잃은 뒤에 `Commit`이 늦게 도착하므로 Slint #10861과 같은 유형으로 보이지만, 우회를 끈 상태로는 확인하지 않았습니다.

- 우회 두 개(macOS의 Cmd 단축키, 포커스 상실 시 조합 확정)는 앱 코드로 넣은 것이라 Slint 업스트림이 고치기 전까지 유지해야 합니다.
- Linux 확인은 fcitx5, Wayland 한정입니다. X11 세션, ibus, 소프트웨어 렌더러는 시험하지 않았습니다.
- **GPUI, Tauri, Electron의 한글 입력과 텍스트 편집은 이 프로젝트에서 측정하지 않았습니다.** 웹뷰 계열은 브라우저 IME를 그대로 쓰므로 이 부분에서는 성숙도가 앞설 가능성이 높지만, 실기기로 확인한 것은 아닙니다.

## 이 결과로 판단할 수 있는 것과 없는 것

**판단할 수 있는 것**

1. 82k 커밋, 100만 행 규모의 가상 스크롤은 네 프레임워크 모두 60Hz 기준 60fps로 처리합니다(144Hz에서는 Slint 소프트웨어 렌더러와 Tauri가 못 따라갔음). 성능 때문에 탈락시킬 프레임워크는 없습니다.
2. 네이티브(Slint, GPUI)의 확실한 이점은 **메모리**(Windows/Linux에서 웹뷰의 절반 이하)이고, macOS와 Windows에서는 프레임당 CPU도 절반 이하입니다. Linux에서는 CPU 이점이 없거나 반대입니다.
3. Tauri의 WebKitGTK 우려는 AMD GPU + Wayland(두 컴포지터) 한 구성에서는 재현되지 않았습니다. 60fps를 유지했고 DMABUF 우회도 필요 없었습니다. 다만 시작이 가장 느립니다(약 2초).
4. Slint의 한글 입력은 우회 두 개를 넣으면 macOS, Windows, Linux(fcitx5)에서 실사용 가능한 수준입니다.

**판단할 수 없는 것**

- NVIDIA/Intel GPU, X11, ibus 등 다른 Linux 구성. macOS 외장 모니터가 아닌 환경과 Windows의 다른 GPU.
- 실제 앱(코드 에디터, 대형 diff, 텍스트 선택/검색, 접근성)의 성능. 이 벤치마크는 스크립트로 스크롤하는 합성 부하입니다.
- GPUI의 위젯, IME, 텍스트 입력. 그리고 gpui 0.2.x가 안정화될 때 API가 얼마나 바뀔지.
- Linux에서 네이티브 렌더러가 같은 머신의 Windows보다 1.7~2.8배 무거운 이유.

## 선택 (잠정)

- **지금 시작한다면**: Slint + Skia가 가장 안전한 선택으로 보입니다. 세 OS에서 60fps를 유지하고, 메모리가 작고, 한글 입력을 실기기로 확인했습니다. 대가는 Windows에서 시작 +160~210ms, 위젯(코드 에디터, diff 뷰 등)을 직접 만들어야 한다는 점입니다.
- **GPUI로 옮길지**는 이 벤치마크만으로 결정하기 어렵습니다. 프레임당 CPU와 메모리는 Windows에서 가장 좋고 macOS에서도 상위지만, 텍스트 입력과 한글 IME를 검증하지 못했고 아직 0.2.x입니다. 안정화 후 마이그레이션을 고려한다면 그 시점에 `slint-widgets`와 같은 한글 입력 체크를 GPUI로 먼저 해보는 것을 권합니다.
- **웹뷰(Tauri, Electron)** 는 성능 문제는 없고 메모리가 500MB 이상이며 Tauri는 Linux 시작이 약 2초입니다. TypeScript 자산을 그대로 쓸 수 있고 텍스트 입력이 성숙하다는 점이 이 비용을 정당화하는지가 관건입니다.

## 부록: 초기 클라우드 VM 측정 (참고용)

2 vCPU, GPU 없음, Xvfb 위에서 잰 첫 결과입니다. UI 렌더링 수치는 소프트웨어 렌더링 환경 값이라 위 실기기 결과로 대체했고(특히 GPUI는 소프트웨어 Vulkan이라 성능 지표가 아님), UI와 무관한 코어 수치만 참고로 남깁니다.

| 항목 | 결과 |
|---|---|
| 커밋 82k 순회+디코딩, git2 | 1,555 ms |
| 커밋 82k 순회+디코딩, gix | 1,966 ms |
| 그래프 레인 계산 | 252 ms |
| 로드 후 코어 메모리 | 149 MB |
| 커밋 행 전체 JSON 직렬화 | 44 MB / 183 ms (역직렬화 264 ms) |
| SQLite 100행 페이지 조회 | p50 0.06 ms, p95 0.10 ms |
| SQLite 100만 행 전부 로드 | 478 ms / +165 MB |

당시 Tauri의 WebKitGTK는 GPU 없는 환경에서 기본값이 38.3ms/frame이었고 `WEBKIT_DISABLE_DMABUF_RENDERER=1`로 16.3ms가 됐지만, 실제 GPU 환경(D, E)에서는 기본값으로 60fps였으므로 그 열화는 소프트웨어 GL에서 나온 것입니다.

## 재현

`README.md`의 "실제 기기에서 실행하기"를 따릅니다. 요약:

```
python3 scripts/xbench.py --fw slint-skia --runs 3 --real-gpu    # 프레임워크별
python3 scripts/summarize.py                                     # 중앙값 표
```

고주사율 모니터에서는 Slint에 `BENCH_HZ=<주사율>`이 필요합니다.

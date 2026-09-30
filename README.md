# rust-gui-benchmark

Git/DB 클라이언트용 GUI 프레임워크(Electron+Rust, Tauri, Slint, GPUI)를 같은 Rust 코어와 같은 시나리오로 비교하는 벤치마크입니다.
Linux/Xvfb/GPU 없음 환경에서 잰 결과와 해석은 [`REPORT.md`](REPORT.md), 크로스 플랫폼 빌드·실행 검증은 GitHub Actions(`.github/workflows/ci.yml`)에서 합니다.
이 문서는 **내 기기(GPU 있는 실제 데스크톱)에서 직접 돌리는 방법**을 설명합니다.

## 구성

| 경로 | 내용 |
|---|---|
| `core/` | 공용 코어: 커밋 그래프(gix/git2) + SQLite 100만 행 페이징. `core-bench` 바이너리는 UI 없이 코어만 측정 |
| `web/` | Electron·Tauri 공용 프론트엔드 (TypeScript, Vite) |
| `electron/` | Electron 앱 + napi-rs 애드온(`electron/addon`) |
| `tauri/src-tauri/` | Tauri 2 앱 |
| `slint-app/` | Slint 앱 (소프트웨어 렌더러) |
| `gpui-app/` | GPUI 앱 (GPU 필요) |
| `scripts/` | `xbench.py` 측정 하니스, `make_db.py` 데이터 생성, `summarize.py` 결과 표 |

시나리오는 두 가지입니다. `commits`(git/git 저장소 커밋 약 8.2만 개, 그래프 열 포함 가상 리스트)와 `grid`(SQLite 100만 행 가상 그리드). 빠른 스크롤(프레임당 90행) 900프레임 동안 프레임 간격, CPU, 메모리를 잽니다.

## 요구사항

### 공통

| 항목 | 버전 | 비고 |
|---|---|---|
| Git | 2.30+ | `--filter=blob:none` 지원 필요 |
| Rust | stable (테스트: 1.95) | [rustup](https://rustup.rs)으로 설치 |
| Node.js | **24 LTS** | `.nvmrc` 참고. Tauri·Electron 프론트 빌드와 Electron 실행에 필요 (Slint·GPUI만 쓰면 불필요) |
| Python | 3.10+ | 하니스용. `pip install psutil` |
| 디스크 | 약 15GB | 데이터 약 0.3GB + 빌드 산출물(`target/`)이 프레임워크마다 2~5GB |
| 네트워크 | 필요 | crates.io, npm, github.com/git/git 클론 |

기기 자체 조건: 그래픽 드라이버가 정상이고, **원격 데스크톱/가상 머신이 아닌 실제 디스플레이 세션**이어야 합니다. 노트북은 전원을 연결하고 절전 모드를 끄세요.

### Linux (Ubuntu/Debian 계열 기준)

```bash
# 공통 빌드 의존성 (Slint, GPUI)
sudo apt-get install -y build-essential pkg-config cmake clang \
  libxkbcommon-x11-dev libxkbcommon-dev libwayland-dev libxcb1-dev \
  libfontconfig-dev libfreetype-dev libx11-dev libxcursor-dev libxi-dev libxrandr-dev

# Tauri (WebKitGTK 4.1)
sudo apt-get install -y libwebkit2gtk-4.1-dev libgtk-3-dev libsoup-3.0-dev \
  libjavascriptcoregtk-4.1-dev librsvg2-dev libayatana-appindicator3-dev libxdo-dev libssl-dev

# GPUI (Vulkan): GPU 드라이버 + 로더
sudo apt-get install -y libvulkan1 libvulkan-dev mesa-vulkan-drivers vulkan-tools

# Electron 실행 라이브러리
sudo apt-get install -y libgbm1 libnss3 libatk-bridge2.0-0t64 libgtk-3-0t64 \
  libasound2t64 libxss1 libxtst6 libcups2t64
```

- Ubuntu 22.04 이하는 패키지 이름의 `t64` 접미사를 빼세요. Fedora/Arch는 같은 이름의 `-devel` 패키지를 찾으세요.
- NVIDIA는 사유 드라이버(Vulkan 포함)를 쓰세요. `vulkaninfo --summary`에서 GPU가 lavapipe(소프트웨어)가 아니라 실제 GPU로 나와야 GPUI 결과가 의미 있습니다.
- Wayland/X11 어느 쪽이든 됩니다. 두 환경의 결과가 다를 수 있으니 어느 쪽에서 쟀는지 기록하세요.

### macOS (Apple Silicon/Intel)

```bash
xcode-select --install                 # Command Line Tools
# GPUI는 Metal 셰이더 컴파일이 필요합니다: Xcode 전체 설치 후
xcodebuild -downloadComponent MetalToolchain
```

Tauri는 시스템 WKWebView를 씁니다. 추가 설치는 필요 없습니다.

### Windows 10/11

- [Visual Studio 2022 Build Tools](https://visualstudio.microsoft.com/downloads/)에서 "C++를 사용한 데스크톱 개발" 워크로드(MSVC + Windows 11 SDK)
- [Rust](https://rustup.rs) (`x86_64-pc-windows-msvc`), [Git for Windows](https://git-scm.com/download/win), [Node.js 24 LTS](https://nodejs.org), [Python 3](https://www.python.org/downloads/)
- Tauri: WebView2 런타임 (Windows 11은 기본 포함, 10은 [설치](https://developer.microsoft.com/microsoft-edge/webview2/))
- GPUI 빌드가 실패하면 CMake와 LLVM(clang)을 추가로 설치해 보세요. CI 러너에는 기본으로 들어 있어서 통과했지만, 깨끗한 PC에서는 확인하지 못했습니다.
- PowerShell에서 실행하세요. 아래 명령은 Linux/macOS 기준이며 Windows 차이점은 각 단계에 적었습니다.

## 실행 매뉴얼

### 1. 클론과 준비

```bash
git clone https://github.com/PtCookie/rust-gui-benchmark.git
cd rust-gui-benchmark
python3 -m pip install psutil            # Windows: py -m pip install psutil
```

### 2. 데이터 준비

```bash
mkdir data results
# 커밋 그래프용: git/git 저장소 (blobless, 약 120MB)
git clone --bare --filter=blob:none https://github.com/git/git.git data/git-blobless.git
# 그리드용: SQLite 100만 행 (약 100MB, 수십 초)
python3 scripts/make_db.py data/events.db 1000000
```

내 저장소로 재려면 `data/git-blobless.git`을 그 저장소의 bare 클론이나 심볼릭 링크로 바꾸면 됩니다.

### 3. 코어만 측정 (UI 없음, 가장 먼저 권장)

```bash
cargo run --release --manifest-path core/Cargo.toml --bin core-bench -- \
  data/git-blobless.git data/events.db 1000000
```

JSON이 출력됩니다. 커밋 82,327개, 최대 레인 281이 나와야 데이터가 정상입니다.

### 4. 프레임워크별 빌드

실제 성능 측정이므로 반드시 `--release`입니다. 첫 빌드는 5~20분 걸립니다.

```bash
# Slint
cargo build --release --manifest-path slint-app/Cargo.toml

# GPUI
cargo build --release --manifest-path gpui-app/Cargo.toml

# Tauri (프론트를 먼저 빌드해야 합니다. 프론트를 고치면 Rust도 다시 빌드하세요)
(cd web && npm ci && npm run build)
cargo build --release --manifest-path tauri/src-tauri/Cargo.toml

# Electron (프론트 + 네이티브 애드온)
(cd web && npm ci && npm run build)          # 위에서 했다면 생략
cargo build --release --manifest-path electron/addon/Cargo.toml
# 애드온 라이브러리를 addon.node로 복사:
#   Linux  : electron/addon/target/release/libbench_electron_addon.so
#   macOS  : electron/addon/target/release/libbench_electron_addon.dylib
#   Windows: electron\addon\target\release\bench_electron_addon.dll
cp electron/addon/target/release/libbench_electron_addon.so electron/addon.node
(cd electron && npm ci && npx install-electron)   # Electron 44+: npm install 시 바이너리를 받지 않으므로 별도 실행
```

Windows PowerShell에서는 `cp`가 `Copy-Item`으로 동작하므로 경로만 위 표대로 바꾸면 됩니다.

### 5. 측정 실행

```bash
python3 scripts/xbench.py --fw slint    --runs 3 --real-gpu
python3 scripts/xbench.py --fw gpui     --runs 3 --real-gpu
python3 scripts/xbench.py --fw tauri    --runs 3 --real-gpu
python3 scripts/xbench.py --fw electron --runs 3 --real-gpu
python3 scripts/summarize.py             # 중앙값 표
```

- 실행 중에는 앱 창이 뜹니다. **창을 가리거나 최소화하거나 다른 창을 조작하지 마세요.** 가려지면 프레임이 멈춥니다. 한 번에 약 10~30초입니다.
- `--real-gpu`는 GPU가 있는 기기용입니다. 이 옵션이 없으면 CI용 설정(Electron GPU 끔, GPUI 소프트웨어 Vulkan·200프레임)으로 동작합니다. Slint는 소프트웨어 렌더러라서 옵션과 무관합니다.
- 시나리오를 하나만 돌리려면 `--scenarios commits` 또는 `--scenarios grid`, 진행 로그와 앱 출력은 `results/*.log`에 남습니다.
- Linux에서 Tauri는 WebKitGTK 렌더러 설정에 따라 성능이 크게 달라집니다. 비교하려면 태그를 나눠서 돌리세요.

```bash
python3 scripts/xbench.py --fw tauri --runs 3 --real-gpu --tag default
python3 scripts/xbench.py --fw tauri --runs 3 --real-gpu --tag dmabuf-off --env WEBKIT_DISABLE_DMABUF_RENDERER=1
```

### 6. 결과 읽는 법

| 지표 | 의미 |
|---|---|
| ready ms | 프로세스 시작부터 첫 화면 준비까지 (외부 측정) |
| core load ms | 코어가 데이터를 읽는 시간. 프레임워크와 무관하므로 `ready`에서 빼면 프레임워크 오버헤드 |
| frame avg / p95 ms | 스크롤 중 프레임 간격. **모니터 주사율이 상한**입니다 (60Hz면 16.7ms, 120Hz면 8.3ms). 주사율이 다른 기기끼리는 이 값을 직접 비교하지 마세요 |
| >33ms frames | 60Hz 기준 두 프레임 이상 끊긴 횟수 |
| fps | `1000 / frame avg`. 주사율이 다른 프레임워크끼리는 이 값과 아래 CPU %로 비교하세요 |
| CPU ms/frame | 프레임당 프로세스 트리 전체 CPU 시간. fps가 다르면 직접 비교할 수 없습니다 |
| CPU % of 1 core | `CPU ms/frame × fps`. 코어 1개를 100%로 본 초당 CPU 사용률 (멀티 프로세스 합계라 100% 초과 가능) |
| PSS / RSS MB | 준비 후 메모리 중앙값. macOS의 Tauri는 앱 밖에서 도는 `com.apple.WebKit.*` 프로세스 중 실행 후 새로 생긴 것을 합산합니다 (측정 중 Safari 등 WebKit 앱을 새로 열지 마세요). PSS는 Linux에서만 나오고 공유 페이지를 나눠 셉니다. macOS/Windows는 RSS라서 웹뷰 계열이 실제보다 크게 나올 수 있습니다 |

기기 정보(CPU, GPU, OS, 주사율, 배율)를 결과와 함께 적어 두세요.

## 트러블슈팅

| 증상 | 원인과 해결 |
|---|---|
| `xbench.py`가 `run failed`와 `exit=101` | `results/*.log` 끝부분 확인. 데이터 경로(`data/git-blobless.git`, `data/events.db`)가 맞는지 확인 |
| Slint 프레임 값이 이상함 (macOS) | CI의 macOS 러너에서는 Slint가 리드로 이벤트를 받지 못해(원인 미확인) **타이머 간격**으로 대체했습니다. 이 경우 JSON의 `frame_source`가 `timer`이고 실제 렌더 간격이 아닙니다. Slint는 모니터 주사율(`display_hz`, `BENCH_HZ` 환경변수로 강제 가능)에 맞춰 스크롤을 구동합니다. 실기기에서는 `redraw`로 나오는지 확인하세요 |
| GPUI가 창만 뜨고 진행이 없음 | Linux/X11에서는 창 관리자가 필요합니다. Vulkan이 소프트웨어(lavapipe)로 잡혔는지 `vulkaninfo --summary` 확인 |
| GPUI 빌드에서 Metal 오류 (macOS) | `xcodebuild -downloadComponent MetalToolchain` 후 다시 빌드 |
| Tauri가 시작 후 멈추거나 빈 화면 (Linux) | `WEBKIT_DISABLE_DMABUF_RENDERER=1`로 재시도 (위 `--env` 참고) |
| Tauri 화면이 예전 프론트 그대로 | 프론트는 바이너리에 임베드됩니다. `web`을 다시 빌드하고 `touch tauri/src-tauri/src/main.rs` 후 재빌드 |
| Electron 실행 파일이 없다는 오류 (`electron/dist`) | `cd electron && npx install-electron`으로 바이너리 받기 |
| Electron이 `addon.node`를 못 찾음 | 4단계의 애드온 복사를 빠뜨림. OS별 라이브러리 이름 확인 |
| Windows에서 시작 직후 한 번 크게 멈춤 | 백신 실시간 검사와 첫 파일 접근 영향으로 보입니다. 같은 조건에서 `--runs`를 늘려 중앙값을 보세요 |
| `EBADENGINE` 경고 | Node가 24보다 낮음. `nvm use`(`.nvmrc`)로 24 LTS 사용 |

## 크로스 플랫폼 CI

`main`에 push하거나 Actions 탭에서 수동 실행하면 ubuntu / macos / windows × (core, slint, tauri, gpui, electron) 15개 잡이 빌드와 실행을 검증합니다. 러너에는 GPU가 없어서 절대 성능이 아니라 "각 OS에서 빌드되고 돌아가는가"를 보는 용도입니다. 결과는 각 잡의 어노테이션, 요약, artifact(`results-*`)에 남습니다.

## 버전

Node 24 LTS, Electron 44, Vite 8, TypeScript 7, Tauri 2.12, Slint 1.18, gpui 0.2.2, Python 3.10+ (CI는 3.14), Rust stable (테스트 1.95).

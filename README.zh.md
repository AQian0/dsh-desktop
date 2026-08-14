# dsh-desktop

[English](README.md) | 中文

DeepSeek Harness Web 界面的 Tauri 2 桌面套壳。套壳启动一个 Harness 运行时（默认 `dsh web`），等待运行时在 stdout 上打印的本地回环 URL，然后在该 URL 上打开一个 webview 窗口。它不提供任何应用界面：窗口渲染产品自身的 Web 前端，由运行时通过 127.0.0.1 上的 HTTP 服务提供。

本项目由 **DeepSeek Harness（dsh）驱动**——它启动并监督运行时，运行时是一个独立项目；代码由 **AI 原生编写**。

## 工作原理

启动时套壳运行 `DSH_BIN` 与 `DSH_ARGS` 指定的命令（默认 `dsh web`），强制追加 `--host 127.0.0.1`，并在设置了 `DSH_PORT` 时追加 `--port <port>`。它扫描子进程 stdout 中的本地回环 URL（`dsh web: http://127.0.0.1:<port>`），只有该 URL 出现后才打开 webview 窗口；子进程的 stdout 行以 `[dsh]` 前缀回显，stderr 直接继承。如果运行时在发布 URL 之前退出，或 60 秒内没有出现 URL，套壳改为打开一个静态错误窗口。

关闭最后一个窗口会终止运行时：先发 SIGTERM 让 harness 完成收尾（会话落盘、终端恢复），五秒宽限后再发 SIGKILL；Windows 上则终止整个进程树。如果运行时自行退出，套壳随之关闭。

Web 信任栅栏在此无需任何配置：窗口导航到回环源，因此每个请求都携带回环 `Host`。

## 设计笔记

**为什么是套壳，而不是第二份 UI。** Web host 刻意只为浏览器服务——webview 通过 `file://` 加载构建产物 `dist` 会与运行时的 `/api` 不同源——因此套壳通过运行时自身的回环 HTTP 监督既有运行时：一个进程、一个窗口，不复制 UI。页面拿不到任何 Tauri API，因此也无需 capability 授权。

**否决的备选。** 通过 `file://` 加载 `dist`（破坏同源信任模型，复制连接层）；固定端口加 TCP 轮询（打印的 URL 行才是运行时自身的唯一事实源）；运行时内的 Cordis UI bundle（窗口生命周期与进程监督属于启动器职责，留在运行时之外让双方都能独立打补丁）；把运行时打进应用内（推迟给 harness 的单文件分发工作）。

**验证。** `cargo test` 固定 URL 扫描（忽略 LAN 后缀与无端口 URL）、环境变量命令组装、spawn–终止–回收、提前退出报告。组装烟测用 debug 二进制拉起源码启动的 `dsh web`（使用空闲端口），检查发布的 URL、注入 `window.__DSH_BOOT__` 的 200 响应，以及对套壳发 SIGTERM 后端口被释放。

## 前置条件

- 一个 DeepSeek Harness 运行时，二选一：
  - 已安装的 `dsh` CLI（`npm i -g @deepseek-ai/dsh`），供默认的 `dsh web` 命令使用；
  - [deepseek-harness](https://github.com/deepseek-ai/deepseek-harness) 源码检出且已构建 Web 制品（`pnpm run build`），供下方的源码检出命令使用。
- Node.js 与 pnpm，本项目脚本与 Tauri CLI 需要。
- Rust 工具链（经 [rustup](https://rustup.rs/) 安装），用于构建本套壳。
- Tauri 平台依赖：macOS 需要 Xcode Command Line Tools；Windows 需要 WebView2 与 MSVC 构建工具；Linux 需要 webkit2gtk-4.1（或发行版等价物）——参见 [Tauri 前置条件](https://tauri.app/start/prerequisites/)。

## 运行

```sh
pnpm install   # 安装 Tauri CLI
pnpm dev       # 以开发模式构建并启动套壳
```

`pnpm dev` 从 PATH 启动 `dsh web`，运行时打印出 URL 后才打开窗口——在此之前没有窗口出现。运行时的 stdout 会以 `[dsh]` 前缀回显在启动终端上；窗口迟迟不出现时请看该终端。

对 harness 源码检出进行开发：

```sh
DSH_BIN=node \
  DSH_ARGS="--import tsx/esm <checkout>/apps/cli/src/bin.ts web" \
  DSH_PORT=3180 \
  pnpm dev
```

`DSH_PORT` 缺省使用 harness 默认端口（3080）；若已有实例占用 3080 请换一个端口。

## 构建发行包

```sh
pnpm build             # 发布构建并产出平台安装包
pnpm build:no-bundle   # 仅发布二进制
```

安装包位于 `src-tauri/target/release/bundle`（macOS 为 `.app`/`.dmg`，Windows 为 `.msi`/`.exe`，Linux 为 `.deb`/`.rpm`/`AppImage`）。安装包未签名——见[已知限制](#已知限制与后续工作)。

## 配置

| 变量 | 默认值 | 含义 |
|---|---|---|
| `DSH_BIN` | `dsh` | 要启动的可执行文件。 |
| `DSH_ARGS` | `web` | 追加在 `DSH_BIN` 之后的参数，按空白切分。 |
| `DSH_PORT` | 未设置 | 设置时追加 `--port <value>`。 |

按空白切分的解析无法表达带引号的参数；当某个参数包含空格时，请把 `DSH_BIN` 指向一个包装脚本。

## 常见问题

- **窗口显示静态错误页** — 运行时在发布 URL 前失败，或 60 秒等待超时。从终端启动并查看运行时的 stderr。
- **构建报错 `failed to read plugin permissions … No such file or directory`** — 仓库在之前构建后被移动或改名，构建缓存里残留了旧绝对路径。在 `src-tauri` 下执行 `cargo clean`（或删除 `src-tauri/target`）后重试。
- **3080 端口被占用** — 设置 `DSH_PORT` 为空闲端口。
- **`dsh: command not found`** — 安装 CLI（`npm i -g @deepseek-ai/dsh`）或把 `DSH_BIN` 指向你的运行时。

## 已知限制与后续工作

- **运行时未打包** — 套壳启动的是单独安装或构建的 harness；把运行时嵌入单一应用分发的方案留待后续。
- **Windows 上的关闭阶梯未经测试** — SIGTERM 收尾仅限 Unix；Windows 使用 `taskkill /T /F`。
- **无 macOS 代码签名与公证** — `pnpm build` 产出未签名安装包；在本地使用之外分发需要签名凭据。
- **错误详情留在 stderr** — 失败窗口是静态的；从终端启动可读到运行时的诊断信息。

## 许可证与声明

MIT 许可证——见 [LICENSE](LICENSE)。

### DeepSeek Harness 声明

本项目由 [DeepSeek Harness](https://github.com/deepseek-ai/deepseek-harness)（`dsh`）驱动：套壳以子进程方式启动并监督一个 dsh 运行时。运行时是独立项目，按 MIT 许可证分发，Copyright (c) 2026 DeepSeek；本仓库不打包、不分发运行时。

`icons-src/app-icon.svg` 与 `src-tauri/icons/` 下的应用图标源自 DeepSeek Harness Web favicon 图形（[apps/web/public/favicon.svg](https://github.com/deepseek-ai/deepseek-harness/blob/master/apps/web/public/favicon.svg)），按同一 MIT 许可证使用。

完整的 MIT 许可声明见 [harness 仓库](https://github.com/deepseek-ai/deepseek-harness)。

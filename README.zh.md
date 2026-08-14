# dsh-desktop

[English](README.md) | 中文

DeepSeek Harness Web 界面的 Tauri 2 桌面套壳。它自己**不启动也不监督任何运行时**：
由 desktop profile 的插件（本仓库 `plugin/`，包 `@aqian0/dsh-desktop-plugin`）在 Web
运行时绑定回环端口后，以 `dsh-desktop --attach http://127.0.0.1:<port>` 拉起本二进制；
套壳只在该 URL 上打开一个 webview 窗口。它不提供任何应用界面：窗口渲染产品自身的 Web
前端，由运行时通过 127.0.0.1 上的 HTTP 服务提供。

本项目由 **DeepSeek Harness（dsh）驱动**——运行时是父进程、套壳是被拉起的一方，运行时是独立项目；
代码由 **AI 原生编写**。

## 快速开始

```sh
pnpm install                 # 安装依赖（Tauri CLI）
pnpm shell:build             # 构建 debug 套壳二进制（src-tauri/target/debug/dsh-desktop）
pnpm plugin:install          # 安装 desktop profile（含本插件；可重复执行）
pnpm plugin:smoke            # （可选）双向生命周期冒烟，无需 GUI
pnpm plugin:run              # 启动：等价于 dsh --profile desktop，弹出桌面窗口
```

想换端口：`dsh --profile desktop --port 3081`（web profile 的参数原样可用）。
更新仓库后重跑 `pnpm shell:build && pnpm plugin:install` 即可（install 幂等）。

## 工作原理

插件的 `desktop-launch` 行在 web server 绑定后 spawn 本二进制并传入回环 URL，然后双向绑定生命周期：

- **窗口关闭**（用户关闭最后一个窗口）：套壳进程退出，插件经 `ctx.appExit` 请求 profile 优雅退出
  （会话落盘由 harness 的树释放完成；插件带 5 秒有界强制退出兜底，防止启动器级句柄占住事件循环）；
- **运行时先死**（信号、崩溃、kill -9）：三条并行链路关闭窗口——树释放时插件的 `ctx.effect` 清理钩子
  直接 SIGTERM 套壳；套壳自身的 stdin 管道 EOF；以及 Unix 的父进程收养轮询（macOS 启动时会重接
  stdio，故以轮询为准）。窗口不会比运行时活得更久。

Web 信任栅栏在此无需任何配置：窗口导航到回环源，因此每个请求都携带回环 `Host`。

## 设计笔记

**为什么是套壳，而不是第二份 UI。** Web host 刻意只为浏览器服务——webview 通过 `file://` 加载构建
产物 `dist` 会与运行时的 `/api` 不同源——因此套壳通过运行时自身的回环 HTTP 附着既有运行时：一个进程、
一个窗口，不复制 UI。页面拿不到任何 Tauri API，因此也无需 capability 授权。

**运行时是父进程，窗口生命周期留在壳内。** `plugin/` 把套壳封装成可安装的 dsh bundle：desktop profile
的 bundles 依次列出 `@deepseek-ai/dsh-base`、`@deepseek-ai/dsh-web-app` 与本插件，因此完整继承了 Web
profile。插件在 web server 绑定后拉起套壳；窗口关闭请求 profile 退出，树被释放时反向关闭窗口。窗口
生命周期与父进程监测都留在套壳二进制内，运行时只是新增了一个“启动器”角色。

**否决的备选。** 通过 `file://` 加载 `dist`（破坏同源信任模型，复制连接层）；运行时内的 Cordis UI bundle
（窗口生命周期属于壳、不属于运行时，留在运行时之外让双方都能独立打补丁）；套壳反过来 spawn 并监督
运行时（旧的监督形态：运行时成了子进程，窗口关闭的退出阶梯、stdout 扫 URL 都落在壳里；插件形态下
运行时天然是父进程，这些机制整体删除）；把运行时打进应用内（推迟给 harness 的单文件分发工作）。

**验证。** `cargo test` 固定 attach 参数解析（缺省、非回环、无端口、畸形 URL）与回环 URL token 扫描
（忽略 LAN 后缀与无端口 URL）；`pnpm plugin:smoke` 无 GUI 地回归双向生命周期（窗口关闭→profile 以
码 0 退出；SIGTERM dsh→树释放→清理钩子杀死壳）；真实二进制烟测验证壳随父进程消亡退出（macOS 上
1 秒内）。

## 前置条件

- 已安装的 `dsh` CLI（`npm i -g @deepseek-ai/dsh`），并已按下方步骤安装 desktop profile。
- Node.js 与 pnpm，本项目脚本与 Tauri CLI 需要。
- Rust 工具链（经 [rustup](https://rustup.rs/) 安装），用于构建本套壳。
- Tauri 平台依赖：macOS 需要 Xcode Command Line Tools；Windows 需要 WebView2 与 MSVC 构建工具；
  Linux 需要 webkit2gtk-4.1（或发行版等价物）——参见 [Tauri 前置条件](https://tauri.app/start/prerequisites/)。

## 安装为 dsh profile 插件

仓库根目录 `plugin/` 是可安装的 bundle 包（`@aqian0/dsh-desktop-plugin`）：`package.json` 声明
`dsh.bundle.patch`，`cordis.patch.yml` 增加一行 `desktop-launch`，宿主插件在 web server 绑定后拉起套壳，
并把 profile 生命周期与窗口绑定（关窗即退出、树释放即关窗）。dsh 没有显式的 profile 继承，“继承 web
profile”即 bundle 组合：

```sh
pnpm plugin:install   # dsh plugin --profile desktop add ./plugin，并把 web-app 列入 bundles
pnpm plugin:smoke     # 无 GUI 的双向生命周期冒烟：窗口关闭→profile 退出；运行时先死→壳关闭
pnpm plugin:run       # 等价于 dsh --profile desktop
```

生成的 `~/.dsh/profiles/desktop/package.json` 的 `dsh.profile.bundles` 为
`["@deepseek-ai/dsh-base", "@deepseek-ai/dsh-web-app", "@aqian0/dsh-desktop-plugin"]`；配置树按序叠加，
后写者胜。`@deepseek-ai/dsh-web-app` 只列入 bundles、不作为 profile 依赖安装——bundle 解析先走运行中的
dsh 安装目录，Web 界面永远与所装 dsh 版本一致。套壳二进制解析顺序与发布计划见 `plugin/README.zh.md`。

## 运行（源码检出的开发形态）

```sh
pnpm install        # 安装 Tauri CLI
pnpm shell:build    # 构建 debug 套壳二进制

# 套壳指向 debug 二进制，由 desktop profile 拉起
DSH_DESKTOP_BIN=src-tauri/target/debug/dsh-desktop dsh --profile desktop
```

`pnpm plugin:install` 装好的 profile 默认在 PATH 上找不到套壳时，也可在 profile 的
`~/.dsh/profiles/desktop/cordis.patch.yml` 里给 `desktop-launch` 行写 `config: { bin: <绝对路径> }`
固定二进制位置。直接运行二进制而不带 `--attach` 会打印提示并显示静态错误窗口：套壳只能经 profile
插件拉起。

## 构建发行包

```sh
pnpm build             # 发布构建并产出平台安装包
pnpm build:no-bundle   # 仅发布二进制
```

安装包位于 `src-tauri/target/release/bundle`（macOS 为 `.app`/`.dmg`，Windows 为 `.msi`/`.exe`，Linux 为
`.deb`/`.rpm`/`AppImage`）。安装包未签名——见[已知限制](#已知限制与后续工作)。

## 打包 npm 分发（插件装上即跑）

插件以 per-platform optionalDependencies 随包分发套壳二进制，用户只需
`dsh plugin --profile desktop add @aqian0/dsh-desktop-plugin`，无需任何环境变量：

```sh
pnpm package:current -- --build   # release 构建并把二进制装入 platforms/<当前平台>-<arch>/bin/
                                  # 然后在 dist/ 下打包出两个 tgz：插件 + 当前平台包
```

发布时**同版本一起发布**插件与全部平台包（`platforms/` 下的 4 个目标）：

```sh
npm publish dist/<插件 tgz> dist/<各平台 tgz> ...
```

其他平台的二进制需要在对应操作系统上各跑一次 `pnpm package:current -- --build`（或由 CI 矩阵完成）。

## 配置

| 变量 | 默认值 | 含义 |
|---|---|---|
| `DSH_DESKTOP_BIN` | 未设置 | 插件拉起套壳时定位可执行文件；未设置则回落到行的 `bin` 配置、per-platform 随包二进制或 PATH。 |

## 常见问题

- **窗口显示静态错误页** — 直接运行二进制时没有带合法的 `--attach <url>`。从终端启动可读 stderr 提示；
  正常用法是 `dsh --profile desktop`（插件负责拉起套壳）。
- **构建报错 `failed to read plugin permissions … No such file or directory`** — 仓库在之前构建后被移动或改名，
  构建缓存里残留了旧绝对路径。在 `src-tauri` 下执行 `cargo clean`（或删除 `src-tauri/target`）后重试。
- **`dsh: command not found`** — 安装 CLI（`npm i -g @deepseek-ai/dsh`）。
- **`pnpm plugin:smoke` 提示 profile 未安装** — 先运行 `pnpm plugin:install`。

## 已知限制与后续工作

- **运行时未打包** — 套壳是 attach-only 形态，依赖单独安装的 dsh 与 desktop profile；把运行时嵌入单一
  应用分发的方案留待后续。
- **无 macOS 代码签名与公证** — `pnpm build` 产出未签名安装包；在本地使用之外分发需要签名凭据。
- **错误详情留在 stderr** — 失败窗口是静态的；从终端启动可读到诊断信息。
- **多平台二进制需要各 OS 的构建矩阵** — 当前平台的打包已实现（`pnpm package:current`）；其他平台的
  tgz 需要在对应操作系统上构建（或由 CI 矩阵完成）。

## 许可证与声明

MIT 许可证——见 [LICENSE](LICENSE)。

### DeepSeek Harness 声明

本项目由 [DeepSeek Harness](https://github.com/deepseek-ai/deepseek-harness)（`dsh`）驱动：套壳经 desktop
profile 插件由 dsh 运行时拉起，附着在运行时自身的回环 Web 服务上；它不打包、不分发运行时。运行时是
独立项目，按 MIT 许可证分发，Copyright (c) 2026 DeepSeek。

`icons-src/app-icon.svg` 与 `src-tauri/icons/` 下的应用图标源自 DeepSeek Harness Web favicon 图形
（[apps/web/public/favicon.svg](https://github.com/deepseek-ai/deepseek-harness/blob/master/apps/web/public/favicon.svg)），
按同一 MIT 许可证使用。

完整的 MIT 许可声明见 [harness 仓库](https://github.com/deepseek-ai/deepseek-harness)。

# dsh-desktop

[English](README.md) | 中文

DeepSeek Harness Web 界面的 Tauri 2 桌面套壳。dsh 插件
`@aqian0/dsh-desktop-plugin` 会在 Web 运行时绑定回环端口后，以
`dsh-desktop --attach <url>` 和运行时提供的鉴权回环 URL 打开桌面窗口。运行时仍是
父进程：关闭窗口即退出 profile，窗口也不会比运行时活得更久。

## 前置条件

- [`dsh`](https://github.com/deepseek-ai/deepseek-harness) CLI 0.1.2-rc.1 或更高版本：
  `npm i -g @deepseek-ai/dsh@next`
- 源码安装还需：Node.js + pnpm、Rust，以及
  [Tauri 平台依赖](https://tauri.app/start/prerequisites/)。

## 安装

### 快捷插件式安装

```sh
dsh plugin --profile desktop add @aqian0/dsh-desktop-plugin
dsh --profile desktop
```

插件包通过 optionalDependencies 附带各平台预编译的套壳二进制，无需安装 Rust
工具链。插件层还会把 `web-runtime.openBrowser` 固定为 `false`，因此
`dsh --profile desktop` 只打开桌面窗口，不会再拉起系统默认浏览器。套壳窗口内
同源的 Web 应用路由继续留在窗口内，跨源链接以及 `mailto:`/`tel:` 链接会交给
系统默认浏览器或应用打开。初始 URL 带一次性进程令牌，Web 运行时会将其换成
会话 Cookie。

如果 profile 是新建的且没有 Web bundle，请把
`~/.dsh/profiles/desktop/package.json` 中的 `dsh.profile.bundles` 设为：

```json
["@deepseek-ai/dsh-base", "@deepseek-ai/dsh-web-app", "@aqian0/dsh-desktop-plugin"]
```

`@deepseek-ai/dsh-web-app` 随已安装的 `dsh` 解析，无需从 npm 安装。

### 手动安装（源码）

```sh
git clone https://github.com/aqian0/dsh-desktop.git
cd dsh-desktop
pnpm install
pnpm shell:build
pnpm plugin:install   # 将 ./plugin 加入 desktop profile 并配置 bundles
DSH_DESKTOP_BIN=src-tauri/target/debug/dsh-desktop dsh --profile desktop
```

`pnpm plugin:install` 可重复执行；拉取更新后重跑
`pnpm shell:build && pnpm plugin:install`。

## 构建与打包

```sh
pnpm build:no-bundle              # 仅构建 release 二进制
pnpm build                        # release 二进制 + 平台安装包
pnpm package:current -- --build   # 打包插件与当前平台二进制，用于 npm 分发
pnpm plugin:smoke                 # 无 GUI 的生命周期冒烟测试
```

## 桌面启动与输入焦点

macOS、Windows 和 Linux 使用同一套启动顺序：

1. 隐藏创建原生窗口，不请求窗口和 WebView 的初始聚焦。
2. 等待 Tauri `Ready` 及随后一轮事件处理完成，只请求一次显示窗口。
3. 确认窗口已可见且未最小化后，一次性请求原生窗口与 WebView 聚焦，结束启动处理。

Linux 的 GTK 显示请求是异步的，因此会在显示前恢复 GTK 的可聚焦状态，并在后续
事件轮次确认可见，避免窗口尚未显示就请求聚焦而被忽略。macOS 则借此将窗口呈现
移出 AppKit 的启动回调；WKWebView 在创建时仍可能激活应用。

此流程不等待页面加载，也适用于启动错误页。收到关闭或销毁事件、检测到最小化状态，
或等待窗口可见期间收到主窗口失焦事件时，会取消待执行的聚焦。套壳不会通过定时器
抢焦点，也不会在页面刷新、切换应用或屏幕后重新执行启动处理。激活结果仍由
Windows 前台限制、Linux 窗口管理器及 Wayland 合成器策略决定；API 调用成功不代表
系统一定允许应用进入前台。

修改源码不会更新已安装的预编译二进制。macOS/Linux 从项目根目录测试本地版本：

```sh
pnpm shell:build
DSH_DESKTOP_BIN="$PWD/src-tauri/target/debug/dsh-desktop" dsh --profile desktop
```

Windows PowerShell：

```powershell
pnpm shell:build
$env:DSH_DESKTOP_BIN = Join-Path $PWD "src-tauri/target/debug/dsh-desktop.exe"
dsh --profile desktop
```

运行跨平台启动状态测试：`cargo test --locked --manifest-path src-tauri/Cargo.toml`。
打包和发布矩阵也会在各平台原生构建环境中先运行测试，再进行打包。

手动回归检查（需要各平台真实桌面，单元测试不能代替）：

- 分别从主屏、副屏重复冷启动，覆盖混合缩放、macOS Spaces、Windows 前台限制和
  Linux X11/Wayland；系统允许激活时，应能直接点击按钮、输入文字，无需先点击其他屏幕。
- 启动期间和启动后切换应用、刷新页面、最小化或关闭窗口，确认不会反复抢回焦点或恢复窗口。
- 页面加载缓慢、不可用以及未传 `--attach`（启动错误页）时，窗口仍应出现。
- 单屏启动、关闭窗口退出 profile、退出运行时关闭窗口的行为应保持正常。

## 许可证

MIT——见 [LICENSE](LICENSE)。`icons-src/` 与 `src-tauri/icons/` 下的应用图标源自
DeepSeek Harness Web favicon，按同一 MIT 许可证使用。

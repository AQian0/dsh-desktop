# dsh-desktop

[English](README.md) | 中文

DeepSeek Harness Web 界面的 Tauri 2 桌面套壳。dsh 插件
`@aqian0/dsh-desktop-plugin` 会在 Web 运行时绑定回环端口后，以
`dsh-desktop --attach http://127.0.0.1:<port>` 打开一个桌面窗口。运行时仍是
父进程：关闭窗口即退出 profile，窗口也不会比运行时活得更久。

## 前置条件

- [`dsh`](https://github.com/deepseek-ai/deepseek-harness) CLI：
  `npm i -g @deepseek-ai/dsh`
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
系统默认浏览器或应用打开。

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

## 许可证

MIT——见 [LICENSE](LICENSE)。`icons-src/` 与 `src-tauri/icons/` 下的应用图标源自
DeepSeek Harness Web favicon，按同一 MIT 许可证使用。

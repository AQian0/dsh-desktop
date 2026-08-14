# @aqian0/dsh-desktop-plugin

DeepSeek Harness 的桌面套壳 bundle:把 Web 界面开进一个 Tauri webview 窗口。
作为 dsh profile 的插件安装后,`dsh --profile desktop` 启动 Web 运行时并在其
绑定回环端口后打开桌面窗口;关闭窗口即优雅退出 profile。

## 安装

```sh
# 加入本插件(本地路径或发布后的包);profile 不存在时以 dsh-base 模板初始化
dsh plugin --profile desktop add /path/to/dsh-desktop/plugin

# 然后把 ~/.dsh/profiles/desktop/package.json 的 dsh.profile.bundles 调整为
# ["@deepseek-ai/dsh-base", "@deepseek-ai/dsh-web-app", "@aqian0/dsh-desktop-plugin"]
# 仓库脚本会自动完成上面两步:pnpm plugin:install

# 启动:配置树 = dsh-base → dsh-web-app → dsh-desktop-plugin → cordis.patch.yml
dsh --profile desktop
```

`@deepseek-ai/dsh-web-app` 只列入 bundles、**不作为 profile 依赖安装**:bundle
解析先走运行中的 dsh 安装目录,Web 界面因此永远与所装 dsh 版本一致(与内置
web 模板同一机制)。不要 `dsh plugin add @deepseek-ai/dsh-web-app`——registry
的 latest 标签指向依赖不完整的旧版本。

等价地,把 `dsh.profile.bundles` 直接写成
`["@deepseek-ai/dsh-base", "@deepseek-ai/dsh-web-app", "@aqian0/dsh-desktop-plugin"]`
即可——dsh 没有显式的 profile 继承,继承即 bundle 组合。

**验证**:仓库自带无 GUI 的双向生命周期冒烟 `pnpm plugin:smoke`——窗口关闭
方向(假壳退出 0 → profile 自行以码 0 退出)与运行时先死方向(SIGTERM dsh →
树释放 → 插件的 `ctx.effect` 清理钩子杀死假壳)。

## 套壳二进制解析

desktop-launch 行按以下顺序解析套壳可执行文件:

1. 行的 `bin` 配置(在 `~/.dsh/profiles/desktop/cordis.patch.yml` 里对
   `desktop-launch` 行写 `config: { bin: ... }` 即可固定);
2. 环境变量 `DSH_DESKTOP_BIN`(源码检出的开发路径);
3. 随包分发的 `bin/dsh-desktop`(规划中的 per-platform optionalDependencies);
4. PATH 上的 `dsh-desktop`。

解析失败即 fail-loud:stderr 报错并退出码 1。

## 与套壳二进制的关系

套壳二进制(`src-tauri`)是 attach-only 形态,只有一种启动方式:

`dsh-desktop --attach http://127.0.0.1:<port>`

它只开窗口,不启动也不监督任何运行时(运行时是父进程)。生命周期双向绑定:

- 窗口关闭即进程退出,由本插件请求 profile 优雅退出(带 5 秒有界强制兜底);
- 运行时先死(信号、崩溃)有三条并行的关闭链路:运行时树释放时本插件的
  `ctx.effect` 清理钩子直接 SIGTERM 壳(根 fiber 的 dispose 经 loader 级联到
  本行,已端到端验证);壳自身的 stdin 管道 EOF;以及 Unix 的父进程收养轮询
  (macOS 启动时会重接 stdio,故以轮询为准)。窗口不会比运行时活得更久。

不带合法 `--attach` 直接运行二进制会打印提示并显示静态错误窗口。窗口与
URL 解析逻辑都在壳内,插件只负责拉起时机与退出请求,不复制 UI。

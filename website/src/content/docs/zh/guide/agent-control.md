---
title: Agent 控制
description: 让本地命令行或 agent 驱动你交付的产品，开关始终握在用户手里。
sidebar:
  order: 10
---

`lxdev` 驱动的是**开发**会话。交付出去的产品则可以开放自己的本地接口，让命令行或
agent 直接驱动已安装的应用（macOS 与 Windows）。产品决定开放哪些面，用户决定接口
是否打开。

## 声明控制面

```yaml
capabilities:
  appUse: true       # 本产品自己的窗口
  computerUse: true  # 整台机器
  browserUse: true   # 本产品的内置浏览器（需要 `browser`）
```

这些能力由运行中的产品强制执行。`computerUse` 蕴含 `appUse`——能控制整机自然也能
触达产品自身窗口。`browserUse` 相互独立，且永远够不到外部的 Chrome、Edge 或
Safari 进程：那些是普通的机器窗口，需要 `computerUse`。被拒绝的命名空间就是最终
结论。

## 产品自身即命令

接口只属于启动该应用的用户，只在应用运行时应答，并且只开放产品声明的控制面。LingXia
不提供 `control enable`、`control disable` 或其他可由 agent 自行授予访问权的命令；开关
属于产品及其可信设置页面。

产品从 `HostAddon::start_services` 调用
`lingxia_control_runtime::local_control::install(enabled)`，传入产品自己持久化的偏好。尚未
交付设置页面时，可以明确用 `true` 作为过渡默认值；后续通过
`local_control::set_enabled` 即时应用变化，用 `local_control::is_enabled` 查询实时状态。

LingXia 把 endpoint 放在 `<app_data>/lingxia/control`，绝不会把 executable、locator 或
socket 写进宿主拥有的 `app_state`。

叶子命令用 `--help` 说明自己的语法，能给 `--json` 的优先用 `--json`。失败使用稳定
退出码——2 用法错误、3 未找到、4 有歧义、5 超时、6 权限或拒绝、7 不支持、8 不可用、
9 句柄失效、10 目标已解析但执行失败。

## 接入产品自己的 Agent 工具

LingXia 提供本地命令传输，但 Codex、Claude 或其他 Agent 集成由宿主产品自己负责。
Agent 工具用私有的 `--cli` 参数执行产品本身的准确 executable；框架不再生成 launcher，
也不假设 agent 能读取用户 shell 的 `PATH`。

release 构建可以把 `current_exe()` 原子写成产品 locator 中唯一一行，例如
`~/.<product>/path`。developer 构建不应覆盖 release locator；产品 skill 可以先解析一个
明确的开发覆盖变量（例如 `<PRODUCT>_PATH`），再读取 release locator。release、preview
与 developer 的 app-data 目录不同，因此 endpoint 已自然隔离。

LingXia 不选择 locator，也不生成会与产品业务规则发生漂移的 skill；二者都由宿主拥有
并分发。Agent 工具应在描述能力前查询运行中的产品。

Provider 应在 `HostAddon::install_product_cli` 中通过
`cli.command(name, about, handler)` 声明命令。LingXia 会在独立 CLI 进程解析参数之前
调用该 hook，此时 UI、service 与数据库都尚未初始化。对应的 App 内请求 namespace
放在 `install_host_apis`；等到 `start_services` 再注册任一侧都太晚。

产品 skill 会添加框架保留的 `--cli` 判别参数，LingXia 在内置命令或 provider
解析参数前将其移除。这样无 TTY 的 Agent 启动也会保持 CLI 模式，同时不会把框架参数
泄漏给产品命令。

## 权限与告知

在 macOS 上，`computerUse` 需要"辅助功能"与"屏幕录制"。命令在产品进程内执行，因此
macOS 把这两项授权归属到已安装的产品，而不是调用它的终端。做整机操作前应先检查：

```text
<product> computer permissions --json
```

第一条会产生改动的命令会打开一个可见的活动指示器：它跟随操作，静置一段时间后隐藏；
只读命令不会打开它。被控制的会话还会在整个会话期间保持一条常驻告知，包括只读阶段。
两者都不是 agent 命令——agent 绝不能隐藏或关闭它们。

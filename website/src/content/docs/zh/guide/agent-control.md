---
title: Agent 控制
description: 让本地命令行或 agent 驱动你交付的产品，开关始终握在用户手里。
sidebar:
  order: 9
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

接口只属于启动该应用的用户，只在应用运行时应答，并且在用户启用之前保持关闭。

```text
<product> control enable    # 打印 launcher 路径与需要加入 PATH 的那行配置
<product> control status    # 区分"正在监听"与"下次启动才生效"
<product> control disable   # 停止监听、持久化该状态、删除 socket
```

叶子命令用 `--help` 说明自己的语法，能给 `--json` 的优先用 `--json`。失败使用稳定
退出码——2 用法错误、3 未找到、4 有歧义、5 超时、6 权限或拒绝、7 不支持、8 不可用、
9 句柄失效、10 目标已解析但执行失败。

## 从运行中的构建生成技能

```text
<product> skills show
<product> skills install --agent claude   # 或 --agent codex
```

生成的技能只包含当前构建真正允许的入口，因此不会宣传产品拒绝提供的能力。连不上产品
时，`show` 与 `install` 会直接失败，而不是靠猜写出一份技能。安装会写入另一个 agent
的配置目录，所以它始终是一条显式的用户命令。

## 权限与告知

在 macOS 上，`computerUse` 需要"辅助功能"与"屏幕录制"。命令在产品进程内执行，因此
macOS 把这两项授权归属到已安装的产品，而不是调用它的终端。做整机操作前应先检查：

```text
<product> computer permissions --json
```

第一条会产生改动的命令会打开一个可见的活动指示器：它跟随操作，静置一段时间后隐藏；
只读命令不会打开它。被控制的会话还会在整个会话期间保持一条常驻告知，包括只读阶段。
两者都不是 agent 命令——agent 绝不能隐藏或关闭它们。

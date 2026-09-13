---
title: CLI
description: lingxia 与 lxdev 各自负责什么，以及为什么 flags 以 --help 为准。
sidebar:
  order: 12
---

LingXia 有两条命令。先记住分工；具体 flags 以 `--help` 为准。

| 命令 | 负责 | 常用子命令 |
|---|---|---|
| `lingxia` | 项目生命周期 | `new`、`doctor`、`dev`、`build`、`package`、`publish`、`upgrade` |
| `lxdev` | 已经运行的 `lingxia dev` 会话 | `lxapp`、`app`、`desktop`、`browser`、`test`、`logs`、`runner`、`session` |

`lingxia` 负责脚手架、构建、安装或启动，并维持经过认证的开发 WebSocket。`lxdev` 从不启动会话。它连上、执行一条命令、打印结果然后退出（`logs -f` 除外）。

```bash
lingxia --help
lingxia new --help
lingxia dev --background
lingxia dev status

lxdev --help
lxdev lxapp nav --help
lxdev logs -f
```

命令集合随项目类型变化。`lxdev <family> <command> --help` 是你当前会话、当前安装版本的完整列表。本站不重复这些 flags。

## 启动、接管、停止

```bash
lingxia dev                  # 交互式；本终端拥有会话
lingxia dev --background     # runtime websocket ready 后才返回
lingxia dev stop             # 在项目内停止同一平台的 owner
```

再次运行 `lingxia dev` 会接管同一项目、同一平台的旧会话。不同平台可以并行。只有一个 live session 时自动选中；有多个时必须在家族名之前写 `lxdev --session <id-or-target> …`。

## 原生宿主主界面

```bash
lingxia new my-app -t native-app -p macos,windows -y
lingxia new my-terminal -t native-app --main terminal --control native -y
lingxia new my-browser -t native-app -p windows --main browser --control lxapp -y
```

`--main` 与 `--control` 只用于原生宿主。见[构建形态](../what-you-build/)与[原生宿主应用](../native-host-apps/)。

## 接下来读

- [快速开始](../getting-started/) — 安装并创建项目
- [开发工作流](../development-workflow/) — 重载、自动化与验证
- [测试](../testing/) — `@lingxia/test` 与 `lxdev test`
- [Agent 控制](../agent-control/) — 已交付产品的命令行，不是 `lxdev`

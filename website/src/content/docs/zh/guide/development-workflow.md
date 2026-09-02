---
title: 开发工作流
description: 启动 LingXia dev session，按变更层重载，自动化交互并验证结果。
sidebar:
  order: 8
---

LingXia 把 session 生命周期与实时自动化分开：

- `lingxia dev` 负责构建、安装或启动，并拥有 dev session。
- `lxdev` 连接该 session，负责检查、重载、自动化、测试和读取日志。

## 启动 session

交互式终端使用：

```bash
lingxia dev
```

脚本与 agent 使用后台模式；命令会在 runtime websocket ready 后才返回：

```bash
lingxia dev --background
lingxia dev status
```

再次运行 `lingxia dev` 会接管同一项目、同一平台的旧 session。不同平台可以并行运行。需要停止 owner 时，在项目内执行 `lingxia dev stop`。

## 按变更层重载

| 修改内容 | 执行命令 |
|---|---|
| View、Logic 或 `lxapp.json` | `lxdev lxapp reload` |
| `lingxia.yaml`、native Rust 或平台工程 | 重新运行 `lingxia dev` |

`lxdev lxapp reload` 会重建 lxapp bundle 并重载正在运行的 lxapp，不创建新的 native session。

## 完成验证闭环

构建成功只是开始：

1. 用 `lxdev lxapp nav ...` 进入变更页面。
2. 用 `lxdev lxapp page click`、`type`、`fill` 或 `press` 真正触发行为。
3. 通过页面 DOM（`page eval` / `query`）或 Logic evaluation（`lxapp eval`）断言结果。
4. 查看 `lxdev logs`，确认没有新增 warning 与 error。

完整原生宿主画面使用 `lxdev app screenshot`，单个页面 WebView 使用 `lxdev lxapp page screenshot`。预期结果不是视觉效果时，优先断言具体值而不是看截图猜测。

## 八个命令家族

| 家族 | 目标 |
|---|---|
| `lxapp` | lxapp 生命周期、导航、页面自动化、Logic 与 View evaluation |
| `app` | 原生宿主窗口、全画面截图、底层鼠标与键盘输入 |
| `desktop` | 桌面本身：窗口、无障碍树、指针、键盘、剪贴板、像素 |
| `browser` | 应用内浏览器标签、DOM 自动化、cookie、截图 |
| `test` | 可重复的 API、页面与跨页面测试 case |
| `logs` | native、lxview、lxlogic、browser 与 automation 汇总日志 |
| `runner` | 桌面 Runner 呈现的模拟设备与外框 |
| `session` | 发现并选择 live sessions |

`desktop` 伸到应用之外——用例靠它回应系统对话框，或证明某个窗口真的占满了屏幕。

命令集合会随项目类型变化。精确 flags 始终以 `lxdev <family> <command> --help` 的当前安装版本输出为准。

## 多 session 选择

只有一个 live session 时，即使在项目目录之外运行，`lxdev` 也会自动选择。存在多个 session 时，它会拒绝猜测：

```bash
lxdev session list
lxdev --session ios lxapp current
```

全局 selector 必须写在命令家族之前。

## 把可重复行为沉淀为测试

`lxdev test` 配合 `@lingxia/test` 使用。API contract 放在 `tests/api/`，页面行为放在 `tests/pages/`，用户旅程放在 `tests/flows/`。一次性的视觉微调仍需实时交互与截图验证，但不一定要写永久测试。

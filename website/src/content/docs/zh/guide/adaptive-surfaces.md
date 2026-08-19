---
title: 自适应 surfaces
description: 只声明一次宿主 surface，由 LingXia 按可用尺寸决定实际呈现形态。
sidebar:
  order: 7
---

原生宿主在 `lingxia.yaml` 中用扁平的 `surfaces:` 列表描述 UI。你声明内容及其与主体验的关系；宿主根据可用尺寸把它实现为窗口、标签、停靠面板、全屏覆盖层或托盘弹窗。

## 内容键与角色

每个条目只能有一个内容键。它的值同时就是 surface identity；不存在额外的 `id` 或 `render` 字段。

| 内容键 | 打开的内容 | 支持的角色 |
|---|---|---|
| `lxapp` | 以 `appId` 标识的 lxapp | `main`、`aside`、`float` |
| `url` | 应用内浏览器页面；需要 `capabilities.browser` | `main`、`aside` |
| `native` | 宿主原生 surface；目前只有 `terminal` | `aside` |

角色描述关系，而不是平台控件：

- `main` 是顶层目的地；最多一个 lxapp main 可设 `launch: true`。
- `aside` 辅助当前 main；`edge` 与 `size` 是布局提示。
- `float` 是由托盘锚定的弹窗，因此必须包含 `tray:`。

## 一份有效声明

```yaml
capabilities:
  browser: true
  terminal: true

surfaces:
  - lxapp: my-home
    role: main
    launch: true
    tray:
      icon: icons/tray.svg
      label: My App
      action: activate

  - lxapp: assistant
    role: aside
    edge: right
    size: { width: 320 }

  - native: terminal
    role: aside
    edge: bottom
    platforms: [macos, windows]
```

每个 lxapp 还必须列在 `resources.bundles` 中，除非由 runtime 或 update provider 提供。`lingxia build` 会校验源配置并生成 `ui.json`；不要直接编辑 `ui.json`。

配置中不存在 `sidebar:` 字段。应用拥有的侧栏入口由 home lxapp 通过 `lx.shell.activators` 在运行时声明，每个回调显式打开 surface 或执行其他动作。用户拥有的 Pins 则刻意不向应用代码开放写入能力。

## 尺寸等级

lxapp 通过 `lx.onSurfaceContext` 获得自己的 surface viewport 等级：

| 尺寸等级 | viewport 宽度 |
|---|---:|
| `compact` | 小于 600 logical pixels |
| `medium` | 600 到 840 |
| `expanded` | 大于 840 |

这是 lxapp surface 的尺寸，不是设备类型判断，也不一定等于宿主窗口尺寸。宽桌面宿主里的 aside 仍可能是 `compact`。只改变布局时用 CSS/container query；组件树或交互模型变化时再使用 surface context。

在 shell 层，宽桌面布局可保留完整侧栏与多个 aside；medium 布局会收起侧栏并最多停靠一个 aside；compact 布局让 main 全屏，并把 aside 覆盖在其上。同一份声明驱动三种结果。

## 运行时打开 surface

`lx.openSurface` 根据 source 选择行为：

```ts
lx.openSurface({ surface: 'assistant' })
lx.openSurface({ url: 'https://example.com' })
lx.openSurface({ url: 'https://example.com', as: 'aside' })
lx.openSurface({ page: 'inspector', as: 'float' })
```

- `{ surface }` 打开 `lingxia.yaml` 声明的内容；其值是该声明的内容 identity。
- `{ url }` 打开普通应用内浏览器标签；`{ url, as: 'aside' }` 打开 browser aside。
- lxapp 自己的页面可作为无 chrome 的 `float`，或桌面 `window` 打开，但不能直接成为 `aside`。自己的侧栏面板应声明为 lxapp surface。
- `hide()` 保留状态，`close()` 销毁 surface。page overlay 的 form 在打开时确定，已声明 surface 则继续随 shell 自适应。

## 需要记住的构建规则

- 产品宿主需要一个 lxapp `main`；纯 `role: float` 托盘弹窗应用除外。
- `launch` 只可用于 lxapp `main`，且最多一个 main 启动。
- `edge` 与 `size` 只可用于 `aside`。
- `url` 需要 `capabilities.browser: true`。
- `native: terminal` 需要 `capabilities.terminal: true`，只接受 `top` / `bottom`，且仅桌面可用。
- `float` 必须带 `tray:`，并且最多一个 surface 可声明 tray。
- 托盘图标必须是相对宿主根目录的方形 SVG 源文件。

完整 schema 与托盘行为请安装 LingXia skill 并阅读 `app/project.md`；lxapp 响应式实现见 [LxApp 页面](../lxapp-pages/)。

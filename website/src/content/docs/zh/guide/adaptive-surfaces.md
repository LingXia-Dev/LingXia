---
title: 自适应 surfaces
description: 只声明一次宿主 surface，由 LingXia 按可用尺寸决定实际呈现形态。
sidebar:
  order: 8
---

原生宿主在 `lingxia.yaml` 中用扁平的 `surfaces:` 列表描述 UI。你声明内容及其与主体验的关系；宿主根据可用尺寸把它实现为窗口、标签、停靠面板、全屏覆盖层或托盘弹窗。

## 内容键与角色

每个条目只能有一个内容键。它的值同时就是 surface identity；不存在额外的 `id` 或 `render` 字段。

| 内容键 | 打开的内容 | 支持的角色 |
|---|---|---|
| `lxapp` | 以 `appId` 标识的 lxapp | `main`、`aside`、`float` |
| `url` | 应用内浏览器页面；需要 `capabilities.browser` | macOS / Windows 上可作为 `main`；宿主支持停靠时也可为 `aside` |
| `native` | 宿主原生 surface：`terminal` 或 `browser` | `terminal`：桌面上可为 `main` / `aside`；`browser`：macOS / Windows 上可为 `main` |

角色描述关系，而不是平台控件：

- `main` 是顶层目的地；最多一个 `main` 可设 `launch: true`。
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

配置中不存在 `sidebar:` 字段。应用拥有的侧栏入口由 [Control app](../control-app/) 通过 `lx.shell.sidebarActions`（`replace`、`update`、`remove`、`clear`）在运行时声明，每个回调显式打开 surface 或执行其他动作。用户拥有的 Pins 则刻意不向应用代码开放写入能力。

## 尺寸等级

lxapp 通过 `lx.surface.onContext` 获得自己的 surface viewport 等级：

| 尺寸等级 | viewport 宽度 |
|---|---:|
| `compact` | 小于 600 logical pixels |
| `regular` | 600 及以上 |

这是 lxapp surface 的尺寸，不是设备类型判断，也不一定等于宿主窗口尺寸。宽桌面宿主里的 aside 仍可能是 `compact`。`regular` 不是桌面：要和 `isMobile()` / `isDesktop()` 一起用（平板和折叠屏手机都是 mobile）。展开折叠屏会改 `sizeClass`，不会改宿主形态。平板是 `regular` + mobile（系统分屏很窄时是 `compact` + mobile），壳仍是手机投影，没有桌面侧栏。桌面 workspace View 是桌面宿主上的 `regular`；手持设备上的两栏是 mobile 上的 `regular`，用 CSS，不要第三档 size class。

宿主 shell 仍用独立的 `medium` 档做 chrome 仲裁（icon rail、最多一个停靠 aside）——那个名字不是页面尺寸档。

同一份声明在 shell 层会呈现为几种形态：

- **宽桌面** — 完整侧栏，main 旁可停靠多个 aside。
- **中等桌面** — 侧栏收成 icon rail，最多停靠一个 aside。
- **窄桌面** — icon rail 仍在，main 保持桌面 workspace；无法停靠的 aside 覆盖在 main 上。浏览器 chrome 留在顶部。
- **手机或平板 / 对应 Runner** — 侧栏消失，main 全屏，aside 覆盖其上。平板仍是这一列；多出来的宽度给页面两栏，不是桌面壳。

## 运行时打开 surface

`lx.surface` 按方法选择行为：

```ts
lx.surface.openDeclared('assistant')
lx.surface.openUrl('https://example.com')
lx.surface.openUrl('https://example.com', { as: 'aside' })
lx.surface.openPage('inspector', { as: 'float' })
lx.surface.openPage('editor', { as: 'window', chrome: 'full' })

const unsubscribe = lx.surface.onContext((context) => {
  this.setData({ surfaceContext: context })
})
```

- `openDeclared(id)` 打开 `lingxia.yaml` 声明的内容；`id` 是该声明的内容 identity。
- `openUrl(url)` 打开普通应用内浏览器标签；`{ as: 'aside' }` 把浏览器停靠为 aside。
- `openPage(page)` 把**本** lxapp 的页面作为无 chrome 的 `float`，或桌面 `window` 打开。页面不能成为 `aside`——自己的侧栏面板应声明为 lxapp surface。
- 提供贴边窗口前先问 `lx.supports({ capability: 'surface', value: 'window', chrome: 'full' })`。自定义 chrome 用 `var(--lx-page-chrome-top-inset)` 留白。
- `hide()` 保留状态，`close()` 销毁 surface。page overlay 的 form 在打开时确定，已声明 surface 则继续随 shell 自适应。
- `lx.surface.get(key)` 只返回本 lxapp **带 `key` 打开**的 surface 句柄。

`lx.openSurface` 与 `lx.onSurfaceContext` 已不存在。

## 需要记住的构建规则

- macOS 与 Windows 只允许一个声明的 `main`，内容可以是 `lxapp`、`url`、`native: terminal` 或 `native: browser`。其他目标目前仍要求 home lxapp 作为初始 main。
- 纯桌面托盘弹窗应用可以只声明一个带 `tray:` 的 `role: float`，不需要 main。
- `launch` 只可用于 `main`，且最多一个 main 启动。
- `edge` 与 `size` 只可用于 `aside`。
- `url` 需要 `capabilities.browser: true`。声明式 URL main 仅桌面可用。
- `native: terminal` 需要 `capabilities.terminal: true`；作为 aside 时只接受 `top` / `bottom`，且仅桌面可用。
- `native: browser` 需要 `capabilities.browser: true`，并支持 macOS / Windows main。
- `float` 必须带 `tray:`，并且每个目标平台最多一个 surface 可声明 tray。
- 托盘图标必须是相对宿主根目录的方形 SVG 源文件。

完整 schema 与托盘行为请安装 LingXia skill 并阅读 `app/project.md`；lxapp 响应式实现见 [LxApp 页面](../lxapp-pages/)。

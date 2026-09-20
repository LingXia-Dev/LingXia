---
title: LxApp 页面
description: 用分离的 View 与 Logic 文件、类型化 actions、原生组件和自适应状态构建页面。
sidebar:
  order: 5
---

lxapp 是有明确 View / Logic 边界的页面式应用。View 在 WebView 中渲染；Logic 独立运行，掌管需要持久的业务状态，并调用可移植的 `lx.*` 平台 API。

## 一条路由，两类文件

典型 React 页面包含：

```text
pages/home/
├── index.ts      # Logic：Page({ data, lifecycle, actions })
├── index.tsx     # View：React + useLxPage()
└── index.json    # 页面配置
```

Vue 使用 `index.vue`，HTML 项目使用 `index.html`。一个项目只选择一种 View framework；不要为同一路由同时创建三种实现。

## Logic 掌管状态与 actions

```ts
type PageData = { count: number }

Page({
  data: { count: 0 } as PageData,

  increment() {
    this.setData({ count: this.data.count + 1 })
  },
})
```

公开方法会成为 View 可调用的 action；生命周期 hooks 与 `_` 开头的辅助方法保持私有。`data` 中只放可序列化值，函数、DOM 节点和 unsubscribe handle 都不能穿过 bridge。

## View 订阅并派发动作

```tsx
import { useLxPage } from '@lingxia/react'

type PageActions = { increment(): Promise<void> }

export default function Home() {
  const { data, actions } = useLxPage<PageData, PageActions>()

  return <button onClick={() => actions.increment()}>{data.count}</button>
}
```

页面刚连接时，首个 bridge snapshot 可能为空。读取必需的嵌套数据前先 guard，或显示 skeleton。hover、popover 是否打开等临时展示状态留在 View；业务状态和必须跨 remount 保存的草稿留在 Logic。

## 类型与平台 API

把 `@lingxia/types` 安装为开发依赖。它在 Logic 中提供全局声明，`lx`、`Page`、`App` 都不需要 import。

```bash
npm install --save-dev @lingxia/types
```

Logic 包含 `fetch`、timer、URL、stream、console 等标准 Web API，但没有 DOM。可访问的域名和特权由宿主 grant，不写在 `lxapp.json` 里——没有 provider 时默认放行公网和全部特权；只有注册了 provider 才按 grant 收紧。`lingxia.yaml` 不配置 permission。现有非公网地址限制继续保留。

作用于**产品**本身的 API（退出、Dock 角标、shell 侧栏、宿主更新）只属于 [Control app](../control-app/)。同一个 `appId` 以 guest 打开时不能调用它们。

## 原生组件

LingXia 提供两类原生组件：

- **Inline native island** — `LxNativeRoot` 包裹 `LxVideo`，以及 `LxNativeCover` / `LxNativeView` / `LxNativeText` / `LxNativeButton`。`LxVideo` 必须是显式 `LxNativeRoot` 的**直接子节点**。裸写 `<LxVideo>` 会得到 `NATIVE_ROOT_INVALID_STRUCTURE`。
- **Presenters** — `LxPicker`、`LxMediaSwiper`、`LxNavigator`（不在 island 上）。

React 与 Vue 会重新导出这两类。HTML View 注册对应 custom element（`<lx-native-root>`、`<lx-video>` 等）。文本输入直接使用普通 `<input>` / `<textarea>`，不存在 `LxInput`。

```tsx
import { LxNativeRoot, LxVideo, LxPicker } from '@lingxia/react'

<LxNativeRoot className="player">
  <LxVideo src={data.src} aria-label={data.title} controls />
</LxNativeRoot>
```

组件 callback 并不统一：

| 组件 | React/Vue handler 收到的值 |
|---|---|
| Island 节点（`LxNativeButton`、`LxVideo`、Root） | **先给 payload** — `onPress(({ source }) => …)`、`onTimeUpdate(({ currentTime }) => …)`。HTML 仍读 `CustomEvent.detail`。 |
| `LxPicker` | **解析后的 value** — `string \| string[]` |
| `LxMediaSwiper` | 原始 DOM `CustomEvent` — `event.detail.index` |
| `LxNavigator` | 原始 DOM `CustomEvent` |

属性见生成的[组件参考](../../reference/components/)。Island 结构与 seek/controls 约定见 LingXia skill 的 `lxapp/components.md`。

## 适配 surface

间距与列数变化使用 CSS 或 container query；交互模型发生变化时，在 Logic 中订阅 `lx.surface.watchContext`，通过 `setData` 复制可序列化 context，再用 `sizeClass` **和** `isDesktop()` 选择 Compact 或 Workspace——`regular` 不是桌面。详见[自适应 surfaces](../adaptive-surfaces/)。

## 开发与验证

修改 View、Logic 或 `lxapp.json` 后，等正在运行的 `lingxia dev` 重建并重载。导航到变更页面并真实交互，在页面 DOM 或 Logic state 中断言结果，最后检查日志。完整闭环见[开发工作流](../development-workflow/)。

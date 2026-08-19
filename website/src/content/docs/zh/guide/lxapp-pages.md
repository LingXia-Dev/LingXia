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

Page<PageData>({
  data: { count: 0 },

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

Logic 包含 `fetch`、timer、URL、stream、console 等标准 Web API，但没有 DOM。网络 hostname 未列入 `lxapp.json` 时，请求会被拒绝：

```json
{
  "security": {
    "network": { "trustedDomains": ["api.example.com"] }
  }
}
```

## 原生组件

React 与 Vue 会重新导出 `LxPicker`、`LxVideo`、`LxMediaSwiper` 与 `LxNavigator`；HTML View 使用对应 custom-element tag。文本输入直接使用普通 `<input>` / `<textarea>`，不存在 `LxInput`。

组件 callback 并非刻意统一：picker wrapper 直接传解析后的 value，而 video、media-swiper、navigator handler 接收 DOM `CustomEvent`。属性见生成的[组件参考](../../reference/components/)；行为约定见 LingXia skill 的 `lxapp/components.md`。

## 适配 surface

间距与列数变化使用 CSS 或 container query；交互模型发生变化时，在 Logic 中订阅 `lx.onSurfaceContext`，通过 `setData` 复制可序列化 context，再选择 compact 或 workspace View。详见[自适应 surfaces](../adaptive-surfaces/)。

## 开发与验证

修改 View、Logic 或 `lxapp.json` 后，对实时 `lingxia dev` 会话运行 `lxdev lxapp reload`。导航到变更页面并真实交互，在页面 DOM 或 Logic state 中断言结果，最后检查日志。完整闭环见[开发工作流](../development-workflow/)。

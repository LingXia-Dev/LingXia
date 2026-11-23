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
├── index.ts      # Logic：definePage({ data, setup })
├── index.tsx     # View：React + useLxPage()
└── index.json    # 页面配置
```

HTML 项目使用 `index.html`。项目是 React 或 HTML；同一路由同时有两种页面文件时，在 `lxapp.json` 里设置 `"framework"`。

## Logic 掌管状态与 actions

```ts
import { definePage } from '@lingxia/types/page'

const page = definePage({
  data: { count: 0 },
  setup: page => ({
    actions: {
      increment() {
        page.update(d => { d.count += 1 })
      },
    },
  }),
})
export default page
```

只有 `setup` 在 `actions` 下返回的函数可由 View 调用；生命周期 hooks 与闭包里的其他内容保持私有。`data` 中只放可序列化值，函数、DOM 节点和 unsubscribe handle 都不能穿过 bridge。

## View 订阅并派发动作

```tsx
import { useLxPage } from '@lingxia/react'
import type page from './index'

export default function Home() {
  const { data, actions } = useLxPage<typeof page>()

  return <button onClick={() => actions.increment()}>{data.count}</button>
}
```

页面刚连接时，首个 bridge snapshot 可能为空。读取必需的嵌套数据前先 guard，或显示 skeleton。hover、popover 是否打开等临时展示状态留在 View；业务状态和必须跨 remount 保存的草稿留在 Logic。

## 类型与平台 API

把 `@lingxia/types` 安装为开发依赖。它在 Logic 中提供全局声明，`lx`、`App` 都不需要 import；`definePage` 从 `@lingxia/types/page` 引入。

```bash
npm install --save-dev @lingxia/types
```

Logic 包含 `fetch`、timer、URL、stream、console 等标准 Web API，但没有 DOM。可访问的域名和特权由宿主 grant，不写在 `lxapp.json` 里——没有 provider 时默认放行公网和全部特权；只有注册了 provider 才按 grant 收紧。`lingxia.yaml` 不配置 permission。现有非公网地址限制继续保留。

作用于**产品**本身的 API（退出、Dock 角标、shell 侧栏、宿主更新）只属于 [Control app](../control-app/)。同一个 `appId` 以 guest 打开时不能调用它们。

## 原生组件

LingXia 提供 `LxPicker`、`LxMediaSwiper`、`LxNavigator` 三个 presenter。React 会重新导出它们，HTML View 直接使用对应 custom element。文本输入直接使用普通 `<input>` / `<textarea>`，不存在 `LxInput`。WebView 页面中的视频和音频使用普通 HTML `<video>` / `<audio>`。

```tsx
import { LxPicker } from '@lingxia/react'
```

组件 callback 并不统一：

| 组件 | React handler 收到的值 |
|---|---|
| `LxPicker` | **解析后的 value** — `string \| string[]` |
| `LxMediaSwiper` | 原始 DOM `CustomEvent` — `event.detail.index` |
| `LxNavigator` | 原始 DOM `CustomEvent` |

属性见生成的[组件参考](../../reference/components/)。Callback 约定见 LingXia skill 的 `lxapp/webview/native-elements.md`。

## 适配 surface

间距与列数变化使用 CSS 或 container query；交互模型发生变化时，在 View 中读取 `env.sizeClass`（Logic 自己要处理变化时用 `page.env`），再用 `sizeClass` **和** `isDesktop()` 选择 Compact 或 Workspace——`regular` 不是桌面。详见[自适应 surfaces](../adaptive-surfaces/)。

## 开发与验证

修改 View、Logic 或 `lxapp.json` 后，等正在运行的 `lingxia dev` 重建并重载。导航到变更页面并真实交互，在页面 DOM 或 Logic state 中断言结果，最后检查日志。完整闭环见[开发工作流](../development-workflow/)。

---
title: 架构
description: 视图 / 桥 / 逻辑 的分离，让渲染与业务逻辑各居其位。
sidebar:
  order: 3
---

lxapp 是页面式小应用，边界严格：**视图**负责渲染，**逻辑**掌管状态与平台 API，一条 **Rust 桥**在两者之间传递数据与事件——界面工作不会与业务工作纠缠在一起。

## 三层

### 视图 View —— 运行于 WebView，负责渲染

React、Vue 或纯 HTML。它渲染复制而来的数据，掌管临时交互状态，并派发类型化 action。宿主专属的 `#[lingxia::native]` 路由是例外：View 通过 CLI 生成的 `@lingxia/native` 客户端调用它们。

### 桥 Bridge —— Rust 运行时，传递数据与事件

`setData`、流、通道与原生调用。连接两端的类型化接缝。数据从逻辑到视图、事件从视图到逻辑，唯一的通路就是桥。

### 逻辑 Logic —— JS 运行时或 Rust，掌管状态与 API

持久业务状态与可移植 `lx.*` 平台调用放在 JavaScript Logic；纯原生宿主与宿主扩展使用 **Rust** 获得原生能力。View 可以保存临时 UI 状态，但必须跨 remount 存活的状态属于 Logic。

```
┌─────────────┐     setData / 事件        ┌─────────────┐
│   视图 View │  ◄──────────────────────► │  逻辑 Logic │
│  (WebView)  │       经 Rust 桥           │  (JS/Rust)  │
│   只渲染    │                            │   掌管状态  │
└─────────────┘                            └─────────────┘
```

> 视图只渲染 · 逻辑掌状态 · 其余交给桥。

## 为什么这样分

- **渲染保持简单。** 视图是数据的纯函数——易于推理，也易于更换框架。
- **逻辑保持可移植。** 业务代码与平台 API 集中一处，在每个目标平台复用。
- **边界是类型化的。** `@lingxia/types` 描述 `Page({})`、`App({})` 与 `lx.*`；CLI 为宿主 Rust 路由生成 `@lingxia/native`。

## 下一步

- [构建形态](../what-you-build/) —— 选择独立 lxapp 或原生宿主应用，再按需扩展宿主。
- [LxApp 页面](../lxapp-pages/) —— 实现 View / Logic 边界。
- [快速开始](../getting-started/) —— 生成并运行项目。

---
title: 构建形态
description: 在独立 lxapp 与原生宿主应用之间选择，再按宿主需求接入 Rust 能力。
sidebar:
  order: 4
---

LingXia 有**两种项目形态**。Rust 原生代码是宿主应用内的扩展层，并不是第三种脚手架类型。

## 01 · 独立 lxapp

可在任意 LingXia 宿主中运行的页面式小应用。最适合纯界面与页面开发。

```bash
lingxia new my-lxapp -t lxapp -y
```

用 React、Vue 或 HTML 写视图；把状态与平台调用留在逻辑层。可在任意 LingXia Runner 或宿主应用中运行。

## 02 · 原生宿主应用

可安装的 Android / iOS / macOS / Windows / Harmony 应用，内嵌一个或多个 lxapp。多数产品交付的就是它。

```bash
lingxia new my-app -t native-app -p macos --package-id com.example.myapp -y
```

`-p` 接受逗号分隔的列表：`-p android,ios,macos,harmony` 或 `-p all`。

## 用 Rust 扩展宿主

在原生宿主中通过 `#[lingxia::native]` 与 `HostAddon` 提供宿主 API、后台服务、原生媒体或由 Rust 掌管的应用逻辑。

```rust
#[lingxia::native]
fn my_host_api(/* … */) { /* 原生逻辑 */ }
```

需要原生性能、后台工作，或可移植 `lx.*` 接口没有的平台能力时使用。View 通过 CLI 生成的 `@lingxia/native` 客户端调用原生路由；JS Logic 若需要跨页面业务辅助函数，则使用 `lingxia::js` extension。

## 如何组合

真实产品通常是一个原生宿主应用，内嵌一个 home lxapp，并可继续打开其他 bundled 或 runtime lxapp；需要原生能力的部分由 Rust 扩展支撑。视图 / 逻辑 / 桥 的分离仍然成立——见[架构](../architecture/)与 [LxApp 页面](../lxapp-pages/)。

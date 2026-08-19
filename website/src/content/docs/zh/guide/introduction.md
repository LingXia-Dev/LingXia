---
title: LingXia 是什么？
description: 由 Rust 驱动的跨平台应用运行时，用于页面式 lxapp 与原生宿主应用。
sidebar:
  order: 1
  label: LingXia 是什么？
---

LingXia（灵匣，意为"承光之器"）是一个**由 Rust 驱动的跨平台应用运行时**。用一套代码构建页面式 **lxapp** 和**原生宿主应用**，交付到 Android、iOS、macOS、Windows 与 HarmonyOS。

它的核心理念是**渲染**与**逻辑**的干净分离：

- **视图（View）** 负责渲染——React、Vue 或纯 HTML，运行于 WebView。
- **逻辑（Logic）** 掌管状态与平台 API——运行于 JS 运行时，或用 Rust 获得原生能力。
- **Rust 桥** 在两者之间传递数据与事件。

界面工作不会与业务工作纠缠，同一项目即可面向所有平台。

## 适合谁

- 需要把一个产品交付到多个原生平台、又不想维护多套并行代码的团队。
- 想要 Web 框架的开发体验、底层又要原生能力的开发者。
- 想用一流命令行（`new`、`dev`、`doctor`、`build`、`publish`）、而非手工拼接工具链的人。

## 下一步

- [快速开始](../getting-started/) —— 安装命令行并创建第一个项目。
- [架构](../architecture/) —— 视图 / 桥 / 逻辑 分离详解。
- [构建形态](../what-you-build/) —— 独立 lxapp、原生宿主应用，以及 Rust 扩展所在的位置。
- [开发工作流](../development-workflow/) —— 运行、重载、自动化并验证实时会话。

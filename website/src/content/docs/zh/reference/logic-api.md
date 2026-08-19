---
title: 关于 Logic JS API
description: lx.* Reference 的范围，以及 @lingxia/types 的自动生成链路。
sidebar:
  order: 0
---

是的，这里的 **Logic JS API** 指 lxapp 的 Logic context 中可用的 JavaScript / TypeScript 接口，主要入口是全局对象 `lx`。生成的 Reference 还包含 `Page({})`、`App({})`、生命周期、错误、句柄、参数与返回值等完整类型。

需要查询某个 `lx.*` 方法的准确签名时，从自动生成的 [`Lx` interface](../../reference/api/interfaces/lx/) 开始；需要理解架构与实际写法时，先读 [LxApp 页面开发](../../guide/lxapp-pages/)。

## 类型从哪里来

这套公开声明并不是另一份手写 API：

1. 与运行时对应的 struct 和 class 来自 `crates/lingxia-logic` 的 Rust bindings。
2. 语义 union、callback、handle、生命周期等仅存在于 TypeScript 的契约，也与 bindings 一起声明。
3. `rong-typegen` 自动生成 `packages/lingxia-types/src/generated/logic.ts` 和不依赖 DOM 的 Logic Web 声明。
4. 生成结果会提交到仓库并发布为 `@lingxia/types`，使用者不需要安装 Rust 或生成器。
5. Website 使用当前锁定安装的 `@lingxia/types` 声明入口运行 TypeDoc，在构建时产出 **Logic JS API** Reference。

:::note
自动生成页会保留 API 标识符和类型签名的原文。中文站中的提示表达的是“中英文共用同一份生成 Reference”，不是“此内容不支持你的语言”。
:::

## 不属于这里的内容

- View 侧框架绑定与 hooks 分别属于 `@lingxia/react`、`@lingxia/vue` 和 `@lingxia/html`。
- 原生组件集中在 [Components](../../reference/components/) 中。
- 原生宿主自定义 Rust route 使用 CLI 为具体项目生成的 `@lingxia/native` client，不属于全局 `lx.*` API。

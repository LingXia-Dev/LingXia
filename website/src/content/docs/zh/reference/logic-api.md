---
title: 关于 Logic JS API
description: lx.* Reference 的范围，以及 @lingxia/types 的自动生成链路。
sidebar:
  order: 0
---

是的，这里的 **Logic JS API** 指 lxapp 的 Logic context 中可用的 JavaScript / TypeScript 接口，主要入口是全局对象 `lx`。生成的 Reference 还包含 `Page({})`、`App({})`、生命周期、错误、句柄、参数与返回值等完整类型。

自动生成的 [Logic JS API](../../reference/api/) 参考按能力分组——导航、界面外壳、文件、媒体、设备、网络与宿主应用——列出每个 `lx.*` 成员的已发布签名与参数结构；需要理解架构与实际写法时，先读 [LxApp 页面开发](../../guide/lxapp-pages/)。

## 类型从哪里来

这套公开声明并不是另一份手写 API：

1. 与运行时对应的 struct 和 class 来自 `crates/lingxia-logic` 的 Rust bindings。
2. 语义 union、callback、handle、生命周期等仅存在于 TypeScript 的契约，也与 bindings 一起声明。
3. `rong-typegen` 自动生成 `packages/lingxia-types/src/generated/logic.ts` 和不依赖 DOM 的 Logic Web 声明。
4. 生成结果会提交到仓库并发布为 `@lingxia/types`，使用者不需要安装 Rust 或生成器。
5. Website 在构建时读取锁定安装的 `@lingxia/types` 声明，按能力分组产出 **Logic JS API** 页面。

:::note
自动生成页会保留 API 标识符和类型签名的原文。中文站中的提示表达的是“中英文共用同一份生成 Reference”，不是“此内容不支持你的语言”。
:::

这份参考描述的是**已发布**的包，而不是未发布的分支：页面会标明版本；上游新增或删除成员时，站点构建会直接失败，直到分组被更新。参数与返回值的准确类型就是该包里的类型——在编辑器里输入 `lx.` 并悬停成员，读到的就是你项目实际安装版本的同一份信息。

## 不属于这里的内容

- View 侧框架绑定与 hooks 分别属于 `@lingxia/react`、`@lingxia/vue` 和 `@lingxia/html`。
- 原生组件集中在 [Components](../../reference/components/) 中。
- 原生宿主自定义 Rust route 使用 CLI 为具体项目生成的 `@lingxia/native` client，不属于全局 `lx.*` API。

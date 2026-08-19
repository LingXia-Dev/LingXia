---
title: 测试
description: 用 @lingxia/test 编写可重复的 lxapp 用例，通过 lxdev test 在真实开发会话中运行。
sidebar:
  order: 9
---

LingXia 的测试跑在**真正运行的应用上**，而不是模拟环境里。`lingxia dev` 持有会话，`lxdev test` 把用例装载进去，驱动真实的 Logic 运行时和真实的页面 webview。所以用例通过意味着该行为在那个平台上确实可用，而不是一个 mock 和自己达成了一致。

用例用 `@lingxia/test` 编写，它是配套的编写 SDK。

## 安装

```bash
npm install --save-dev @lingxia/test
```

保持它与 CLI 同版本。两者协议共享，版本漂移时 `lxdev` 会告警。

## 写一个用例

一个用例是带标题和异步函数体的 `spec`。在其中通过 app 句柄驱动应用，用 `expect` 断言：

```ts
import { expect, spec } from '@lingxia/test';

spec('通过真实页面输入和 Logic 桥接完成问候', async () => {
  const app = myApp();

  await app.nav.relaunch({ page: 'home' });
  await app.page.waitFor({ page: 'home', css: '[data-testid="home-page"]' });

  await app.page.fill({ page: 'home', css: '[data-testid="name"]', text: 'Ada' });
  await app.page.click({ page: 'home', css: '[data-testid="greet"]' });

  expect(await app.page.text({ page: 'home', css: '[data-testid="greeting"]' }))
    .toContain('Ada');
});
```

两点值得注意。选择器就是针对你自己标记的普通 CSS —— 给元素加稳定的 `data-testid`，不要靠样式类去匹配。另外每次交互都要 await：用例是在和另一个进程对话，没有任何操作是同步的。

## 等条件，不要等时间

应用是活的，状态什么时候到就是什么时候到。等你真正关心的那个条件：

```ts
await app.page.waitFor({ page: 'cart', css: '[data-testid="total"]', state: 'visible' });
```

固定延时是"本机通过、CI 失败"最常见的根源 —— 设备慢一点就需要更久。等条件在应用快时不花时间，在应用慢时依然能通过。

## 按"会坏在哪"来组织

按用例保护的层次分开，这样失败本身就指明了层次：

| 目录 | 放什么 |
| --- | --- |
| `tests/api/` | Logic 契约 —— `lx.*` 返回什么、拒绝什么 |
| `tests/pages/` | 页面行为 —— 渲染、输入、页内导航 |
| `tests/flows/` | 跨页面的用户旅程 |

入口文件决定一次运行包含哪些用例，因此一个项目可以有多套：每次改动跑的快速套件，和发布前跑的完整套件。

## 运行

```bash
lxdev test tests/entries/all.test.ts
```

用 `--arg` 向运行传值，让一套用例覆盖多个平台：

```bash
lxdev test tests/entries/desktop.test.ts --arg platform=macos
```

结果边跑边输出，同时写成报告文件，CI 可以作为产物留存。

## 搁置但不删除

明知还没就绪的用例，声明出来比让它消失更有用 —— 它让缺口留在报告里：

```ts
spec.skip('恢复中断的上传', {
  reason: '需要重试 API',
});
```

## 什么行为值得写成永久测试

不是所有行为都值得。永久用例的价值在于守住不能悄悄改变的契约：某个 API 返回什么、页面拿到输入后做什么、一条旅程端到端保证了什么。一次性的视觉微调更适合直接看运行中的应用 —— 截图比对会在每次正当的设计调整时失败，最后只会教会团队忽略它。

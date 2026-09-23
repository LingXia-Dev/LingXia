---
title: 测试
description: 用 @lingxia/test 编写可重复的 lxapp 用例，通过 lxdev test 在真实开发会话中运行。
sidebar:
  order: 10
---

LingXia 的测试跑在**真正运行的应用上**，而不是模拟环境里。`lingxia dev` 持有会话，`lxdev test` 把用例装载进去，驱动真实的 Logic 运行时和真实的页面 webview。所以用例通过意味着该行为在那个平台上确实可用，而不是一个 mock 和自己达成了一致。

用例用 `@lingxia/test` 编写。spec body 跑在**目标设备**上的 JavaScript worker 里（真机或桌面 Runner），不在开发机上。因此 `fetch('http://127.0.0.1:...')` 打到的是设备回环，不是开发机。

## 安装

```bash
npm install --save-dev @lingxia/test
```

保持它与 CLI 同版本。两者协议共享，版本漂移时 `lxdev` 会告警。

## 写一个用例

一个用例是带标题的 `spec`，异步函数体收到测试句柄 `t`。用 locator 驱动应用，用可重试断言检查结果：

```ts
import { spec } from '@lingxia/test';

spec('通过真实页面输入和 Logic 桥接完成问候', async (t) => {
  await t.app.nav.relaunch({ page: 'home' });
  await t.expect(t.app.page.testId('home-page')).toBeVisible();

  await t.app.page.testId('name').fill('Ada');
  await t.app.page.testId('greet').click();

  await t.expect(t.app.page.testId('greeting')).toContain('Ada');
});
```

给元素加稳定的 `data-testid`，不要靠样式类去匹配。`t.app.page.testId(id)` 与 `t.app.page.css(selector)` 返回 locator；`t.expect(locator)` 会重试直到条件成立或超时。从 `@lingxia/test` 导入的 `expect(value)` 只检查一次，不会重试。

每次交互都要 await：用例是在和另一个进程对话。

## 等条件，不要等时间

应用是活的，状态什么时候到就是什么时候到。等你真正关心的那个条件：

```ts
await t.expect(t.app.page.testId('total')).toBeVisible();
await t.expect.poll(async () => {
  const response = await fetch(statusUrl);
  return (await response.json()).status;
}).toBe('submitted');
```

固定延时是“本机通过、CI 失败”最常见的根源。等条件在应用快时不花时间，在应用慢时依然能通过。用 `t.defer` 注册清理，成功或失败都会跑。

## 按“会坏在哪”来组织

按用例保护的层次分开，这样失败本身就指明了层次：

| 目录 | 放什么 |
| --- | --- |
| `tests/api/` | Logic 契约 —— `lx.*` 返回什么、拒绝什么 |
| `tests/pages/` | 页面行为 —— 渲染、输入、页内导航 |
| `tests/flows/` | 跨页面的用户旅程 |

入口文件决定一次运行包含哪些用例，因此一个项目可以有多套：每次改动跑的快速套件，和发布前跑的完整套件。

## 运行

```bash
lxdev test tests/pages/home.test.ts
lxdev test tests/ --grep checkout
```

用 `--arg` 向运行传值（`t.args`），让一套用例覆盖多个平台或 fixture URL：

```bash
lxdev test tests/flows/checkout.test.ts --arg platform=macos --arg statusUrl=https://…
```

报告会记录运行参数。看起来像凭据的键（`password`、`secret`、`token`、`apiKey`、`credential`）写成 `***`；其他键用 `--secret-arg key=value` 传入即可同样遮蔽。用例从 `t.args` 读到的仍是真实值。

结果边跑边输出，并写到 `test-results/<run-id>/`（`report.html`、`report.json`、`junit.xml`），CI 可以作为产物留存。

## 搁置但不删除

明知还没就绪的用例，声明出来比让它消失更有用 —— 它让缺口留在报告里：

```ts
spec.skip('恢复中断的上传', {
  reason: '需要重试 API',
});
```

只有运行时才知道用例是否适用时，在用例体内跳过。`t.skip(reason)` 立即结束该用例，并以该原因报告为 skipped —— 既不算通过也不算失败：

```ts
spec('重连离线客户端', async (t) => {
  const offline = await findOfflineClient(t);
  if (!offline) t.skip('该账号没有离线客户端');
  // …
});
```

`spec.fail` 声明已知有问题的用例：用例体的任何失败 —— 断言失败或产品抛出的错误 —— 都报告为预期失败（`xfail`）；用例体顺利完成则是意外通过（`xpass`），会让运行失败。

## 什么行为值得写成永久测试

不是所有行为都值得。永久用例的价值在于守住不能悄悄改变的契约：某个 API 返回什么、页面拿到输入后做什么、一条旅程端到端保证了什么。一次性的视觉微调更适合直接看运行中的应用 —— 截图比对会在每次正当的设计调整时失败，最后只会教会团队忽略它。

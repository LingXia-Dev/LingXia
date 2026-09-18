---
title: Control app
description: 哪个 lxapp 受信任、只有该 session 能调用什么，以及为什么 app id 不能当授权。
sidebar:
  order: 7
---

对用户来说产品是一个应用，对运行时则是多个 lxapp：宿主自带的 home、宿主打包的 Settings 屏幕，以及用户随后打开的 guest。它们共用同一套 `lx.*`，甚至可以共用 app id。**一次调用是否被允许，由宿主创建该 session 时的 class 决定**，而不是由正在运行的代码自称。

## 三种 session class

| Class | 哪个 session | 如何指定 |
|---|---|---|
| **Control app** | 产品自己的 home lxapp——每个产品最多一个，且仅当产品有 home | `lingxia.yaml` 的 `app.homeAppId`，打进构建 |
| **Control surface** | 宿主自己打包的 Settings 屏幕（例如终端设置） | 由 Control app 作为 surface 打开；必须是宿主打包的 lxapp |
| **Standard app** | guest、下载的 lxapp、用户打开的其他内容 | 默认 |

这些 class 互斥。把 home lxapp 在别处以 guest 再打开，得到的是同一个 app id、同一份代码的 Standard app。`--control native` 且没有 home lxapp 的桌面宿主，不存在 Control app session。

## 提供产品级界面前先检查

```ts
const control = lx.app.control
if (!control) return
await control.appearance.setPreference('dark')
```

`lx.app.control` 只存在于 Control app。同一答案也是 `lx.supports({ capability: 'control' })`。在 Settings 页顶部绑定一次，不要到处写 `lx.app.control!`。

## 只有 Control app 能调用的接口

这些作用于产品本身，而不是调用方 lxapp：

- `lx.app.exit()`、`lx.app.setBadge()`、`lx.app.cache`、`lx.app.checkUpdate()`、`lx.app.claimCustomUpdate()`、`lx.app.screenshot()`、`lx.app.autostart.*`
- `lx.app.control.displayLanguage` / `lx.app.control.appearance`（写入）
- `lx.shell.*` 的变更 —— `sidebarActions`、打开或重配已声明 surface

每个 lxapp 仍可**读取** `lx.app.displayLanguage.get()` 与 `lx.app.appearance.get()`，并应跟随这些值。本 lxapp 自己的包更新走 `lx.getUpdateManager()`。

拒绝结果是 `E_PERMISSION_DENIED`，并会点名本该被允许的 class。把它当成设计信号：让 Control app 去做产品级工作，不要冒充它。这不是用户关掉对话框，也不是缺少 capability（那个由 `lx.supports()` 回答）。

`process`、`downloads`、`automation` 等特权是宿主按 session 封存的 grant。`capabilities.process` 只授予 Control app，每一次 spawn 都会再检查当前 grant。

`homeAppId` 如何封进构建见[原生宿主应用](../native-host-apps/)；`lx.shell.sidebarActions` 见[自适应 surfaces](../adaptive-surfaces/)。

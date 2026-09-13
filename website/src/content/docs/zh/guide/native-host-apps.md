---
title: 原生宿主应用
description: 配置可安装的多平台宿主、embedded lxapp、capabilities、surfaces 与 Rust 扩展。
sidebar:
  order: 6
---

原生宿主应用是 Android、iOS、macOS、Windows 与 HarmonyOS 上可安装的产品外壳。它拥有 `lingxia.yaml`、各平台原生项目和 Rust host crate。多数产品还会内嵌一个 home lxapp；以桌面终端或浏览器为主界面的宿主可以不带这个 bundle。

## 生成唯一事实源

```bash
lingxia new my-app -t native-app -p macos,windows \
  --package-id com.example.myapp -y
```

主界面是内置终端或浏览器的桌面产品，可以不生成内嵌的控制 lxapp：

```bash
lingxia new my-terminal -t native-app --main terminal --control native -y
lingxia new my-browser -t native-app -p windows --main browser --control lxapp -y
```

`--main terminal|browser` 目前仅 macOS / Windows。`--control native` 会省略 `homeAppId`、`resources.bundles` 和 `lxapp/` 目录。`--control lxapp` 即使可见主界面是浏览器，也保留一个内嵌 lxapp 作为受信任的 [Control app](../control-app/)。

以生成的 `lingxia.yaml` 为准，查看当前 CLI 支持的精确字段。`lingxia build` 会把它编译成 runtime `app.json` 与 `ui.json`；这两个生成文件不是手工编辑入口。

## 对齐 home ids

宿主**确实**内嵌 home lxapp 时，三个值必须一致：

- `app.homeAppId`
- 某个 `resources.bundles[].appId`
- 该 bundle 的 `lxapp.json.appId`

启动 main surface 的 `lxapp:` 值也必须指向同一个 home app。未对齐会导致构建失败或启动错误内容。这个 home session 就是 Control app；其他 lxapp 即使共享同一个 app id，也只是 guest。

## Capabilities 与 surfaces

需要预先启用的宿主集成放进 capabilities。`capabilities.browser` 启用应用内浏览器，`terminal` 启用原生终端 surface，`process` 解锁受信任的桌面进程 API，`autostart` 暴露由用户控制的开机启动注册。camera 等普通 API 在调用时请求权限，不放在这里。

用顶层 `surfaces:` 列表描述 main、aside 与 tray 内容。当前 schema 见[自适应 surfaces](../adaptive-surfaces/)。

## JavaScript Logic 或纯原生 Rust

多数宿主保留 `features.appService: true`，并内嵌带 JS Logic 的普通 lxapp。纯原生宿主必须同时切换两端：

- `lingxia.yaml` 中 `features.appService: false`
- home `lxapp.json` 中 `"logic": false` —— 或以桌面 `native: terminal|browser` 为主界面时直接省略控制 lxapp

这种形态在仍有控制 lxapp 时使用 HTML-only View，由 Rust 替代 Logic。appService 被关闭时，启用 logic 的 lxapp 会在启动时被拒绝。

## 添加宿主专属 Rust API

用 `#[lingxia::native]` 定义宿主路由，经 `HostAddon` 注册，再由一次 native build 生成 View 使用的 `@lingxia/native` 客户端。这些路由不会被添加到 `lx.*`。若 JS Logic 需要跨页面复用的辅助函数，应暴露 `lingxia::js` extension。

## 环境与发布构建

`--env dev|prod` 选择宿主环境，包括 package-id suffix 与 server config；`--release` 选择 compiler profile。两者相互独立，可交付构建通常同时使用：

```bash
lingxia build --env prod --release
```

需要 staging 好的分发产物时使用 `lingxia package`。平台与签名 flags 以 `lingxia build --help` 和 `lingxia package --help` 的当前版本输出为准。

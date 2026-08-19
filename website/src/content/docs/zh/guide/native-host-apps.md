---
title: 原生宿主应用
description: 配置可安装的多平台宿主、embedded lxapp、capabilities、surfaces 与 Rust 扩展。
sidebar:
  order: 6
---

原生宿主应用是 Android、iOS、macOS、Windows 与 HarmonyOS 上可安装的产品外壳。它拥有 `lingxia.yaml`、各平台原生项目、Rust host crate 和一个 embedded home lxapp。

## 生成唯一事实源

```bash
lingxia new my-app -t native-app -p macos,windows \
  --package-id com.example.myapp -y
```

以生成的 `lingxia.yaml` 为准，查看当前 CLI 支持的精确字段。`lingxia build` 会把它编译成 runtime `app.json` 与 `ui.json`；这两个生成文件不是手工编辑入口。

## 对齐 home ids

三个值必须一致：

- `app.homeAppId`
- 某个 `resources.bundles[].appId`
- 该 bundle 的 `lxapp.json.appId`

启动 main surface 的 `lxapp:` 值也必须指向同一个 home app。未对齐会导致构建失败或启动错误内容。

## Capabilities 与 surfaces

需要预先启用的宿主集成放进 capabilities。`capabilities.browser` 启用应用内浏览器，`terminal` 启用原生终端 surface，`process` 解锁受信任的桌面进程 API，`autostart` 暴露由用户控制的开机启动注册。camera 等普通 API 在调用时请求权限，不放在这里。

用顶层 `surfaces:` 列表描述 main、aside 与 tray 内容。当前 schema 见[自适应 surfaces](../adaptive-surfaces/)。

## JavaScript Logic 或纯原生 Rust

多数宿主保留 `features.appService: true`，并内嵌带 JS Logic 的普通 lxapp。纯原生宿主必须同时切换两端：

- `lingxia.yaml` 中 `features.appService: false`
- home `lxapp.json` 中 `"logic": false`

这种形态使用 HTML-only View，由 Rust 替代 Logic。appService 被关闭时，启用 logic 的 lxapp 会在启动时被拒绝。

## 添加宿主专属 Rust API

用 `#[lingxia::native]` 定义宿主路由，经 `HostAddon` 注册，再由一次 native build 生成 View 使用的 `@lingxia/native` 客户端。这些路由不会被添加到 `lx.*`。若 JS Logic 需要跨页面复用的辅助函数，应暴露 `lingxia::js` extension。

## 环境与发布构建

`--env developer|preview|release` 选择环境 slot，包括 package-id suffix 与 server config；`--release` 选择 compiler profile。两者相互独立，可交付构建通常同时使用：

```bash
lingxia build --env release --release
```

需要 staging 好的分发产物时使用 `lingxia package`。平台与签名 flags 以 `lingxia build --help` 和 `lingxia package --help` 的当前版本输出为准。

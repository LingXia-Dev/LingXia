---
title: What you build
description: Choose a standalone lxapp or a native host app, then add Rust capabilities where the host needs them.
sidebar:
  order: 4
---

LingXia has **two project shapes**. Rust native code is an extension layer inside a host app, not a third scaffold type.

## 01 · Standalone lxapp

A page-based mini-app that runs inside any LingXia host. Perfect for pure UI and page work.

```bash
lingxia new my-lxapp -t lxapp -y
```

Write Views in React, Vue, or HTML; keep state and platform calls in Logic. Runs in any LingXia Runner or host app.

## 02 · Native host app

An installable Android / iOS / macOS / Windows / Harmony app embedding one or more lxapps. This is what most products ship.

```bash
lingxia new my-app -t native-app -p macos --package-id com.example.myapp -y
```

`-p` accepts a comma-separated list: `-p android,ios,macos,harmony` or `-p all`.

### Terminal- or browser-main products

On macOS and Windows the launch screen can be a built-in native surface instead of an lxapp:

```bash
lingxia new my-terminal -t native-app --main terminal --control native -y
```

`--main terminal` (or `--main browser`) makes that surface the main screen, and `--control native` leaves out the embedded control lxapp. It is still a native host app — it can open bundled or runtime lxapps later, and an lxapp reaches the same terminal engine through `lx.terminal` where the host enables it.

## Extend a host with Rust

Add host APIs, background services, native media, or Rust-owned app logic to a native host with `#[lingxia::native]` and `HostAddon`.

```rust
#[lingxia::native]
fn my_host_api(/* … */) { /* native logic */ }
```

Use this for native performance, background work, or platform capabilities not exposed by the portable `lx.*` surface. Native routes are called from the View through the CLI-generated `@lingxia/native` client. Cross-page helpers for JS Logic use a `lingxia::js` extension instead.

## How they combine

A real product is usually a native host app that embeds one home lxapp and may open more bundled or runtime lxapps. Rust extensions back the parts that need native power. The View / Logic / Bridge split remains the same — see [Architecture](../architecture/) and [LxApp pages](../lxapp-pages/).

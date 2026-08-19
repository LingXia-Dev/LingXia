---
title: Architecture
description: The View / Bridge / Logic split that keeps rendering and business logic apart.
sidebar:
  order: 3
---

An lxapp is a page-based mini-app with a strict boundary. The **View** renders. The **Logic** owns state and platform APIs. A **Rust bridge** moves data and events between them — so UI work never tangles with business work.

## The three layers

### View — runs in WebView, owns rendering

React, Vue, or plain HTML. It renders replicated data, owns transient interaction state, and dispatches typed actions. Host-specific `#[lingxia::native]` routes are the exception: the View calls them through the CLI-generated `@lingxia/native` client.

### Bridge — Rust runtime, moves data & events

`setData`, streams, channels, and native calls. The typed seam between the two worlds. The bridge is the only path data takes from Logic to View and events take from View to Logic.

### Logic — JS runtime or Rust, owns state & APIs

Durable business state and portable `lx.*` platform calls live in JavaScript Logic. Native-only hosts and host extensions use **Rust** for native power. The View may keep transient UI state, but state that must survive remounts belongs in Logic.

```
┌─────────────┐     setData / events      ┌─────────────┐
│    View     │  ◄──────────────────────► │    Logic    │
│  (WebView)  │      via Rust bridge       │  (JS/Rust)  │
│  renders    │                            │ owns state  │
└─────────────┘                            └─────────────┘
```

> View code renders · Logic code owns state · the bridge carries the rest.

## Why the split matters

- **Rendering stays simple.** The View is a pure function of data — easy to reason about and easy to swap frameworks.
- **Logic stays portable.** Business code and platform APIs live in one place, reused across every target.
- **The boundary is typed.** `@lingxia/types` describes `Page({})`, `App({})`, and `lx.*`; the CLI generates `@lingxia/native` for host Rust routes.

## Next

- [What you build](../what-you-build/) — choose a standalone lxapp or native host app, then extend the host where needed.
- [LxApp pages](../lxapp-pages/) — implement the View / Logic boundary.
- [Getting started](../getting-started/) — scaffold and run a project.

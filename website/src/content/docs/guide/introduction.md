---
title: What is LingXia?
description: A Rust-powered cross-platform app runtime for page-based lxapps and native host apps.
sidebar:
  order: 1
  label: What is LingXia?
---

LingXia (灵匣, "luminous vessel") is a **Rust-powered cross-platform app runtime**. You build page-based **lxapps** and **native host apps** from a single codebase, and ship them to Android, iOS, macOS, Windows, and HarmonyOS.

The defining idea is a clean split between **rendering** and **logic**:

- The **View** renders — React, Vue, or plain HTML, running in a WebView.
- The **Logic** owns state and platform APIs — in a JS runtime, or in Rust for native power.
- A **Rust bridge** moves data and events between them.

UI work never tangles with business work, and the same project targets every platform.

## Who it's for

- Teams shipping one product to several native platforms without maintaining parallel codebases.
- Developers who want web-framework ergonomics for the View but native power underneath.
- Anyone who wants a first-class CLI (`new`, `dev`, `doctor`, `build`, `publish`) instead of hand-wiring toolchains.

## Next steps

- [Getting started](../getting-started/) — install the CLI and create your first project.
- [Architecture](../architecture/) — the View / Bridge / Logic split in detail.
- [What you build](../what-you-build/) — standalone lxapp vs. native host app, and where Rust extensions fit.
- [Development workflow](../development-workflow/) — run, reload, automate, and verify a live session.

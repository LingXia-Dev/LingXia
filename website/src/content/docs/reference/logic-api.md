---
title: About the Logic JS API
description: What the lx.* reference covers and how @lingxia/types is generated.
sidebar:
  order: 0
---

Yes—the **Logic JS API** is the JavaScript/TypeScript surface available in an lxapp's Logic context. Its main entry point is the global `lx` object. The generated reference also includes the `Page({})` and `App({})` contracts, lifecycle types, errors, handles, options, and results that describe that environment.

Start with the generated [`Lx` interface](../api/interfaces/lx/) when you need the exact signature of an `lx.*` method. For the architecture and practical usage pattern, read [LxApp pages](../../guide/lxapp-pages/).

## Where the types come from

The public declarations are not maintained as a second handwritten API tree:

1. Runtime-backed structs and classes originate in the Rust bindings under `crates/lingxia-logic`.
2. TypeScript-only contracts—such as semantic unions, callbacks, handles, and lifecycle metadata—are declared alongside those bindings.
3. `rong-typegen` generates `packages/lingxia-types/src/generated/logic.ts` and the DOM-free Logic Web declarations.
4. The generated outputs are committed and published as `@lingxia/types`, so consumers do not need Rust or the generator.
5. This website runs TypeDoc against the pinned installed `@lingxia/types` declaration entry point and builds the **Logic JS API** section from it.

:::note
The generated pages keep identifiers and type signatures in their source language. When browsing them in Chinese, the information notice means “shared generated reference,” not “this content is unsupported.”
:::

## What it does not cover

- View framework bindings and hooks live in `@lingxia/react`, `@lingxia/vue`, and `@lingxia/html`.
- Native-backed elements are listed under [Components](../components/).
- A native host's custom Rust routes use the CLI-generated `@lingxia/native` client; they are app-specific and are not part of the global `lx.*` reference.

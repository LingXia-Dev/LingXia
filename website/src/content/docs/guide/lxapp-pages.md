---
title: LxApp pages
description: Build a page with separate View and Logic files, typed actions, native components, and adaptive state.
sidebar:
  order: 5
---

An lxapp is a page-based application with a deliberate View / Logic boundary. The View renders in a WebView; Logic runs separately, owns durable business state, and calls the portable `lx.*` platform API.

## One route, two files

A React page typically contains:

```text
pages/home/
├── index.ts      # Logic: Page({ data, lifecycle, actions })
├── index.tsx     # View: React + useLxPage()
└── index.json    # page configuration
```

Vue uses `index.vue`; HTML projects use `index.html`. A project selects one View framework—do not create all three variants for a route.

## Logic owns state and actions

```ts
type PageData = { count: number }

Page({
  data: { count: 0 } as PageData,

  increment() {
    this.setData({ count: this.data.count + 1 })
  },
})
```

Public methods become View-callable actions. Lifecycle hooks and `_`-prefixed helpers stay private. Keep values in `data` serializable; functions, DOM nodes, and unsubscribe handles do not cross the bridge.

## The View subscribes and dispatches

```tsx
import { useLxPage } from '@lingxia/react'

type PageActions = { increment(): Promise<void> }

export default function Home() {
  const { data, actions } = useLxPage<PageData, PageActions>()

  return <button onClick={() => actions.increment()}>{data.count}</button>
}
```

The first bridge snapshot may be empty while the page connects. Guard required nested data or render a skeleton until it exists. Keep transient presentation state such as hover or an open popover in the View; keep business state and drafts that must survive remounts in Logic.

## Types and platform APIs

Install `@lingxia/types` as a development dependency. Its declarations are global in Logic—there is no import for `lx`, `Page`, or `App`.

```bash
npm install --save-dev @lingxia/types
```

Logic includes standard Web APIs such as `fetch`, timers, URL, streams, and console, but it has no DOM. Network hosts and privilege classes are host grants, not `lxapp.json` fields — the default (no provider) allows public network and every privilege class; a registered provider is the only path that restricts. `lingxia.yaml` carries no permission settings. Existing non-public address restrictions still apply.

APIs that act on the **product** (quit, dock badge, shell sidebar, host update) are [Control app](../control-app/) only. The same `appId` opened as a guest cannot call them.

## Native-backed components

LingXia ships two families:

- **Inline native island** — `LxNativeRoot` wraps `LxVideo`, plus `LxNativeCover` / `LxNativeView` / `LxNativeText` / `LxNativeButton`. `LxVideo` must be a **direct child** of an explicit `LxNativeRoot`. A bare `<LxVideo>` is `NATIVE_ROOT_INVALID_STRUCTURE`.
- **Presenters** — `LxPicker`, `LxMediaSwiper`, `LxNavigator` (not on the island).

React and Vue re-export both families. HTML Views register the custom elements (`<lx-native-root>`, `<lx-video>`, …). Text input is normal web `<input>` / `<textarea>`—there is no `LxInput`.

```tsx
import { LxNativeRoot, LxVideo, LxPicker } from '@lingxia/react'

<LxNativeRoot className="player">
  <LxVideo src={data.src} aria-label={data.title} controls />
</LxNativeRoot>
```

Component callbacks are not uniform:

| Component | What the React/Vue handler receives |
|---|---|
| Island nodes (`LxNativeButton`, `LxVideo`, Root) | **Payload first** — `onPress(({ source }) => …)`, `onTimeUpdate(({ currentTime }) => …)`. HTML still reads `CustomEvent.detail`. |
| `LxPicker` | **Resolved value** — `string \| string[]` |
| `LxMediaSwiper` | Raw DOM `CustomEvent` — `event.detail.index` |
| `LxNavigator` | Raw DOM `CustomEvent` |

Use the generated [Components reference](../../reference/components/) for attributes. Island structure and seek/controls contracts live in the LingXia skill's `lxapp/components.md`.

## Adapt to the surface

Use CSS or container queries for spacing and column changes. When the interaction model changes, subscribe in Logic with `lx.surface.watchContext`, replicate the serializable context through `setData`, and select Compact versus Workspace with `sizeClass` **and** `isDesktop()` — `regular` is not desktop. See [Adaptive surfaces](../adaptive-surfaces/).

## Develop and verify

After editing View, Logic, or `lxapp.json`, wait for the live `lingxia dev` session to rebuild and reload. Navigate and interact with the changed page, assert the result in the page DOM or Logic state, and check logs. The complete loop is in [Development workflow](../development-workflow/).

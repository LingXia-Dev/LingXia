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

Page<PageData>({
  data: { count: 0 },

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

Logic includes standard Web APIs such as `fetch`, timers, URL, streams, and console, but it has no DOM. Network access is denied unless the hostname is listed in `lxapp.json`:

```json
{
  "security": {
    "network": { "trustedDomains": ["api.example.com"] }
  }
}
```

## Native-backed components

React and Vue re-export `LxPicker`, `LxVideo`, `LxMediaSwiper`, and `LxNavigator`. HTML Views use their custom-element tags. Text input is normal web `<input>` / `<textarea>`—there is no `LxInput`.

Component callbacks are intentionally not uniform: picker wrappers pass the resolved value, while video, media-swiper, and navigator handlers receive a DOM `CustomEvent`. Use the generated [Components reference](../../reference/components/) for attributes and the LingXia skill's `lxapp/components.md` for behavioral contracts.

## Adapt to the surface

Use CSS or container queries for spacing and column changes. When the interaction model changes, subscribe in Logic with `lx.onSurfaceContext`, replicate the serializable context through `setData`, and select the compact or workspace View. See [Adaptive surfaces](../adaptive-surfaces/).

## Develop and verify

After editing View, Logic, or `lxapp.json`, run `lxdev lxapp reload` against the live `lingxia dev` session. Navigate and interact with the changed page, assert the result in the page DOM or Logic state, and check logs. The complete loop is in [Development workflow](../development-workflow/).

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
├── index.ts      # Logic: definePage({ data, setup })
├── index.tsx     # View: React + useLxPage()
└── index.json    # page configuration
```

HTML projects use `index.html`. A project is React or HTML — set `"framework"` in `lxapp.json` when both page files exist.

## Logic owns state and actions

```ts
import { definePage } from '@lingxia/types/page'

const page = definePage({
  data: { count: 0 },
  setup: page => ({
    actions: {
      increment() {
        page.update(d => { d.count += 1 })
      },
    },
  }),
})
export default page
```

Only what `setup` returns under `actions` is View-callable; lifecycle hooks and everything else in its closure stay private. Keep values in `data` serializable; functions, DOM nodes, and unsubscribe handles do not cross the bridge.

## The View subscribes and dispatches

```tsx
import { useLxPage } from '@lingxia/react'
import type page from './index'

export default function Home() {
  const { data, actions } = useLxPage<typeof page>()

  return <button onClick={() => actions.increment()}>{data.count}</button>
}
```

The first bridge snapshot may be empty while the page connects. Guard required nested data or render a skeleton until it exists. Keep transient presentation state such as hover or an open popover in the View; keep business state and drafts that must survive remounts in Logic.

## Types and platform APIs

Install `@lingxia/types` as a development dependency. Its declarations are global in Logic—there is no import for `lx` or `App`; `definePage` comes from `@lingxia/types/page`.

```bash
npm install --save-dev @lingxia/types
```

Logic includes standard Web APIs such as `fetch`, timers, URL, streams, and console, but it has no DOM. Network hosts and privilege classes are host grants, not `lxapp.json` fields — the default (no provider) allows public network and every privilege class; a registered provider is the only path that restricts. `lingxia.yaml` carries no permission settings. Existing non-public address restrictions still apply.

APIs that act on the **product** (quit, dock badge, shell sidebar, host update) are [Control app](../control-app/) only. The same `appId` opened as a guest cannot call them.

## Native-backed components

LingXia ships the presenters `LxMediaSwiper` and `LxNavigator`. React re-exports them; HTML Views use the custom elements. Text input and form pickers use normal web `<input>`, `<textarea>`, `<select>`, and `<input type="date|time">`—in GPU View pages, use `<input>`, `<select>`, or `@liulidesign/core`. In WebView pages, play video and audio with plain HTML `<video>` / `<audio>`.

```tsx
import { LxMediaSwiper, LxNavigator } from '@lingxia/react'
```

Component callbacks are not uniform:

| Component | What the React handler receives |
|---|---|
| `LxMediaSwiper` | Payload first in React — `{ index, ... }` |
| `LxNavigator` | Raw DOM `CustomEvent` |

Use the generated [Components reference](../../reference/components/) for attributes. Callback contracts live in the LingXia skill's `lxapp/webview/native-elements.md`.

## Adapt to the surface

Use CSS or container queries for spacing and column changes. When the interaction model changes, read `env.sizeClass` in the View (Logic has `page.env` when it acts on a change itself) and select Compact versus Workspace with `sizeClass` **and** `isDesktop()` — `regular` is not desktop. See [Adaptive surfaces](../adaptive-surfaces/).

## Develop and verify

After editing View, Logic, or `lxapp.json`, wait for the live `lingxia dev` session to rebuild and reload. Navigate and interact with the changed page, assert the result in the page DOM or Logic state, and check logs. The complete loop is in [Development workflow](../development-workflow/).

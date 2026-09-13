---
title: Adaptive surfaces
description: Declare host surfaces once and let LingXia realize them for the available screen size.
sidebar:
  order: 8
---

A native host describes its UI as a flat `surfaces:` list in `lingxia.yaml`. You declare the content and its relationship to the main experience; the host realizes it as a window, tab, docked panel, full-screen overlay, or tray popover according to the available size.

## Content keys and roles

Every entry has exactly one content key. Its value is also the surface identity; there is no separate `id` or `render` field.

| Content key | What it opens | Supported roles |
|---|---|---|
| `lxapp` | An lxapp by `appId` | `main`, `aside`, `float` |
| `url` | An in-app browser page; requires `capabilities.browser` | `main` on macOS/Windows; `aside` where the host docks browser asides |
| `native` | A host-native surface: `terminal` or `browser` | `terminal`: `main` / `aside` on desktop; `browser`: `main` on macOS/Windows |

Roles describe relationships, not platform widgets:

- `main` is a top-level destination. At most one `main` may set `launch: true`.
- `aside` assists the current main. `edge` and `size` are placement hints.
- `float` is a tray-anchored popover and therefore requires `tray:`.

## A valid declaration

```yaml
capabilities:
  browser: true
  terminal: true

surfaces:
  - lxapp: my-home
    role: main
    launch: true
    tray:
      icon: icons/tray.svg
      label: My App
      action: activate

  - lxapp: assistant
    role: aside
    edge: right
    size: { width: 320 }

  - native: terminal
    role: aside
    edge: bottom
    platforms: [macos, windows]
```

Each lxapp must also be listed in `resources.bundles`, unless the runtime or update provider supplies it. `lingxia build` validates this source and generates `ui.json`; never edit `ui.json` directly.

There is no `sidebar:` field. App-owned sidebar entries are runtime actions declared by the [Control app](../control-app/) through `lx.shell.sidebarActions` (`replace`, `update`, `remove`, `clear`). Each callback explicitly opens a surface or performs another action. User-owned Pins are intentionally not writable by app code.

## Size classes

An lxapp receives its own surface viewport class through `lx.surface.onContext`:

| Size class | Viewport width |
|---|---:|
| `compact` | less than 600 logical pixels |
| `medium` | 600 through 840 |
| `expanded` | greater than 840 |

This is the lxapp surface size, not a device-family check and not necessarily the host window size. An aside inside a wide desktop shell may still be `compact`. Use CSS/container queries for layout-only changes and surface context when the component tree or interaction model changes.

At the shell level the same declaration drives several realizations:

- **Wide desktop** — full sidebar and several docked asides beside the main.
- **Medium desktop** — the sidebar collapses to an icon rail; at most one aside stays docked.
- **Narrow desktop** — the icon rail remains and `main` keeps a desktop workspace; asides overlay the main when they cannot dock. Browser chrome stays at the top.
- **Mobile / phone Runner** — the sidebar disappears, `main` goes full screen, and asides overlay it.

## Open surfaces at runtime

`lx.surface` selects behavior by method:

```ts
lx.surface.openDeclared('assistant')
lx.surface.openUrl('https://example.com')
lx.surface.openUrl('https://example.com', { as: 'aside' })
lx.surface.openPage('inspector', { as: 'float' })
lx.surface.openPage('editor', { as: 'window', chrome: 'full' })

const unsubscribe = lx.surface.onContext((context) => {
  this.setData({ surfaceContext: context })
})
```

- `openDeclared(id)` opens content declared in `lingxia.yaml`; `id` is the declaration's content identity.
- `openUrl(url)` opens a normal in-app browser tab; `{ as: 'aside' }` docks the browser aside.
- `openPage(page)` opens one of **this** lxapp's pages as a chrome-less `float` or a desktop `window`. A page cannot become an `aside` — declare an lxapp surface for your own side panel.
- Ask `lx.supports({ capability: 'surface', value: 'window', chrome: 'full' })` before offering an edge-to-edge window. Pad custom chrome with `var(--lx-page-chrome-top-inset)`.
- `hide()` preserves state; `close()` destroys the surface. Page-overlay form is chosen when opened, while declared surfaces continue to adapt with the shell.
- `lx.surface.get(key)` returns a handle only for surfaces this lxapp opened **with a `key`**.

`lx.openSurface` and `lx.onSurfaceContext` no longer exist.

## Build-time rules worth remembering

- macOS and Windows admit exactly one declared `main`, whose content may be `lxapp`, `url`, `native: terminal`, or `native: browser`. Other targets still require the home lxapp as their initial main.
- A pure desktop popover app may declare one `role: float` surface with a `tray:` and no main.
- `launch` is valid only on a `main`; at most one main launches.
- `edge` and `size` are valid only on `aside`.
- `url` requires `capabilities.browser: true`. Declarative URL main is desktop-only.
- `native: terminal` requires `capabilities.terminal: true`; an aside uses `top` or `bottom` and is desktop-only.
- `native: browser` requires `capabilities.browser: true` and supports a macOS or Windows main.
- A `float` requires `tray:`, and at most one surface may declare a tray on each target.
- Tray icons are host-root-relative square SVG source files.

For the complete schema and tray behavior, install the LingXia skill and read `app/project.md`. For responsive lxapp implementation, see [LxApp pages](../lxapp-pages/).

---
title: Adaptive surfaces
description: Declare host surfaces once and let LingXia realize them for the available screen size.
sidebar:
  order: 7
---

A native host describes its UI as a flat `surfaces:` list in `lingxia.yaml`. You declare the content and its relationship to the main experience; the host realizes it as a window, tab, docked panel, full-screen overlay, or tray popover according to the available size.

## Content keys and roles

Every entry has exactly one content key. Its value is also the surface identity; there is no separate `id` or `render` field.

| Content key | What it opens | Supported roles |
|---|---|---|
| `lxapp` | An lxapp by `appId` | `main`, `aside`, `float` |
| `url` | An in-app browser page; requires `capabilities.browser` | `main`, `aside` |
| `native` | A host-native surface; currently only `terminal` | `aside` |

Roles describe relationships, not platform widgets:

- `main` is a top-level destination. At most one lxapp main may set `launch: true`.
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

There is no `sidebar:` field. App-owned sidebar entries are runtime **activators** declared by the home lxapp through `lx.shell.activators`; each callback explicitly opens a surface or performs another action. User-owned Pins are intentionally not writable by app code.

## Size classes

An lxapp receives its own surface viewport class through `lx.onSurfaceContext`:

| Size class | Viewport width |
|---|---:|
| `compact` | less than 600 logical pixels |
| `medium` | 600 through 840 |
| `expanded` | greater than 840 |

This is the lxapp surface size, not a device-family check and not necessarily the host window size. An aside inside a wide desktop shell may still be `compact`. Use CSS/container queries for layout-only changes and surface context when the component tree or interaction model changes.

At the shell level, wide desktop layouts can keep the sidebar and several asides visible; medium layouts collapse the sidebar and keep at most one aside docked; compact layouts make main content full screen and present asides over it. The same declaration drives all three.

## Open surfaces at runtime

`lx.openSurface` selects behavior by source:

```ts
lx.openSurface({ surface: 'assistant' })
lx.openSurface({ url: 'https://example.com' })
lx.openSurface({ url: 'https://example.com', as: 'aside' })
lx.openSurface({ page: 'inspector', as: 'float' })
```

- `{ surface }` opens content declared in `lingxia.yaml`; its value is the declaration's content identity.
- `{ url }` opens a normal in-app browser tab; `{ url, as: 'aside' }` opens the browser aside.
- Your own lxapp page can open as a chrome-less `float` or a desktop `window`; it cannot become an `aside`. Use a declared lxapp surface for your own side panel.
- `hide()` preserves state; `close()` destroys the surface. Page-overlay form is chosen when opened, while declared surfaces continue to adapt with the shell.

## Build-time rules worth remembering

- A product host needs an lxapp `main`, unless it is a pure `role: float` tray-popover app.
- `launch` is valid only on an lxapp `main`; at most one main launches.
- `edge` and `size` are valid only on `aside`.
- `url` requires `capabilities.browser: true`.
- `native: terminal` requires `capabilities.terminal: true`, uses `top` or `bottom`, and is desktop-only.
- A `float` requires `tray:`, and at most one surface may declare a tray.
- Tray icons are host-root-relative square SVG source files.

For the complete schema and tray behavior, install the LingXia skill and read `app/project.md`. For responsive lxapp implementation, see [LxApp pages](../lxapp-pages/).

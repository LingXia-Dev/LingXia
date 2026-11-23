# LingXia View Environment and Host Chrome Layout Specification

> Platforms: Android / iOS / HarmonyOS / macOS / Windows / Runner
>
> Scope: the ownership, layout, geometry, transport, and View-facing contract
> for host chrome that shares or overlaps an lxapp page.

This document uses MUST / MUST NOT / SHOULD / MAY in their normative sense.
Signatures are not repeated here: `@lingxia/types` is the Logic surface,
`@lingxia/page-runtime` and `@lingxia/react` the WebView View surface, and
`@lingxia/view` the GPU View surface.

Related specifications:

- [`shell-ui-spec.md`](./shell-ui-spec.md) remains authoritative for shell
  roles and desktop/mobile projection semantics; this document owns the page
  viewport and host-chrome geometry contract.
- [`bridge-protocol.md`](./bridge-protocol.md) owns the Logic–View wire. Page
  chrome geometry does not travel on it (§6.1).

---

## 0. Summary

Logic has no geometry query for host chrome: there is no
`lx.getCapsuleRect()` and no `lx.getViewportMetrics()`.

The native host measures host chrome and pushes the result to each page:

```text
Logic: lx.app.tabBar.update(...), navigation bar patches
              |
              | semantic intent
              v
Rust: page chrome state + page chrome revision
              |
              | apply
              v
Native host: lay out the page and host chrome, measure the capsule
              |
              | measured page geometry
              v
Rust: page chrome layout for this page instance
              |
       +------+---------------------+
       v                            v
 WebView page                  GPU View page
 window.lxPageChrome           view.env
 --lx-page-chrome-* CSS
 lxpagechromechange
```

The public tab bar layout vocabulary is `presentation`:

- `"standard"`: the host ends the page above the tab bar, which occupies
  adjacent space;
- `"immersive"`: the page extends beneath the tab bar and the host renders the
  tab bar above it.

Colours control appearance only and MUST NOT select the layout.

---

## 1. Goals and non-goals

### 1.1 Goals

The design:

1. puts pixel geometry in the View/host boundary rather than the Logic API;
2. gives React and HTML Views the same synchronous layout snapshot;
3. supports further host chrome without adding one `getXxxRect()` API per
   control;
4. makes tab bar colours independent from page layout;
5. uses post-layout native frames rather than configured sizes;
6. updates atomically after navigation, rotation, safe-area changes, runtime
   tab bar changes, and page attachment;
7. provides both edge insets for ordinary layout and an exact capsule
   rectangle for local collision avoidance;
8. keeps page popups usable when native chrome is rendered above the page; and
9. keeps a single cross-platform semantic model while allowing platform
   rendering differences.

### 1.2 Non-goals

This specification does not define:

- the visual design of a tab bar, capsule, popup, or modal;
- a general-purpose browser layout engine;
- page business state replication;
- keyboard and IME avoidance;
- acceptance of the flat `tabBar` fields listed in §4.1; or
- aliases for `lx.*` methods that do not exist.

---

## 2. Terminology

| Term | Definition |
|---|---|
| **Logic** | The native JS runtime that owns page definitions, business state, navigation intent, and `lx.*` calls. |
| **View** | The React or HTML document rendered by a page WebView, or a GPU View page. |
| **host chrome** | Native host UI sharing the page surface, such as a capsule, tab bar, or navigation bar. |
| **layout viewport** | The page coordinate space used by CSS layout and `getBoundingClientRect()`. |
| **standard presentation** | A layout where visible tab bar space is removed from the page bounds. |
| **immersive presentation** | A layout where the page continues beneath the tab bar and the tab bar is rendered above it. |
| **occlusion** | A host-owned rectangle intersecting the page layout viewport and rendered above the View. |
| **system inset** | An edge reservation caused by platform UI such as a status area, cutout, system gesture area, or home indicator. |
| **page chrome layout** | The per-page snapshot of host-chrome geometry the host publishes to a View (§5.1). |
| **page chrome revision** | A monotonic per-lxapp revision for semantic host-chrome state. Each page chrome layout carries the revision it reflects. |

---

## 3. Ownership and invariants

### 3.1 Layer ownership

The layers have distinct responsibilities:

| Layer | Owns | MUST NOT own |
|---|---|---|
| Logic | tab bar intent, item state, requested visibility, navigation bar patches, navigation semantics | native frames, density conversion, page-relative rects |
| Rust lxapp core | validated config, runtime chrome state, revisions, mutation serialization, each page's chrome layout | final platform geometry |
| Native host | page bounds, chrome views, z-order, safe areas, frame measurement | page business layout decisions |
| Page runtime | the page's layout snapshot, CSS variables, change event | querying Logic for host geometry |

### 3.2 Normative invariants

1. Logic MUST NOT expose a host-control rectangle API.
2. A native platform MUST NOT derive page layout from tab bar background
   alpha. The configured `presentation` is authoritative.
3. Geometry MUST be measured after native layout and expressed relative to the
   target page, not the screen or application window.
4. Public geometry MUST use CSS (logical) pixels and the top-left origin of the
   layout viewport.
5. A page chrome layout is scoped to one page instance. It MUST NOT be cached
   only by appId.
6. All numeric values MUST be finite. Negative sizes and empty intersections
   MUST be clamped or omitted before publication.
7. A visible `standard` tab bar is outside the page viewport and therefore
   contributes no inset.
8. A visible `immersive` tab bar intersecting the page sets the bottom inset.
9. A capsule is a local occlusion. It MUST NOT force a full-width top inset
   merely because it touches the top edge.
10. Insets MUST be computed from the current measured layout, not from a
    configured size.
11. Hiding host chrome MUST remove its inset and occlusion in the same
    committed update.
12. A Logic promise for a geometry-affecting chrome mutation MUST NOT resolve
    before the host has applied the matching page chrome revision.

---

## 4. Tab bar model

### 4.1 Manifest

`tabBar` has three fields: `presentation` (`"standard"` by default, or
`"immersive"`), `style`, and 2 to 10 `items`. The shapes are in
`@lingxia/types`.

- Style colours MUST be opaque `#RRGGBB`.
- An `immersive` tab bar MUST omit `style.backgroundColor` and
  `style.dividerColor`; the host draws it see-through.
- The manifest has no edge or size. The host owns tab bar placement and size.
- The host grows a bottom bar by the system's bottom area below it (the iOS
  home indicator, the Android navigation bar or gesture zone, the HarmonyOS
  gesture zone), and a page's bottom inset is the whole bar.

Project templates SHOULD use `"standard"` unless the template is explicitly an
immersive design.

The parser MUST reject these root fields, naming the replacement in the
error:

- `color`;
- `selectedColor`;
- `backgroundColor`;
- `borderStyle`;
- `position`;
- `dimension`; and
- `list`.

It also rejects `items[].pagePath`; an item names its page with `page`. No
alias parser is provided.

### 4.2 Why `standard` and `immersive`

The two presentations describe what happens to app content:

```text
standard

+--------------------+
|                    |
|        Page        |
|                    |
+--------------------+
|       TabBar       |
+--------------------+
```

```text
immersive

+--------------------+
|                    |
|        Page        |
|                    |
|  +--------------+  |
|  |    TabBar    |  |  native layer above the page
+--+--------------+--+
```

The presentation alone selects the layout. A `standard` bar paints an opaque
background; an `immersive` bar has no background colour of its own.

No `auto` presentation is defined. An automatic mode would re-couple layout to
style, make runtime colour changes move the viewport, and recreate platform
alpha-rule differences.

### 4.3 Configuration and runtime state are separate

The Rust model keeps the declared configuration apart from runtime state. The
runtime state holds the visibility preference (`auto`, `visible`, or
`hidden`), the selected index, and per-item overrides (text, icon, badge, red
dot).

The visibility preference records the app request. Effective visibility is the
result after navigation rules are applied. For example, navigating from a tab
page to a detail page may make the bar effectively hidden without overwriting
the app's preference.

### 4.4 Logic mutation API

`lx.app.tabBar.update(patch)` is the one transactional mutation. A patch sets
`visibility` and per-item `label`, `icon`, and `badge`, with items keyed by
tab name. A `badge` of `true` shows a dot; `null` clears the badge and the dot.
Style is declared in the manifest only; the update refuses a `style` field.

The update MUST be validated and committed atomically. One call produces at
most one page chrome revision and one platform layout transaction.
`visibility: "auto"` lets navigation derive effective visibility;
`visibility: "visible"` explicitly reveals the bar on any route, and
`visibility: "hidden"` explicitly suppresses it.

Navigation is a separate concern. `lx.nav.go({ tab })` selects and navigates
to a tab item; `tabBar.update` does not navigate.

All errors are reported as rejected promises. There are no boolean success
values.

---

## 5. Page chrome layout

### 5.1 Data model

A page chrome layout has these fields, all always present:

| Field | Meaning |
|---|---|
| `revision` | The page chrome revision this layout reflects. |
| `topInset` | Height of the runtime-owned drag strip across the top of a `chrome: 'full'` window. Zero everywhere else. |
| `bottomInset` | The band a visible `immersive` tab bar covers, including the system area below it. Zero otherwise. |
| `capsuleRect` | The capsule's rectangle where it floats over the page, or `null`. |
| `capsuleInlineEndInset` | The capsule's width plus its trailing gap, for controls in the capsule's top band. Zero without a capsule. |

The empty state uses zero insets and a `null` capsule; it does not use
optional numeric fields or `{}` as a sentinel. A GPU View page reads the same
capsule rectangle as `view.env.capsule`.

### 5.2 Coordinate space

Published rectangles MUST match View layout APIs:

- origin is the top-left of the page layout viewport;
- units are CSS pixels;
- `right == left + width`;
- `bottom == top + height`;
- the rect is clipped to the page viewport; and
- a fully non-intersecting native view is omitted.

Native screen points, Android physical pixels, HarmonyOS px/vp, and device
scale values MUST be converted before the snapshot reaches application code.
Platforms SHOULD reuse the same native-to-CSS transform used by native
components so both systems agree with `getBoundingClientRect()`.

### 5.3 Inset derivation

The insets describe only avoidance still present inside the page. If the host
has already positioned the page below or beside a system area, that edge
reserves nothing; the View MUST NOT receive a second reservation.

The bottom inset of an `immersive` bar runs from the bar's top to the page
viewport bottom. A gap between the bar and the viewport edge, including a
system gesture region, is part of it.

A capsule is a local obstruction rather than a continuous edge band. It is
published as `capsuleRect` and `capsuleInlineEndInset` and contributes nothing
to `topInset`.

### 5.4 Presentation examples

For a visible bottom tab bar:

| Presentation | Page bounds | Bottom inset |
|---|---|---|
| `standard` | ends at tab bar top | `0` |
| `immersive` | extends to host content bottom | viewport bottom to measured tab bar top |

For a hidden tab bar, both presentations produce no bottom inset. A
`standard` page expands in the same native layout commit that hides the bar.

---

## 6. Host-to-View transport

### 6.1 Delivery

Page chrome geometry MUST NOT travel through:

- page `data`;
- `page.update()`;
- a page action;
- `lx.*` Logic invocation; or
- Logic-to-View RPC intended for application UI.

The host delivers it by script into the page document. A document-start script
defines `window.lxPageChrome` and the page-runtime apply function; each later
layout is applied by calling that function with the layout and the resolved
colour scheme.

The layout is ordered independently from page business state, but each one
carries its page chrome revision so a Logic mutation can be correlated with
the host layout that resulted from it.

### 6.2 Initial snapshot

The document-start script installs a layout before any page script runs, so
the page framework mounts with one defined. It is the empty layout until the
host has measured the page; the measured layout follows the first chrome
commit for that page.

The page runtime MUST expose the layout synchronously. A View MUST NOT need to
await a native round trip before its first layout.

### 6.3 Subsequent updates

A new layout MUST be published when any relevant value changes, including:

- page attachment or replacement;
- host window/container resize;
- device rotation;
- system safe-area or gesture-inset changes;
- tab bar visibility or presentation;
- capsule creation, removal, visibility, or frame;
- navigation-bar layout changes; and
- entry into or exit from native fullscreen.

Updates SHOULD be coalesced per native frame. Duplicate layouts with
identical geometry and revision SHOULD NOT be published.

### 6.4 Commit and acknowledgement

There is one asynchronous chrome commit path:

```text
Rust mutates page chrome state -> revision N
    -> platform applies revision N and completes native layout
    -> platform acknowledges N
    -> Rust measures the capsule and stores the page's layout for N
    -> Rust publishes the layout to the page
    -> lx.app.tabBar.update(...) resolves
```

A layout older than the page's current one is ignored. A non-geometric item
change uses the same commit path; the host may skip a layout pass when it can
prove geometry is unchanged, but acknowledgement ordering remains the same.

If no presenter is mounted, the core keeps the new semantic state and resolves
after that state commit, because no physical layout exists to await. The first
presenter that mounts MUST be initialized from the latest revision.

---

## 7. View runtime and public APIs

### 7.1 Package ownership

| Package | Responsibility |
|---|---|
| `@lingxia/page-runtime` | `window.lxPageChrome`, the apply function, CSS-variable projection, the `lxpagechromechange` event |
| `@lingxia/react` | `useLxViewEnvironment()`, whose `insets` and `safeArea` come from the page chrome layout |
| `@lingxia/view` | `view.env` for GPU View pages (§8.4) |

The page chrome types MUST NOT be generated into `@lingxia/types`, which is
the Logic-side declaration surface.

### 7.2 Synchronous View APIs

React:

```tsx
import { useLxViewEnvironment } from '@lingxia/react'

function PageHeader() {
  const environment = useLxViewEnvironment()
  // ...
}
```

HTML:

```ts
const initial = window.lxPageChrome.layout
window.addEventListener('lxpagechromechange', (event) => {
  // event.detail is the next layout; update geometry-dependent DOM
})
```

`window.lxPageChrome.layout` is synchronous because it reads the pushed
snapshot. No public Promise getter is defined.

### 7.3 CSS variables

Before dispatching `lxpagechromechange`, the page runtime MUST project the
layout to `document.documentElement`:

```css
:root {
  --lx-page-chrome-top-inset: 0px;
  --lx-page-chrome-bottom-inset: 0px;
  --lx-page-chrome-capsule-inline-end-inset: 0px;

  --lx-page-chrome-capsule-top: 0px;
  --lx-page-chrome-capsule-right: 0px;
  --lx-page-chrome-capsule-bottom: 0px;
  --lx-page-chrome-capsule-left: 0px;
  --lx-page-chrome-capsule-width: 0px;
  --lx-page-chrome-capsule-height: 0px;
}
```

These variables are always defined; the capsule variables are `0px` without a
capsule. Pages SHOULD use them for ordinary controls:

```css
.bottom-sheet {
  position: fixed;
  left: 0;
  right: 0;
  bottom: var(--lx-page-chrome-bottom-inset);
}
```

Pages MUST NOT add `env(safe-area-inset-bottom)` to
`--lx-page-chrome-bottom-inset`. The inset already includes the system area
below the bar. Adding both double-counts the same physical area.

### 7.4 Exact capsule use

Insets solve continuous edge avoidance. The capsule rectangle solves local
collision problems such as a custom header beside the capsule:

```ts
const capsule = window.lxPageChrome.layout.capsuleRect

if (capsule && intersects(titleRect, capsule)) {
  // shorten, shift, or choose an alternate header layout
}
```

Framework packages MAY provide shared collision helpers, but the contract does
not mandate one positioning algorithm.

---

## 8. Popup and modal policy

### 8.1 Non-modal page popups

Dropdowns, popovers, menus, and bottom sheets rendered inside the page WebView
remain below native host chrome in z-order. They MUST position interactive
content inside the insets and SHOULD use the capsule rectangle for anchor
collision and flip/shift decisions.

An `immersive` layout intentionally permits non-interactive backgrounds,
images, and videos to continue beneath the tab bar. Avoidance applies to
interactive foreground content, not necessarily to the page background.

### 8.2 Standard system modal UI

Standard mobile APIs such as `lx.ui.confirm()` and `lx.ui.actionSheet()` SHOULD
use native platform presentation. Native presentation is above tab bar and
capsule chrome and therefore does not depend on WebView z-index.

Desktop WebView implementations of the same APIs MUST consume the page chrome
layout when positioning their DOM presentation.

### 8.3 Custom View modal UI

Calling tab bar show/hide for a transient custom modal is forbidden as the
standard solution. Tab bar visibility is semantic application state; using it
as a modal implementation causes restoration races, navigation conflicts, and
incorrect behavior for nested modals.

A custom dialog or sheet in a WebView page remains in the WebView and MUST fit
within the content area. A design that requires arbitrary HTML to render
physically above native chrome must use a native presentation or a separate
host-managed surface; ordinary WebView z-index cannot provide that guarantee.

### 8.4 GPU View pages and the host bars

A GPU page spans the window under the navigation bar and tab bar, which reach
it as `env.insets`. One rule on every page and platform:

- Below the bars: page content. A `scroll` or `list` that is the page's last
  in-flow child runs to the bottom edge; the bottom inset becomes its end
  padding, which also bounds reveals, so its last row scrolls clear. A keyboard
  taller than the inset still ends it. Any other last child keeps the inset as
  page padding. Whether content shows through is the bar's own paint
  (`immersive` is see-through).
- Above the bars: the top layer. While a `<dialog>` (sheet, picker, modal,
  action sheet, with its scrim) is open or exiting, the host raises the whole
  page above both bars and hides them from assistive tech and chrome
  automation. Popovers stay below. Tab bar visibility is never changed.
- Adaptive canvas contract (`view.env`):
  - `viewport`: the page's own region, not the window. Beside a docked side
    panel it is the main pane. The View receives every change.
  - `sizeClass`: `'compact' | 'regular'` (`< 600 dp` vs `≥ 600 dp`, `±24 dp`
    hysteresis), computed from the page's own viewport, so a narrow aside in a
    `regular` window reads `compact`. Crossing the 820 dp sidebar budget never
    changes it. `<ListDetail>` and `<Split>` scope it, and `regular:` classes,
    for a subtree through `<SizeClassScope>`; nothing else does.
  - `canDock`: true while the window's size class is `regular`; false
    otherwise and before the host has reported a layout. It says nothing about
    where the page is shown; the page is not told its role or edge.
  - `insets` and `safeArea`: both the physical edges `{ top, right, bottom, left }`
    and the logical `inlineStart`/`inlineEnd`, which follow the text direction.
  - `hinge`: `{ x, y, width, height, orientation: 'vertical' | 'horizontal', posture: 'flat' | 'half-opened' } | null`
    in page-local logical coordinates, clipped to each page; `null` where the
    hinge does not cross the page. A `half-opened` posture reaches the page
    as is: the host does not crop the page to one half.
  - `platform`: `'apple' | 'android' | 'harmony' | 'windows' | 'web'` (shortcut
    glyphs only; never for layout branching).
- Logic's `page.env` carries only `sizeClass` and `canDock`.
  `page.env.watch` fires only when one of them flips, never on a resize.
- Declarative `<page title>` owns the page navigation bar title; direct
  `<page>` children may pass `bleed` (`boolean | 'top' | 'bottom' | 'horizontal'`)
  to extend to the physical screen edge beneath bars and cutouts.
- `useBackHandler(enabled, onBack)`: while enabled, Back calls `onBack` and
  the page stays. The most recently mounted enabled handler takes the press.
  Home is never intercepted. On the home app's first page only a back handler
  takes Back; a leave guard there does not, and Back backgrounds the app.
- A page with no `pageOrientation` does not lock the orientation; the host's
  declared orientation applies.

---

## 9. Platform rendering contract

Every platform follows the same sequence:

1. receive a semantic page chrome revision;
2. resolve effective visibility;
3. apply the `standard` or `immersive` constraints without inspecting colours;
4. complete layout and acknowledge the revision;
5. measure the visible capsule frame in the target page's CSS viewport;
6. derive the insets; and
7. publish the layout.

### 9.1 Android

Android MUST use the actual tab bar and capsule `View` frames after layout. It
must account for the page container origin, display density, system bar
insets, gesture navigation, and any transparent navigation-bar placement.

`standard` is implemented with page container margins or constraints.
`immersive` keeps the page container at the full content bounds and places the
tab bar later in native z-order.

### 9.2 iOS

iOS MUST maintain explicit mutable constraints for all four page edges.
`standard` changes the bottom constraint to the tab bar edge. `immersive` pins
the page to the host content edge and brings the tab bar above it.

Frames MUST be converted using UIKit view conversion after
`layoutIfNeeded()`. Safe-area values already excluded by page constraints
must not be emitted again.

### 9.3 HarmonyOS

HarmonyOS MUST make `presentation` authoritative in its ArkUI composition.
`standard` reserves sibling space. `immersive` uses a Stack with the tab bar
above the Web component.

The measured component areas, bottom navigation-indicator inset, px/vp
conversion, and Web component origin must all be included when generating CSS
viewport geometry.

### 9.4 macOS, Windows, and Runner

Desktop projections use the same contract even when mobile tab bar items are
rendered as sidebar children rather than a bottom bar. Chrome outside a page
is not an occlusion. Any Runner device frame that projects a mobile tab bar
over the simulated page MUST publish the same geometry the real mobile host
would expose.

This common contract keeps View code independent from the host projection.

### 9.5 Touch-host chrome in `regular`

On a touch host in `regular`, four pieces of host chrome change a page's
insets or region:

- **Status band**, while a side column is visible: the page starts below
  it, and an immersive page does not bleed under it.
- **Floating tab bar**: the page keeps clear of the strip the bar stands in;
  an immersive page runs under it.
- **Tab column**: opaque; it narrows the page rather than covering it.
- **Docked side panel**: it narrows the main page.

---

## 10. No Logic capsule query

No Logic API exposes capsule geometry. There is no `lx.getCapsuleRect()`, no
generated `CapsuleRect` declaration, and no `AppRuntime::get_capsule_rect`.
`CapsuleRect` does not exist in any other Logic namespace.

The native capsule control and its layout remain. Each host measures the
laid-out capsule frame through the platform UI trait
(`measure_page_chrome_capsule`), and the result reaches pages only through the
page chrome layout.

---

## 11. Rejected alternatives

### 11.1 Infer layout from `backgroundColor`

Rejected because style changes would mutate layout, partially transparent
colours have ambiguous semantics, and platforms tend to implement different
alpha thresholds.

### 11.2 Always end the page above the tab bar

Rejected because it prevents immersive backgrounds and media from extending
beneath a see-through or floating tab bar.

### 11.3 Always extend and make every page add padding

Rejected because ordinary opaque tab bars should have native adjacent layout,
first-paint correctness would depend on page CSS, and every application would
need to reproduce framework behavior.

### 11.4 Add `lx.getViewportMetrics()`

Rejected because it leaves View geometry in Logic, introduces an asynchronous
query race, duplicates state between View and Logic, and still requires a
separate change-notification mechanism.

### 11.5 Keep one getter per host control

Rejected because capsule, tab bar, navigation, floating controls, and further
chrome would each require their own types, FFI, callbacks, and platform code.

### 11.6 Add a third padded presentation

Rejected because it is not a distinct host geometry. It is `immersive` plus
the View consuming the bottom inset. Keeping that composition explicit avoids a
third presentation whose native layout is indistinguishable from `immersive`.

### 11.7 Hide/show the tab bar around every modal

Rejected because semantic visibility is not transient presentation state. It
breaks nested modal restoration and races with navigation and app-requested
visibility.

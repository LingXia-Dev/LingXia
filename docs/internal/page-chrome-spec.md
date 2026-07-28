# LingXia Page Chrome, Appearance APIs, and View Environment Specification

> Status: proposal (breaking redesign) · Date: 2026-07-28
>
> Platforms: Android / iOS / HarmonyOS / macOS / Windows / Runner
>
> Scope: the configuration, ShellTheme inheritance, runtime APIs, layout,
> geometry, transport, and View-facing contract for tabbars, navigation bars,
> and other host chrome sharing an lxapp WebView. This specification
> intentionally provides no compatibility path for the APIs and schemas it
> replaces.

This document uses MUST / MUST NOT / SHOULD / MAY in their normative sense.
Unless a section is explicitly marked as current-state analysis, it describes
the target architecture rather than the implementation that happens to exist
today.

Related specifications:

- [`shell-theme-spec.md`](./shell-theme-spec.md) owns host appearance and the
  semantic color palette consumed here. It MUST land before this design is
  implemented.
- [`shell-ui-spec.md`](./shell-ui-spec.md) remains authoritative for shell
  roles and desktop/mobile projection semantics; this document owns the page
  viewport and host-chrome geometry contract.
- [`bridge-protocol.md`](./bridge-protocol.md) remains authoritative for the
  final wire encoding; the control frame shown here defines required semantics,
  not an independently versioned transport.

---

## 0. Decision summary

Tabbar and navigation bar consume the host-owned ShellTheme. They MUST NOT
declare `light` / `dark` style blocks of their own.

- An omitted component color resolves through its assigned ShellTheme role and
  follows the effective host appearance automatically.
- An explicit manifest color is a fixed component override in both schemes.
- A runtime color is a fixed session override; `null` clears that runtime field
  and reveals the manifest override or semantic fallback beneath it.
- Native chrome refresh is host-driven. Lxapp JavaScript is never required to
  synchronize a color-scheme change.
- View code uses standard CSS `prefers-color-scheme` / `matchMedia`.
- Logic may observe appearance with `lx.getAppearance()` and the matching
  change listener. Only the home lxapp may change the persisted host preference
  through `lx.app.setAppearance(...)`.

LingXia MUST remove `lx.getCapsuleRect()` and MUST NOT replace it with another
Logic-side geometry query such as `lx.getViewportMetrics()`.

The replacement is a reactive **View Environment** owned by the native host and
delivered directly to each WebView:

```text
Logic: lx.tabBar.update(...)
              |
              | semantic intent
              v
Rust: ChromeState + chrome revision
              |
              | apply
              v
Native host: lay out WebView and host chrome
              |
              | measured CSS-viewport geometry
              v
Bridge control plane: environment.snapshot
              |
              v
@lingxia/page-runtime
       |              |                 |
       v              v                 v
 CSS variables   React/Vue hooks   HTML subscription
```

The public tabbar layout vocabulary is:

- `contentMode: "resize"`: the host shortens the WebView so the tabbar occupies
  adjacent space;
- `contentMode: "extend"`: the WebView extends beneath the tabbar and the host
  renders the tabbar above it.

`overlay` is an implementation term for the native z-order used by `extend`.
It is deliberately not the public configuration value because it can be
confused with transparency. `backgroundColor` controls appearance only and
MUST NOT select the layout mode.

---

## 1. Goals and non-goals

### 1.1 Goals

This redesign MUST:

1. make tabbar and navigation-bar defaults inherit one host-owned semantic
   palette without per-lxapp light/dark configuration;
2. give manifest and runtime overrides one predictable precedence model;
3. put pixel geometry in the View/host boundary rather than the Logic API;
4. give React, Vue, and HTML Views the same synchronous environment snapshot;
5. support bottom, left, right, floating, and future host chrome without adding
   one `getXxxRect()` API per control;
6. make transparent styling independent from WebView layout;
7. use post-layout native frames rather than configured sizes to calculate
   occlusion;
8. update atomically after appearance, navigation, rotation, safe-area changes,
   runtime chrome changes, and WebView attachment;
9. provide both edge insets for ordinary layout and exact rectangles for local
   collision avoidance;
10. keep page popups usable when native chrome is rendered above the WebView;
11. preserve a single cross-platform semantic model while allowing platform
   rendering differences; and
12. eliminate the dedicated capsule geometry FFI and bridge chain.

### 1.2 Non-goals

This specification does not define:

- the visual design of a tabbar, capsule, popup, or modal;
- a general-purpose browser layout engine;
- page business state replication;
- an lxapp-owned theme or design-token system;
- ShellTheme token values or appearance persistence internals;
- keyboard/IME avoidance beyond allowing it to be added to the environment in
  a future revision;
- backward-compatible parsing of the old flat `tabBar` schema; or
- aliases for removed `lx.*` methods.

---

## 2. Terminology

| Term | Definition |
|---|---|
| **Logic** | The native JS runtime that owns `Page({})`, business state, navigation intent, and `lx.*` calls. |
| **View** | The React, Vue, or HTML document rendered by a page WebView. |
| **host chrome** | Native host UI sharing the page surface, such as a capsule, tabbar, navigation bar, or future floating host control. |
| **semantic fallback** | The ShellTheme role, then platform semantic color, used when a component field is not explicitly configured. |
| **manifest override** | A fixed component color declared in `lxapp.json` or page JSON. |
| **runtime override** | A fixed component value applied to one live app/page session by a Logic mutation. |
| **layout viewport** | The WebView coordinate space used by CSS layout and `getBoundingClientRect()`. |
| **resize mode** | A layout where visible tabbar space is removed from the WebView bounds. |
| **extend mode** | A layout where the WebView continues beneath the tabbar and the tabbar is rendered above it. |
| **occlusion** | A host-owned rectangle intersecting the WebView layout viewport and rendered above the View. |
| **system inset** | An edge reservation caused by platform UI such as a status area, cutout, system gesture area, or home indicator. |
| **chrome inset** | An edge reservation derived from host chrome that overlays a continuous band along an edge. |
| **content inset** | The effective per-edge avoidance value exposed to ordinary page layout. |
| **chrome revision** | A monotonic revision for semantic host-chrome state. |
| **environment revision** | A monotonic per-WebView revision for measured View Environment snapshots. |
| **modal shield** | A transient native layer that blocks interaction with host chrome while a custom View modal is active. |

---

## 3. Ownership and invariants

### 3.1 Layer ownership

The layers have distinct responsibilities:

| Layer | Owns | MUST NOT own |
|---|---|---|
| Logic | tabbar/navbar intent, item state, requested visibility, navigation semantics, appearance observation | native frames, ShellTheme ownership, density conversion, WebView-relative rects |
| Rust lxapp core | validated config, runtime chrome state, revisions, mutation serialization | final platform geometry |
| Native host | WebView bounds, chrome views, z-order, safe areas, frame measurement | page business layout decisions |
| Bridge | ordered environment delivery and subscriptions | deriving geometry from colors or app state |
| View runtime | snapshot store, CSS variables, framework bindings, popup collision inputs | querying Logic for host geometry |

### 3.2 Normative invariants

1. Logic MUST NOT expose a host-control rectangle API.
2. A native platform MUST NOT derive WebView layout from tabbar background
   alpha. The configured `contentMode` is authoritative.
3. Geometry MUST be measured after native layout and expressed relative to the
   target WebView, not the screen or application window.
4. Public geometry MUST use CSS pixels and the top-left origin of the layout
   viewport.
5. A View Environment snapshot is scoped to one concrete WebView instance. It
   MUST NOT be cached only by appId.
6. All numeric values MUST be finite. Negative sizes and empty intersections
   MUST be clamped or omitted before publication.
7. A visible `resize` tabbar is outside the WebView viewport and therefore is
   not an occlusion.
8. A visible `extend` tabbar intersecting the WebView is an occlusion.
9. A capsule is a local occlusion. It MUST NOT force a full-width top content
   inset merely because it touches the top edge.
10. `contentInsets` MUST be computed from the current measured layout, not from
    manifest `thickness` alone.
11. Hiding host chrome MUST remove its layout reservation and occlusion in the
    same committed update.
12. A Logic promise for a geometry-affecting chrome mutation MUST NOT resolve
    before the host has applied the matching chrome revision.
13. Tabbar and navigation-bar schemas MUST NOT contain appearance-specific
    style branches.
14. Omitted component style MUST resolve through ShellTheme; explicit component
    colors MUST stay fixed when appearance changes.
15. Child lxapps MUST NOT mutate the host appearance preference.

---

## 4. Breaking page-chrome model

### 4.1 Manifest schema

The old flat shape is replaced by separate layout, style, and items. Only
`items` is required; layout and style inherit useful defaults:

```json
{
  "tabBar": {
    "layout": {
      "edge": "bottom",
      "contentMode": "extend"
    },
    "style": {
      "backgroundColor": "transparent"
    },
    "items": [
      {
        "text": "Home",
        "pagePath": "pages/home/index",
        "iconPath": "public/home.png",
        "selectedIconPath": "public/home_selected.png"
      },
      {
        "text": "Settings",
        "pagePath": "pages/settings/index",
        "iconPath": "public/settings.png",
        "selectedIconPath": "public/settings_selected.png"
      }
    ]
  }
}
```

The target types are:

```ts
type TabBarEdge = 'bottom' | 'left' | 'right'
type TabBarContentMode = 'resize' | 'extend'

interface TabBarLayout {
  edge?: TabBarEdge
  contentMode?: TabBarContentMode
  /** Cross-axis size in logical/CSS pixels before platform safe-area addition. */
  thickness?: number
}

interface TabBarStyle {
  foregroundColor?: string
  selectedForegroundColor?: string
  backgroundColor?: string
  dividerColor?: string
}

interface TabBarConfig {
  layout?: TabBarLayout
  style?: TabBarStyle
  items: TabBarItemConfig[]
}
```

Defaults are `edge: "bottom"`, `contentMode: "resize"`, and a platform-standard
thickness. New-project templates SHOULD emit only `items` unless they are
explicitly demonstrating layout or visual customization. `items` keeps the
existing platform-supported minimum/maximum constraints.

The parser MUST reject the removed root fields:

- `color`;
- `selectedColor`;
- `backgroundColor`;
- `borderStyle`;
- `position`;
- `dimension`; and
- `list`.

No alias or migration parser is provided.

### 4.2 Style inheritance and overrides

Each optional style field has one ShellTheme role:

| Tabbar field | Semantic fallback |
|---|---|
| `foregroundColor` | `mutedForegroundColor` |
| `selectedForegroundColor` | `accentColor` |
| `backgroundColor` | `surfaceBackgroundColor` |
| `dividerColor` | `separatorColor` |

If a field is omitted, the host resolves that role for the effective appearance
and refreshes it automatically. If a string is supplied, that literal value is
a fixed override in both light and dark modes.

The following schema is forbidden:

```json
{
  "style": {
    "light": { "backgroundColor": "#ffffff" },
    "dark": { "backgroundColor": "#111111" }
  }
}
```

Appearance-specific product colors belong in the host's `shellTheme`, once.
A tabbar style exists only for a genuine component exception. Component color
strings use the cross-platform color grammar and may use `transparent` where
the field permits alpha.

### 4.3 Why `resize` and `extend`

The two modes describe what happens to app content:

```text
resize

+--------------------+
|                    |
|      WebView       |
|                    |
+--------------------+
|       TabBar       |
+--------------------+
```

```text
extend

+--------------------+
|                    |
|      WebView       |
|                    |
|  +--------------+  |
|  |    TabBar    |  |  native layer above the WebView
+--+--------------+--+
```

Transparency is independent:

| Style | Content mode | Meaning |
|---|---|---|
| opaque | `resize` | conventional adjacent tabbar |
| transparent | `extend` | conventional immersive tabbar |
| opaque | `extend` | content extends behind an opaque overlay by explicit choice |
| transparent | `resize` | transparent adjacent region showing host background by explicit choice |

No `auto` mode is defined. An automatic mode would re-couple layout to style,
make runtime color changes move the viewport, and recreate platform alpha-rule
differences.

### 4.4 Configuration and runtime state are separate

The Rust model MUST split immutable/declared configuration from runtime state:

```rust
pub struct TabBarConfig {
    pub layout: TabBarLayout,
    pub style: TabBarStyle,
    pub items: Vec<TabBarItemConfig>,
}

pub struct TabBarRuntimeState {
    pub visibility_intent: TabBarVisibilityIntent,
    pub effective_visible: bool,
    pub selected_index: usize,
    pub item_states: Vec<TabBarItemState>,
    pub revision: u64,
}
```

The core resolves optional manifest values into concrete layout defaults but
preserves whether each style field was declared. Native renderers need that
declaration mask to distinguish a fixed override from a semantic fallback.

`visibility_intent` records the app request. `effective_visible` is the result
after navigation rules are applied. For example, navigating from a tab page to
a detail page may make the effective value false without overwriting the app's
intent.

### 4.5 Tabbar Logic mutation API

The flat collection of `showTabBar`, `hideTabBar`, `setTabBarStyle`,
`setTabBarItem`, badge, and red-dot calls SHOULD be replaced by one namespaced,
transactional mutation:

```ts
interface TabBarStyleUpdate {
  foregroundColor?: string | null
  selectedForegroundColor?: string | null
  backgroundColor?: string | null
  dividerColor?: string | null
}

interface TabBarUpdate {
  visibility?: 'auto' | 'hidden'
  layout?: Partial<TabBarLayout>
  style?: TabBarStyleUpdate
  items?: Array<{
    index: number
    text?: string
    iconPath?: string
    selectedIconPath?: string
    badge?: string | null
    redDot?: boolean
  }>
}

interface LxTabBarAPI {
  update(patch: TabBarUpdate): Promise<void>
}

await lx.tabBar.update(patch)
```

The update MUST be validated and committed atomically. One call produces at
most one chrome revision and one platform layout transaction. `badge: null`
removes a badge. `visibility: "auto"` allows navigation to derive effective
visibility; `visibility: "hidden"` explicitly suppresses it.

For style fields, omission means "leave the runtime state unchanged" and
`null` means "clear this runtime override." Clearing reveals the manifest
override when present, otherwise the ShellTheme semantic fallback. A runtime
string is fixed across appearance changes for the rest of the app session or
until cleared.

Navigation remains a separate concern. `switchTab` continues to select and
navigate to a tab item; `tabBar.update` does not navigate.

All errors are reported as rejected promises. Boolean success values are not
part of the new contract.

### 4.6 Navigation-bar model and API

Page JSON replaces the old flat navigation fields with one optional object:

```json
{
  "navigationBar": {
    "title": "Account",
    "visibility": "visible"
  }
}
```

An exceptional fixed override is explicit:

```json
{
  "navigationBar": {
    "title": "Brand Preview",
    "style": {
      "backgroundColor": "#4C1D95",
      "foregroundColor": "#FFFFFF"
    }
  }
}
```

The target types are:

```ts
interface NavigationBarStyle {
  backgroundColor?: string
  foregroundColor?: string
  dividerColor?: string
}

interface NavigationBarConfig {
  title?: string
  visibility?: 'visible' | 'hidden'
  style?: NavigationBarStyle
}
```

The defaults are an empty title, visible native navigation bar, and semantic
style. Back/home button visibility is derived from navigation state and is not
page configuration.

Style roles are:

| Navigation-bar field | Semantic fallback |
|---|---|
| `backgroundColor` | `surfaceBackgroundColor` |
| `foregroundColor` | `foregroundColor` |
| `dividerColor` | `separatorColor` |

The parser rejects the removed `navigationBarTitleText`,
`navigationBarBackgroundColor`, `navigationBarTextStyle`, `navigationStyle`,
and `navigationBarStyle.light/dark` fields. There are no aliases.

Runtime mutation uses the same transaction and reset semantics as tabbar:

```ts
interface NavigationBarUpdate {
  title?: string
  visibility?: 'visible' | 'hidden'
  style?: {
    backgroundColor?: string | null
    foregroundColor?: string | null
    dividerColor?: string | null
  }
}

await lx.navigationBar.update(patch)
```

The override is scoped to the concrete page instance. It does not mutate the
page manifest or leak to another instance of the same route. A shown native
status area derives its glyph style from the resolved navigation foreground;
a hidden/custom navigation bar follows the effective host appearance unless
the platform has a more specific system rule.

### 4.7 Appearance Logic API

Native chrome follows appearance without JavaScript. The Logic API exists for
business behavior that genuinely depends on the host state:

```ts
type AppearancePreference = 'system' | 'light' | 'dark'
type EffectiveColorScheme = 'light' | 'dark'

interface AppearanceState {
  preference: AppearancePreference
  effective: EffectiveColorScheme
}

type AppearanceChangeCallback = (state: AppearanceState) => void

const state = lx.getAppearance()
lx.onAppearanceChange(callback)
lx.offAppearanceChange(callback)

// Home lxapp only; persists the host preference.
await lx.app.setAppearance({ preference: 'dark' })
```

`getAppearance()` is synchronous and reads the host state cached at Logic
startup. The host pushes changes; the Logic runtime does not poll. Listener
callbacks run only after native chrome and WebViews have entered the published
effective scheme.

Every lxapp may read and subscribe. Only the home lxapp may call
`lx.app.setAppearance`; other callers receive a permission error. The Promise
resolves after persistence and the native appearance transaction commit.

View code MUST use the browser-standard surface:

```ts
const dark = window.matchMedia('(prefers-color-scheme: dark)')
```

No separate View appearance hook is added, and ShellTheme token values are not
exposed to either Logic or View.

---

## 5. View Environment contract

### 5.1 Public data model

```ts
interface LxRect {
  x: number
  y: number
  width: number
  height: number
  top: number
  right: number
  bottom: number
  left: number
}

interface LxEdgeInsets {
  top: number
  right: number
  bottom: number
  left: number
}

type LxOcclusionKind =
  | 'capsule'
  | 'tabbar'
  | 'navigationBar'
  | 'hostControl'

interface LxViewOcclusion {
  /** Stable within the lifetime of this WebView. */
  id: string
  kind: LxOcclusionKind
  rect: LxRect
  /** True when the native layer consumes pointer input in this rectangle. */
  interactive: boolean
}

interface LxViewEnvironment {
  /** Monotonic for this WebView; changes for every published snapshot. */
  revision: number

  /** Latest semantic chrome revision included in this measured layout. */
  appliedChromeRevision: number

  /** The WebView layout viewport in its own CSS coordinate space. */
  viewport: LxRect

  /** Remaining platform-system avoidance inside this WebView. */
  systemInsets: LxEdgeInsets

  /** Continuous edge bands occupied by overlaying host chrome. */
  chromeInsets: LxEdgeInsets

  /** Per edge: max(systemInsets[edge], chromeInsets[edge]). */
  contentInsets: LxEdgeInsets

  /** Exact irregular or edge chrome intersections above the View. */
  occlusions: readonly LxViewOcclusion[]
}
```

All properties are required. The empty state uses zero insets and an empty
`occlusions` array; it does not use optional numeric fields or `{}` as a
sentinel.

### 5.2 Coordinate space

Published rectangles MUST match View layout APIs:

- origin is the top-left of the WebView layout viewport;
- `viewport.x == 0`, `viewport.y == 0`, `viewport.left == 0`, and
  `viewport.top == 0`;
- units are CSS pixels;
- `x == left` and `y == top`;
- `right == left + width`;
- `bottom == top + height`;
- the rect is clipped to the WebView viewport; and
- a fully non-intersecting native view is omitted.

Native screen points, Android physical pixels, HarmonyOS px/vp, and device
scale values MUST be converted before the snapshot reaches application code.
Platforms SHOULD reuse the same native-to-CSS transform used by native
components so both systems agree with `getBoundingClientRect()`.

### 5.3 Inset derivation

`systemInsets` describe only system-owned avoidance still present inside the
WebView. If the host has already positioned the WebView below or beside a
system area, that edge value is zero; the View MUST NOT receive a second
reservation.

`chromeInsets` describe continuous host-chrome edge bands:

- bottom overlay tabbar: `viewport.bottom - tabbarRect.top`;
- left overlay tabbar: `tabbarRect.right - viewport.left`;
- right overlay tabbar: `viewport.right - tabbarRect.left`.

The calculation uses the actual clipped frame. Consequently a gap between a
bottom tabbar and the viewport edge, including a system gesture region, is
part of the bottom chrome inset.

`contentInsets` use `max`, not addition:

```ts
contentInsets.bottom = Math.max(
  systemInsets.bottom,
  chromeInsets.bottom,
)
```

A capsule is a local obstruction rather than a continuous edge band. It is
published in `occlusions` and normally contributes zero to `chromeInsets`.

### 5.4 Mode examples

For a visible bottom tabbar:

| Mode | WebView bounds | Tabbar occlusion | Bottom chrome inset |
|---|---|---|---|
| `resize` | ends at tabbar top | absent | `0` |
| `extend` | extends to host content bottom | present | viewport bottom to measured tabbar top |

For a hidden tabbar, both modes produce no tabbar occlusion and no tabbar
chrome inset. A `resize` WebView expands in the same native layout commit that
hides the bar.

---

## 6. Host-to-View transport

### 6.1 Separate control plane

View Environment data MUST NOT travel through:

- `Page({ data })`;
- `this.setData()`;
- a page action;
- `lx.*` Logic invocation; or
- Logic-to-View RPC intended for application UI.

The bridge gains a host-to-View control message, conceptually:

```ts
interface EnvironmentSnapshotFrame {
  v: 2
  kind: 'environment.snapshot'
  environment: LxViewEnvironment
}
```

The concrete discriminant and encoding MUST be incorporated into
`bridge-protocol.md`; implementations MUST NOT create a second transport beside
the existing bridge solely for View Environment delivery.

This is transport-level state. It is ordered independently from page business
state, but each snapshot carries `appliedChromeRevision` so a Logic mutation
can be correlated with the host layout that resulted from it.

### 6.2 Initial snapshot

The host MUST install the initial environment before the page framework mounts.
It SHOULD:

1. establish WebView bounds;
2. lay out host chrome;
3. measure and prepare the initial snapshot;
4. install the document-start bootstrap containing that snapshot; and
5. reveal or attach the page only after the bootstrap is available.

The bridge/runtime MUST synchronously expose the bootstrap snapshot. A View
MUST NOT need to await a native round trip before its first layout.

If a final native layout pass changes a provisional measurement, the host MUST
publish a newer snapshot before making the WebView interactive.

### 6.3 Subsequent updates

A new snapshot MUST be published when any relevant value changes, including:

- WebView attachment or replacement;
- host window/container resize;
- device rotation;
- effective host appearance or ShellTheme palette changes;
- system safe-area or gesture-inset changes;
- tabbar visibility, edge, mode, or thickness;
- capsule creation, removal, visibility, or frame;
- navigation-bar layout changes; and
- entry into or exit from native fullscreen.

Updates SHOULD be coalesced per native frame. Duplicate snapshots with
identical geometry and `appliedChromeRevision` SHOULD NOT be published.

### 6.4 Commit and acknowledgement

There is one asynchronous chrome commit path:

```text
Rust mutates ChromeState -> chrome revision N
    -> platform applies revision N
    -> platform completes native layout
    -> platform publishes environment with appliedChromeRevision N
    -> platform acknowledges N
    -> lx.tabBar.update(...) / lx.navigationBar.update(...) resolves
```

The old split between synchronous fire-and-forget UI updates and selected
asynchronous updates is removed. A non-geometric item change may reuse the same
commit mechanism; the host may avoid a layout pass when it can prove geometry
is unchanged, but acknowledgement ordering remains the same.

If there is no attached View, the core stores the new semantic state and may
resolve after that durable state commit because no physical layout exists to
await. The first subsequently attached View MUST be initialized from the
latest revision.

---

## 7. View runtime and public APIs

### 7.1 Package ownership

| Package | Responsibility |
|---|---|
| `@lingxia/bridge` | control-frame types, ordered environment receiver, low-level snapshot subscription |
| `@lingxia/page-runtime` | framework-neutral external store and CSS-variable projection |
| `@lingxia/react` | `useLxViewEnvironment()` |
| `@lingxia/vue` | `useLxViewEnvironment()` composable |
| `@lingxia/html` | synchronous getter and subscription |

`@lingxia/page-runtime` remains internal. Application code imports only its
framework package. `LxViewEnvironment` and its supporting View types MUST NOT
be generated into `@lingxia/types`, which is the Logic-side declaration
surface. The framework packages MAY re-export the shared type definitions from
`@lingxia/bridge`.

### 7.2 Synchronous View APIs

React:

```tsx
import { useLxViewEnvironment } from '@lingxia/react'

function PageHeader() {
  const environment = useLxViewEnvironment()
  // ...
}
```

Vue:

```ts
import { useLxViewEnvironment } from '@lingxia/vue'

const environment = useLxViewEnvironment()
```

HTML:

```ts
import {
  getViewEnvironment,
  subscribeViewEnvironment,
} from '@lingxia/html'

const initial = getViewEnvironment()
const unsubscribe = subscribeViewEnvironment((next) => {
  // update geometry-dependent DOM
})
```

`getViewEnvironment()` is synchronous because it reads the pushed snapshot. No
public Promise getter is defined.

### 7.3 CSS variables

Before notifying JavaScript subscribers, page-runtime MUST atomically project
the snapshot to `document.documentElement`:

```css
:root {
  --lx-system-inset-top: 0px;
  --lx-system-inset-right: 0px;
  --lx-system-inset-bottom: 0px;
  --lx-system-inset-left: 0px;

  --lx-chrome-inset-top: 0px;
  --lx-chrome-inset-right: 0px;
  --lx-chrome-inset-bottom: 0px;
  --lx-chrome-inset-left: 0px;

  --lx-content-inset-top: 0px;
  --lx-content-inset-right: 0px;
  --lx-content-inset-bottom: 0px;
  --lx-content-inset-left: 0px;
}
```

These variables are always defined. Pages SHOULD use the content variables for
ordinary controls:

```css
.bottom-sheet {
  position: fixed;
  left: var(--lx-content-inset-left);
  right: var(--lx-content-inset-right);
  bottom: var(--lx-content-inset-bottom);
}
```

Pages MUST NOT add `env(safe-area-inset-bottom)` to
`--lx-content-inset-bottom`. The content value already represents the effective
avoidance. Adding both can double-count the same physical area.

### 7.4 Exact occlusion use

Insets solve continuous edge avoidance. Exact rectangles solve local collision
problems such as a custom header beside the capsule:

```ts
const capsule = environment.occlusions.find(
  (item) => item.kind === 'capsule',
)

if (capsule && intersects(titleRect, capsule.rect)) {
  // shorten, shift, or choose an alternate header layout
}
```

Framework packages MAY provide shared collision helpers, but the environment
contract does not mandate one positioning algorithm.

---

## 8. Popup and modal policy

### 8.1 Non-modal page popups

Dropdowns, popovers, menus, and bottom sheets rendered inside the page WebView
remain below native host chrome in z-order. They MUST position interactive
content inside `contentInsets` and SHOULD use exact `occlusions` for anchor
collision and flip/shift decisions.

An `extend` layout intentionally permits non-interactive backgrounds, images,
and videos to continue beneath a transparent tabbar. Avoidance applies to
interactive foreground content, not necessarily to the page background.

### 8.2 Standard system modal UI

Standard mobile APIs such as `lx.showModal()` and `lx.showActionSheet()` SHOULD
continue to use native platform presentation. Native presentation is above
tabbar and capsule chrome and therefore does not depend on WebView z-index.

Desktop WebView implementations of the same APIs MUST consume the View
Environment when positioning their DOM presentation.

### 8.3 Custom View modal UI

Calling tabbar show/hide APIs for a transient custom modal is forbidden as the
standard solution. Tabbar visibility is semantic application state; using it
as a modal implementation causes restoration races, navigation conflicts, and
incorrect behavior for nested modals.

The View runtime instead provides a tokenized overlay session:

```ts
interface LxOverlaySession {
  close(): void
}

interface LxOverlayOptions {
  modality: 'modal'
  hostChrome: 'shield'
}

const session = await beginLxOverlay({
  modality: 'modal',
  hostChrome: 'shield',
})

try {
  // mount and await the custom modal
} finally {
  session.close()
}
```

The host maintains overlay tokens per WebView:

- the first modal token installs a native shield above interactive host chrome;
- the shield paints the modal scrim continuation and consumes input;
- additional tokens increment the count without duplicating the layer;
- releasing the last token removes the shield;
- WebView detach, navigation destruction, or crash releases all owned tokens;
- tabbar semantic visibility and chrome revisions are not mutated; and
- the View Environment remains stable while only the shield changes.

The custom dialog or sheet itself remains in the WebView and MUST fit within
the content area. A design that requires arbitrary HTML to render physically
above native chrome must use a native presentation or a separate host-managed
surface; ordinary WebView z-index cannot provide that guarantee.

---

## 9. Platform rendering contract

Every platform follows the same sequence:

1. receive a semantic chrome revision;
2. resolve each style field through runtime override, manifest override, then
   ShellTheme role;
3. resolve effective visibility;
4. apply `resize` or `extend` constraints without inspecting colors;
5. render navigation bar, tabbar, and system glyphs from the same palette;
6. complete layout;
7. convert visible chrome frames to the target WebView CSS viewport;
8. derive system, chrome, and content insets;
9. publish the snapshot; and
10. acknowledge the applied chrome revision.

### 9.1 Android

Android MUST use the actual tabbar and capsule `View` frames after layout. It
must account for the WebView container origin, display density, system bar
insets, gesture navigation, and any transparent navigation-bar placement.

`resize` is implemented with WebView container margins or constraints.
`extend` keeps the WebView container at the full content bounds and places the
tabbar later in native z-order.

### 9.2 iOS

iOS MUST maintain explicit mutable constraints for all four WebView edges.
`resize` changes the appropriate bottom/leading/trailing constraint to the
tabbar edge. `extend` pins the WebView to the host content edge and brings the
tabbar above it.

Frames MUST be converted using UIKit view conversion after
`layoutIfNeeded()`. Safe-area values already excluded by WebView constraints
must not be emitted again.

### 9.3 HarmonyOS

HarmonyOS MUST make `contentMode` authoritative in its ArkUI composition.
`resize` reserves sibling space. `extend` uses a Stack with the tabbar above the
Web component.

The measured component areas, bottom navigation-indicator inset, px/vp
conversion, and Web component origin must all be included when generating CSS
viewport geometry.

### 9.4 macOS, Windows, and Runner

Desktop projections use the same environment contract even when mobile
tabbar items are rendered as sidebar children rather than a bottom bar. Chrome
outside a WebView is not an occlusion. Any runner device frame that projects a
mobile tabbar over the simulated WebView MUST publish the same geometry the
real mobile host would expose.

Desktop sidebar projection consumes the same tabbar semantic roles. A
manifest override remains attached to the lxapp's projected items; an omitted
field uses the host ShellTheme. Desktop MUST NOT introduce a separate palette
for the same tabbar state.

This common contract keeps View code independent from the host projection.

---

## 10. Removal of `getCapsuleRect`

The following surface is removed outright:

- Logic API `lx.getCapsuleRect()`;
- generated `CapsuleRect` declaration;
- `crates/lingxia-logic/src/ui/capsule.rs`;
- `AppRuntime::get_capsule_rect`;
- platform-specific Rust runtime implementations;
- Apple capsule-query FFI;
- Android capsule-query JNI/API plumbing;
- HarmonyOS `getCapsuleRect` NativeBridge registration and callback plumbing;
- public API manifests and tests that require the method; and
- JSON serialization used only by the query path.

The native capsule control and its layout remain. Each host reads the laid-out
capsule frame while producing `LxViewEnvironment.occlusions`.

No deprecated alias, forwarding implementation, or old payload parser is
kept. `CapsuleRect` is not moved to another Logic namespace.

---

## 11. Implementation plan

### Prerequisite — ShellTheme foundation

Complete [`shell-theme-spec.md`](./shell-theme-spec.md): host appearance state,
semantic palette resolution, persistence, platform adapters, WebView color
scheme propagation, and Runner parity. Page chrome MUST NOT create an interim
appearance service.

### Phase 1 — semantic page-chrome model

1. Introduce `TabBarConfig`, `TabBarLayout`, `TabBarStyle`, and
   `TabBarRuntimeState`.
2. Introduce `NavigationBarConfig`, `NavigationBarStyle`, and page-instance
   runtime state.
3. Preserve declared style masks separately from resolved semantic values.
4. Replace both manifest schemas and update scaffolded examples.
5. Reject old flat tabbar/navbar fields and all component light/dark blocks.
6. Introduce chrome revisions and the transactional mutation path.
7. Replace platform alpha inference with `contentMode`.

### Phase 2 — Logic APIs and appearance observation

1. Add `lx.tabBar.update` and `lx.navigationBar.update`.
2. Add string/`null` runtime override semantics.
3. Add synchronous `lx.getAppearance` plus change listener registration.
4. Add home-only persisted `lx.app.setAppearance` on the existing host service.
5. Remove the old flat tabbar/navbar mutation APIs and regenerate types.

### Phase 3 — native rendering and layout parity

1. Implement semantic/fixed style resolution for both bars on every platform.
2. Implement `resize` and `extend` on Android.
3. Implement mutable edge constraints and both modes on iOS.
4. Implement both ArkUI compositions on HarmonyOS.
5. Align desktop sidebar and Runner mobile projections.
6. Verify that hidden bars release layout space and hit-test regions.
7. Refresh page chrome atomically with effective appearance changes.

### Phase 4 — environment transport

1. Add shared Rust/bridge View Environment types.
2. Add the host-to-View `environment.snapshot` control frame.
3. Install the initial document-start snapshot.
4. Implement per-WebView revisioned stores and native acknowledgement.
5. Publish on all required layout and lifecycle changes.

### Phase 5 — View packages

1. Implement the framework-neutral store in page-runtime.
2. Project CSS variables atomically.
3. Add React and Vue `useLxViewEnvironment()` bindings.
4. Add HTML getter and subscription exports.
5. Add unit tests for first snapshot, updates, unsubscribe, and CSS values.

### Phase 6 — overlay coordination

1. Implement View-to-host modal session acquisition.
2. Add per-WebView native shield reference counting.
3. Integrate standard page-runtime popup components.
4. Add lifecycle cleanup and nested-modal tests.

### Phase 7 — delete old surfaces

1. Remove `getCapsuleRect` from Logic and generated declarations.
2. Remove all platform query bridges and FFI.
3. Remove old flat tabbar/navbar APIs and public manifest entries.
4. Confirm removed symbols are absent from source and generated output.

These phases describe dependency order, not a compatibility rollout. The final
branch exposes only the new model.

---

## 12. Verification matrix

### 12.1 Core and schema tests

- the new nested manifest parses;
- every removed flat field is rejected;
- component `style.light` / `style.dark` blocks are rejected;
- omitted tabbar/navbar fields resolve from the active ShellTheme scheme;
- explicit manifest colors remain fixed across appearance changes;
- runtime `null` clears only the matching runtime override;
- `backgroundColor` alpha never changes content mode;
- `tabBar.update` applies all fields atomically;
- `navigationBar.update` is scoped to one page instance and applies atomically;
- invalid edge, mode, thickness, item index, and color values reject without a
  partial state mutation;
- navigation-derived visibility does not overwrite visibility intent; and
- chrome revisions increase exactly once per committed update.

### 12.2 Appearance API tests

- `getAppearance` is synchronously initialized before Logic runs;
- all lxapps can read and subscribe;
- only the home lxapp can persist a preference;
- listener delivery occurs after native/WebView application;
- unsubscribe removes the native listener when no callbacks remain; and
- `setAppearance` rejects and rolls back on persistence failure.

### 12.3 View runtime tests

- the initial environment is synchronously available;
- snapshots are monotonic and stale revisions are ignored;
- CSS variables update before subscribers run;
- React, Vue, and HTML consumers receive the same data;
- capsule occlusions do not inflate the top content inset;
- system and chrome insets combine with `max`, not addition; and
- unsubscribe and WebView teardown release listeners.

### 12.4 Platform layout matrix

Each mobile platform MUST cover:

| Dimension | Values |
|---|---|
| edge | bottom / left / right |
| content mode | resize / extend |
| visibility | visible / hidden / navigation-hidden |
| style alpha | 0 / partial / 255 |
| system navigation | gesture / button where supported |
| orientation | portrait / landscape |
| runtime transition | resize to extend / extend to resize |

Assertions include WebView bounds, tabbar bounds, hit testing, published
occlusion rect, all inset groups, and applied revision.

### 12.5 End-to-end behavior

Using the showcase and `lxdev`, verify that:

1. an unstyled tabbar and navbar follow host light/dark with the WebView;
2. a fixed component override remains unchanged while other semantic fields
   follow appearance;
3. clearing a runtime override restores manifest/ShellTheme inheritance;
4. multiple lxapps inherit one host palette without app-level theme config;
5. an `extend` transparent tabbar shows page background underneath it;
6. an interactive bottom control using content insets remains fully visible
   and clickable;
7. a bottom sheet never opens under the tabbar;
8. a custom header can avoid the capsule without moving the whole page down;
9. a runtime mode transition updates native layout and CSS in one committed
   operation;
10. rotation produces a new correct snapshot;
11. nested modal sessions block host chrome until the final session closes;
12. switching pages cannot leak a modal shield or stale environment; and
13. native fullscreen removes irrelevant chrome occlusions and restores them on
   exit.

---

## 13. Rejected alternatives

### 13.1 Infer layout from `backgroundColor`

Rejected because style changes would mutate layout, partially transparent
colors have ambiguous semantics, and platforms already tend to implement
different alpha thresholds.

### 13.2 Always resize the WebView

Rejected because it prevents immersive backgrounds and media from extending
beneath a transparent or floating tabbar.

### 13.3 Always extend and make every page add padding

Rejected because ordinary opaque tabbars should have native adjacent layout,
first-paint correctness would depend on page CSS, and every application would
need to reproduce framework behavior.

### 13.4 Add `lx.getViewportMetrics()`

Rejected because it leaves View geometry in Logic, introduces an asynchronous
query race, duplicates state between View and Logic, and still requires a
separate change-notification mechanism.

### 13.5 Keep one getter per host control

Rejected because capsule, tabbar, navigation, floating controls, and future
chrome would each require new types, FFI, callbacks, and platform code.

### 13.6 Add a third `inset` content mode

Rejected because it is not a distinct host geometry. It is `extend` plus the
View consuming `contentInsets`. Keeping that composition explicit avoids a
third mode whose native layout is indistinguishable from `extend`.

### 13.7 Hide/show the tabbar around every modal

Rejected because semantic visibility is not transient presentation state. It
breaks nested modal restoration and races with navigation and app-requested
visibility. A reference-counted modal shield is the correct primitive.

### 13.8 Put `light` / `dark` styles on tabbar or navbar

Rejected because every lxapp and every component would repeat the host palette,
and runtime overrides would need ambiguous per-scheme merge/reset rules.
ShellTheme owns appearance; component style owns exceptions.

### 13.9 Add an lxapp `theme`

Rejected because the active child must not restyle global shell chrome. Page
design stays in CSS, while native chrome inherits the host product.

### 13.10 Synchronize native chrome from page JavaScript

Rejected because it flashes on first paint, duplicates code in every lxapp,
races suspended/background pages, and makes native correctness depend on View
framework lifecycle.

---

## 14. Acceptance criteria

The redesign is complete when all of the following are true:

- no source or generated public API contains `getCapsuleRect`,
  `get_capsule_rect`, or `CapsuleRect`;
- no platform selects tabbar layout by inspecting background alpha;
- the old flat tabbar manifest is rejected;
- the old flat navigation-bar manifest is rejected;
- no page-chrome schema contains light/dark style blocks;
- omitted tabbar/navbar style resolves from ShellTheme and updates live;
- explicit component colors remain fixed across appearance changes;
- runtime `null` restores the declarative/semantic fallback chain;
- appearance observation is available to all Logic runtimes while mutation is
  home-only;
- `resize` and `extend` have identical semantics on Android, iOS, HarmonyOS,
  and mobile Runner projections;
- every attached page WebView has one synchronous, revisioned View Environment
  snapshot;
- React, Vue, and HTML expose equivalent View Environment APIs;
- CSS content inset variables are present before application mount;
- ordinary page popups remain outside interactive host-chrome occlusions;
- custom modal sessions block host chrome without mutating tabbar state;
- layout-affecting Logic mutations resolve only after their chrome revision is
  applied; and
- the full platform and end-to-end verification matrix passes.

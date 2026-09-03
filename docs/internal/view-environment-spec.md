# LingXia View Environment and Host Chrome Layout Specification

> Status: proposal (breaking redesign) · Date: 2026-07-28
>
> Platforms: Android / iOS / HarmonyOS / macOS / Windows / Runner
>
> Scope: the ownership, layout, geometry, transport, and View-facing contract
> for host chrome that shares or overlaps an lxapp WebView. This specification
> intentionally provides no compatibility path for the APIs and schemas it
> replaces.

This document uses MUST / MUST NOT / SHOULD / MAY in their normative sense.
Unless a section is explicitly marked as current-state analysis, it describes
the target architecture rather than the implementation that happens to exist
today.

Related specifications:

- [`shell-ui-spec.md`](./shell-ui-spec.md) remains authoritative for shell
  roles and desktop/mobile projection semantics; this document owns the page
  viewport and host-chrome geometry contract.
- [`bridge-protocol.md`](./bridge-protocol.md) remains authoritative for the
  final wire encoding; the control frame shown here defines required semantics,
  not an independently versioned transport.

---

## 0. Decision summary

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

1. put pixel geometry in the View/host boundary rather than the Logic API;
2. give React, Vue, and HTML Views the same synchronous environment snapshot;
3. support bottom, left, right, floating, and future host chrome without adding
   one `getXxxRect()` API per control;
4. make transparent styling independent from WebView layout;
5. use post-layout native frames rather than configured sizes to calculate
   occlusion;
6. update atomically after navigation, rotation, safe-area changes, runtime
   tabbar changes, and WebView attachment;
7. provide both edge insets for ordinary layout and exact rectangles for local
   collision avoidance;
8. keep page popups usable when native chrome is rendered above the WebView;
9. preserve a single cross-platform semantic model while allowing platform
   rendering differences; and
10. eliminate the dedicated capsule geometry FFI and bridge chain.

### 1.2 Non-goals

This specification does not define:

- the visual design of a tabbar, capsule, popup, or modal;
- a general-purpose browser layout engine;
- page business state replication;
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
| Logic | tabbar intent, item state, requested visibility, navigation semantics | native frames, density conversion, WebView-relative rects |
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

---

## 4. Breaking tabbar model

### 4.1 Manifest schema

The old flat shape is replaced by separate layout, style, and items:

```json
{
  "tabBar": {
    "layout": {
      "edge": "bottom",
      "contentMode": "extend",
      "thickness": 72
    },
    "style": {
      "foregroundColor": "#999999",
      "selectedForegroundColor": "#1677ff",
      "backgroundColor": "transparent",
      "dividerColor": "#eeeeee"
    },
    "items": [
      {
        "text": "Home",
        "page": "home",
        "iconPath": "public/home.png"
      },
      {
        "text": "Settings",
        "page": "settings",
        "iconPath": "public/settings.png"
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
  edge: TabBarEdge
  contentMode: TabBarContentMode
  /** Cross-axis size in logical/CSS pixels before platform safe-area addition. */
  thickness: number
}

interface TabBarStyle {
  foregroundColor: string
  selectedForegroundColor: string
  backgroundColor: string
  dividerColor: string
}

interface TabBarConfig {
  layout: TabBarLayout
  style: TabBarStyle
  items: TabBarItemConfig[]
}
```

All three top-level fields are required when `tabBar` is present. New-project
templates SHOULD emit `contentMode: "resize"` unless the template is explicitly
an immersive design.

The parser MUST reject the removed root fields:

- `color`;
- `selectedColor`;
- `backgroundColor`;
- `borderStyle`;
- `position`;
- `dimension`; and
- `list`.

No alias or migration parser is provided.

### 4.2 Why `resize` and `extend`

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

### 4.3 Configuration and runtime state are separate

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

`visibility_intent` records the app request. `effective_visible` is the result
after navigation rules are applied. For example, navigating from a tab page to
a detail page may make the effective value false without overwriting the app's
intent.

### 4.4 Logic mutation API

The flat collection of `showTabBar`, `hideTabBar`, `setTabBarStyle`,
`setTabBarItem`, badge, and red-dot calls SHOULD be replaced by one namespaced,
transactional mutation:

```ts
interface TabBarUpdate {
  visibility?: 'auto' | 'visible' | 'hidden'
  layout?: Partial<TabBarLayout>
  style?: Partial<TabBarStyle>
  items?: Array<{
    index: number
    text?: string
    iconPath?: string
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
visibility; `visibility: "visible"` explicitly reveals it on any route, and
`visibility: "hidden"` explicitly suppresses it.

Navigation remains a separate concern. `switchTab` continues to select and
navigate to a tab item; `tabBar.update` does not navigate.

All errors are reported as rejected promises. Boolean success values are not
part of the new contract.

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
    -> lx.tabBar.update(...) resolves
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
2. resolve effective visibility;
3. apply `resize` or `extend` constraints without inspecting colors;
4. complete layout;
5. convert visible chrome frames to the target WebView CSS viewport;
6. derive system, chrome, and content insets;
7. publish the snapshot; and
8. acknowledge the applied chrome revision.

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

### Phase 1 — semantic model

1. Introduce `TabBarConfig`, `TabBarLayout`, `TabBarStyle`, and
   `TabBarRuntimeState`.
2. Replace the manifest schema and update scaffolded examples.
3. Reject old flat tabbar fields.
4. Introduce chrome revisions and the transactional mutation path.
5. Replace platform alpha inference with `contentMode`.

### Phase 2 — native layout parity

1. Implement `resize` and `extend` on Android.
2. Implement mutable edge constraints and both modes on iOS.
3. Implement both ArkUI compositions on HarmonyOS.
4. Align Runner mobile projections.
5. Verify that hidden tabbars release both layout space and hit-test regions.

### Phase 3 — environment transport

1. Add shared Rust/bridge View Environment types.
2. Add the host-to-View `environment.snapshot` control frame.
3. Install the initial document-start snapshot.
4. Implement per-WebView revisioned stores and native acknowledgement.
5. Publish on all required layout and lifecycle changes.

### Phase 4 — View packages

1. Implement the framework-neutral store in page-runtime.
2. Project CSS variables atomically.
3. Add React and Vue `useLxViewEnvironment()` bindings.
4. Add HTML getter and subscription exports.
5. Add unit tests for first snapshot, updates, unsubscribe, and CSS values.

### Phase 5 — overlay coordination

1. Implement View-to-host modal session acquisition.
2. Add per-WebView native shield reference counting.
3. Integrate standard page-runtime popup components.
4. Add lifecycle cleanup and nested-modal tests.

### Phase 6 — delete the old vertical slice

1. Remove `getCapsuleRect` from Logic and generated declarations.
2. Remove all platform query bridges and FFI.
3. Remove old public API manifest entries.
4. Confirm the symbol is absent from source and generated output.

These phases describe dependency order, not a compatibility rollout. The final
branch exposes only the new model.

---

## 12. Verification matrix

### 12.1 Core and schema tests

- the new nested manifest parses;
- every removed flat field is rejected;
- `backgroundColor` alpha never changes content mode;
- `tabBar.update` applies all fields atomically;
- invalid edge, mode, thickness, item index, and color values reject without a
  partial state mutation;
- navigation-derived visibility does not overwrite visibility intent; and
- chrome revisions increase exactly once per committed update.

### 12.2 View runtime tests

- the initial environment is synchronously available;
- snapshots are monotonic and stale revisions are ignored;
- CSS variables update before subscribers run;
- React, Vue, and HTML consumers receive the same data;
- capsule occlusions do not inflate the top content inset;
- system and chrome insets combine with `max`, not addition; and
- unsubscribe and WebView teardown release listeners.

### 12.3 Platform layout matrix

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

### 12.4 End-to-end behavior

Using the showcase and `lxdev`, verify that:

1. an `extend` transparent tabbar shows page background underneath it;
2. an interactive bottom control using content insets remains fully visible
   and clickable;
3. a bottom sheet never opens under the tabbar;
4. a custom header can avoid the capsule without moving the whole page down;
5. a runtime mode transition updates native layout and CSS in one committed
   operation;
6. rotation produces a new correct snapshot;
7. nested modal sessions block host chrome until the final session closes;
8. switching pages cannot leak a modal shield or stale environment; and
9. native fullscreen removes irrelevant chrome occlusions and restores them on
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

---

## 14. Acceptance criteria

The redesign is complete when all of the following are true:

- no source or generated public API contains `getCapsuleRect`,
  `get_capsule_rect`, or `CapsuleRect`;
- no platform selects tabbar layout by inspecting background alpha;
- the old flat tabbar manifest is rejected;
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

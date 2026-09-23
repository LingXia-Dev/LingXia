# Native Components

LingXia ships two native-backed families:

- **Inline native island** (this page's player contract): `LxNativeRoot`, `LxNativeView`, `LxNativeCover`, `LxNativeText`, `LxNativeButton`, and `LxVideo`. `LxVideo` must be a **direct child** of an explicit `LxNativeRoot`. There is no implicit root. Seek stays on `LxVideo` `controls` / `progressBar`.
- **Presenters / wrappers** (not on the island): `LxPicker`, `LxMediaSwiper`, `LxNavigator`.

Text input is deliberately **not** a component: use plain `<input>` / `<textarea>` (see [Text inputs](#text-inputs--use-plain-input--textarea)).

The components live in `@lingxia/elements` (the pure-JS custom elements) and are re-exported as framework-friendly wrappers from `@lingxia/react` and `@lingxia/vue`. **In React/Vue, almost always import from the framework package**, not from `@lingxia/elements`. HTML views skip the wrappers and use the raw custom-element tags (`<lx-native-root>`, `<lx-video>`, …) once `@lingxia/elements` has registered them; `@lingxia/html` re-exports `registerInlineNativeComponents`.

For framework wiring (event short-path vs. View DOM path, `useLxPage` shape) see [`./guide.md`](./guide.md).

---

## Import shape

```ts
// React
import {
  LxNativeRoot, LxNativeCover, LxNativeView, LxNativeText,
  LxNativeButton, LxVideo,
  LxPicker, LxMediaSwiper, LxNavigator,
} from '@lingxia/react';

// Vue — same names from '@lingxia/vue'

// HTML — register once, then use the raw tags:
// <lx-native-root>, <lx-video>, <lx-native-cover>, …
import { registerInlineNativeComponents } from '@lingxia/html';
registerInlineNativeComponents();
```

The React/Vue wrappers accept all the underlying attributes (camelCase or kebab-case where noted) plus the framework's standard `className` / `class` / `style` / `ref`.

---

## Callback shapes by component

A common source of confusion: not every component passes the same thing to its event handler. The framework wrappers unwrap or reshape some events; others come through as raw DOM `CustomEvent`. Keep this table handy:

| Component | What the handler receives | Example |
|---|---|---|
| Island nodes (`LxNativeButton`, `LxVideo`, Root) | **Payload first** in React/Vue; HTML still reads `CustomEvent.detail` | `onPress(({ source }) => …)`, `onTimeUpdate(({ currentTime }) => …)` |
| `LxPicker` | **Resolved value directly** — `string \| string[]` | `onConfirm(value)`, `onColumnChange(value)` |
| `LxMediaSwiper` | **Raw DOM `CustomEvent`** with a typed `detail` | `onChange(event)` → `event.detail.index` |
| `LxNavigator` | Raw DOM `CustomEvent` | `onSuccess(event)` → `event.detail.success` |

When in doubt: log the value once, or read the component's type export below.

---

## Text inputs — use plain `<input>` / `<textarea>`

There is **no `LxInput` or `LxTextarea` component**. Text inputs are plain
web `<input>` / `<textarea>` elements: the browser engine owns IME, keyboard avoidance, autofill,
selection, and accessibility, and your CSS applies directly. Components must
earn their existence by bridging to capabilities the web cannot deliver —
text input is not one of them. (A native **secure-keyboard modal** for
payment-grade password entry is planned as its own component.)

**The keyboard never covers a focused input** in normal document flow — the
engine scrolls it into view. This is window-level host configuration, done
once per platform (Android consumes IME insets; Harmony sets the Web
component's `RESIZE_CONTENT` keyboard-avoid mode; iOS WKWebView handles it
natively). `position: fixed` inputs are the one edge case to test.

Mini-program input attributes have no LingXia equivalent — write the web ones
(`onInput`, `enterkeyhint`, `inputMode`, `maxLength`, `::placeholder`,
`ref.focus()`, `field-sizing: content`).

**Soft-keyboard height** (rarely needed — e.g. pinning a toolbar above the
IME): derive it from `visualViewport`:

```ts
const onResize = () => {
  const h = Math.max(0, Math.round(window.innerHeight - visualViewport.height));
  // h > 0 while the keyboard is up
};
visualViewport?.addEventListener('resize', onResize);
```

---

## `LxPicker`

Use the framework wrapper's exported props as the authoring contract
(`LxPickerProps` in React). React/Vue infer column selection from `columns`;
set `mode` only for date/time. The raw element's `LxPickerAttributes` is a
lower-level contract and is not the React prop list.

| Wrapper configuration | Confirm value |
|---|---|
| `columns={[['A', 'B']]}` | `string` |
| Multiple independent arrays in `columns` | `string[]` |
| Cascading `columns`: `[parents, childrenByParent]` | `string[]` |
| `mode="date"` | Date string; `fields="range"` returns a pair of strings |
| `mode="time"` | Time string (`HH:mm`) |

**Callback reshaping** — the wrappers unwrap the raw event, so `onConfirm` /
`onColumnChange` receive the resolved **value** directly (a `string` for
single-column / date / time, a `string[]` for multi-column / cascading
and date-range selection).
`onConfirm` fires on the confirm button, `onColumnChange` on each column scroll,
`onCancel()` on cancel/dismiss (no argument).

```tsx
<LxPicker
  columns={[
    ['China', 'USA'],
    ['Beijing', 'Shanghai'],
  ]}
  value={['China', 'Beijing']}
  onConfirm={(value) => actions.setCity({ value })}
  onColumnChange={(value) => console.log('scrolling', value)}
/>
```

---

## Inline native island

The island is one native composition tree laid out by CSS. The page still has exactly one View WebView. Standard playback is:

```tsx
<LxNativeRoot className="player">
  <LxVideo src={data.src} aria-label={data.title} controls />
</LxNativeRoot>
```

`LxVideo` is a **direct** child of `LxNativeRoot`. Nested Root, DOM inside Root, a bare `LxVideo`, or Video inside View/Cover is `NATIVE_ROOT_INVALID_STRUCTURE`. Built-in `controls` can coexist with any `LxNativeButton`; button icons do not determine their behavior. Seek is the video leaf's built-in progress bar, not a separate island slider.

**Lifecycle:** the host owns the island container. Removing an `LxNativeRoot`, or
destroying/replacing its page WebView, unmounts every node owned by that Root, stops
its native video resources, and resets its lease before accepting a new commit. Navigating back therefore
cannot leave a stale video surface or an invisible touch-blocking overlay behind.

**Scrolling:** layout snapshots use unscrolled document CSS coordinates plus the
current page viewport offset. The host moves native visuals with top-level and
nested scrolling, clips partially visible nodes against overflow containers inside
and outside Root, and removes fully offscreen nodes from paint and hit testing.
Clipping preserves the original video dimensions; it does not resize the picture.

Cover and Button are author recipes: they expand to `view` / `tappable` before the host commit. Host factories are only `root`, `view`, `text`, `tappable`, and `video`. `LxPicker` / `LxMediaSwiper` / `LxNavigator` stay on the presenter overlay channel.

### Styles and composition limits

Use these components for UI on native surfaces, such as a video menu. Ordinary
page UI stays in HTML; Web component libraries cannot be placed inside a Root.

React/Vue `style` accepts a constrained `NativeStyle`. CSS classes use the same
native rendering limits:

| Area | Supported contract |
|---|---|
| Layout | DOM-measured flex/grid, sizing, positioning, spacing, aspect ratio and overflow clipping |
| Paint | Solid background color, opacity, uniform solid border and a uniform circular corner radius in CSS pixels |
| Text | Font size/weight, line height, text alignment and color; `LxNativeText` also exposes these as props |
| Unsupported | Transforms, shadows, gradients, filters, masks, CSS animation/transitions, per-side borders and unequal/elliptical/percentage corner radii |

Unsupported computed styles (including class styles) produce recoverable
`NATIVE_ROOT_UNSUPPORTED_STYLE` / `NATIVE_ROOT_UNSUPPORTED_LAYOUT` diagnostics
through Root `onError` and the console. Diagnostics name the component/id and
property, and repeat only when the problem changes or reappears. The Root keeps
rendering its supported subset; a style diagnostic does not activate fallback.
Do not rely on the unsupported effect being reproduced by native rendering.

`LxNativeButton.icon` accepts `close`, `play`, `pause`, `mute`, `unmute`,
`fullscreen`, or `more`. These are visual symbols, not automatic video commands;
connect behavior with `onPress`. Arbitrary strings and `{ resource }` icons are
rejected with `NATIVE_COMPONENT_INVALID_PROPS`. Custom resource icons are not
part of the supported contract yet.

Buttons measure their label/icon and have an inline-flex default layout; explicit
CSS sizing and layout override it. Text typography props participate in DOM
measurement, with author CSS taking precedence. Inherited font/color styles are
resolved before sending nodes to native rendering. CSSOM updates, pseudo-classes
and media-query paint changes are observed even when geometry is unchanged.
Browser-supported colors (including `oklch()` and Display P3) are converted to sRGB
for native rendering. Ancestor opacity multiplies each native node's opacity;
this is per-node alpha, not an offscreen group-compositing operation.
Once active, Root suppresses the measurement DOM's paint so translucent native
content is not drawn a second time underneath; DOM layout remains available.

Use stable `id` values when retaining nodes across a change of parent. Sorting and
updating props in the same render preserves node identity and event routing.
React Root refs expose only `{ retry, getLayout }`, including callback refs.

Fullscreen is owned by `LxVideo`: its controls and `lx.createVideoContext(id)`
control the player. Custom Root siblings do not move into the player's fullscreen
window. Root-wide `fullscreenScope` and animated `hiddenTransition` are not
supported APIs; author attributes with those names are rejected. Visibility
changes are immediate.

The cross-platform Button event is `onPress` (`@press` in Vue). Focus/hover and
Root pointer-within callbacks are not public framework APIs yet: host support
varies. Platform events that do arrive remain available as raw element events.

Root `onReady` fires when a Root becomes active, not on subsequent commits. The
optional React `fallback` prop / Vue `#fallback` slot supplies ordinary DOM for
initialization or failure; it never switches playback to a Web video element.
Shown fallback content is accessible, and becomes hidden when the Root is ready.

## `LxVideo`

Native video player with quality/rate switching, fullscreen, and live mode. Always wrap it in `LxNativeRoot`.

Standard HTML `<video>`, `<audio>`, and `new Audio()` are also supported in
View. Use them for ordinary playback and DOM/CSS composition; use `LxVideo`
for native controls and `lx.createVideoContext(id).setStreamSource(...)`.
Web media allows HTTPS, same-origin, `lx:`, `lingxia:`, `data:`, and `blob:`
sources, like images. This does not grant View fetch/XHR access: players that
fetch segments themselves need separate network support. Native `LxVideo`
continues to use the host-granted network policy below.

Web playback follows the platform WebView's codec, autoplay, and fullscreen
support. Handle rejected `play()` promises, and pause/release media when the
page is hidden or unmounted; background playback is not guaranteed. Keep
references to `new Audio()` instances so they can be stopped too.

The full attribute list (`src`, `poster`, `objectFit`, `controls`, `qualities`,
`playbackRates`, …) is the exported `LxVideoAttributes` from `@lingxia/elements`.
Every remote media URL (`src`, `poster`, watermark/quality URLs, and
`setStreamSource`) must be allowed by the host-granted network policy;
protocol-relative URLs and unsupported schemes are rejected. Two pieces of
behavior are doc-only: event reshaping and imperative control.

**Events** — React/Vue handlers receive the **payload** (`onTimeUpdate` →
`{ currentTime }`, `onError` → `{ code, message, recoverable? }`,
`onLoadedMetadata` → `{ duration, width?, height? }`, `onVolumeChange` →
`{ volume, muted? }`, `onFullscreenChange` → `{ fullscreen }`, normalizing the
platform's `fullScreen` alias). HTML still reads `CustomEvent.detail`. Lifecycle
events (`onPlayRequest`, `onPlay`, `onPlaying`, `onPause`, `onStop`, `onEnded`,
`onWaiting`) carry `{}`. Type a named handler with `LxVideoEventPayloads` from
your framework package.

```tsx
<LxNativeRoot>
  <LxVideo
    id="hero"
    src="https://cdn.example.com/intro.mp4"
    controls
    onTimeUpdate={actions.onProgress}     // ({ currentTime }) => …
    onError={actions.onVideoError}
  />
</LxNativeRoot>
```

**Imperative control from Logic** (`pages/.../index.ts`) — give the element an
`id`, then drive it via `lx.createVideoContext(id)` (`VideoContext`):

```ts
const ctx = lx.createVideoContext('hero');
ctx.play();
ctx.pause();
ctx.stop();
ctx.seek(30);            // seconds
ctx.requestFullScreen();
ctx.exitFullScreen();
ctx.setStreamSource({ /* … */ });
```

---

## `LxMediaSwiper`

Carousel for images and videos with native paging (loop, autoplay, dots, peek,
vertical/horizontal). The full attribute list is the exported
`LxMediaSwiperAttributes`, and items are `LxMediaSwiperItem` (`@lingxia/elements`):

```ts
type LxMediaSwiperItem =
  | { id?: string; type: 'image'; src: string }
  | { id?: string; type: 'video'; src: string; poster?: string; controls?: boolean; muted?: boolean };
```

**Events** — each handler receives a raw DOM `CustomEvent`; read `event.detail`,
whose shape is the exported `*EventDetail` types (`LxMediaSwiperChangeEventDetail`,
`LxMediaSwiperEndReachedEventDetail`, `LxMediaSwiperErrorEventDetail`, …).
`onChange` / `onTransitionEnd` carry `{ index, previousIndex, item, source }`;
`onTap` / `onVideoEnded` carry `{ index, item }`; `onEndReached` fires when the
user reaches the last item; `onError` carries an error `code`.

```tsx
<LxMediaSwiper
  items={[
    { type: 'image', src: 'https://cdn.example.com/a.jpg' },
    { type: 'video', src: 'https://cdn.example.com/b.mp4', controls: true },
  ]}
  loop
  dots
  onChange={(e) => actions.onSlideChange({ index: e.detail.index })}
  onEndReached={actions.loadMore}
/>
```

---

## `LxNavigator`

Declarative navigation — wraps content that, when tapped, opens a page, another
lxapp, or a URL. The full attribute list is the exported
`LxNavigatorAttributes` from `@lingxia/elements`. `page` is the configured page
name from `lxapp.json`; full routes are not accepted through a `path` attribute.

**`target`** — what to open. Inferred when omitted: `app-id` → `lxapp`,
`url` → `url`, otherwise `page`.

**`as`** — where to open it. Same values and options as Logic's
`lx.surface.openUrl` / `lx.surface.openPage`:

| `target` | `as` | Result | Extra attributes |
|---|---|---|---|
| `url` | `external` (default) | System browser | — |
| `url` | `tab` | In-app browser tab | — |
| `url` | `aside` | Browser docked beside the main | `edge`, `size` |
| `page` | *(omitted)* | Runs `open-type` in the page stack | — |
| `page` | `float` | Page float over the current surface | `position`, `size`, `interaction` |
| `page` | `window` | Separate desktop window (rejected on mobile) | `chrome`, `size`, `interaction` |
| `lxapp` | *(not allowed)* | Opens `app-id` (optional `page`, `query`, `channel`, `target-version`) | — |

`size` and `interaction` are objects in React/Vue (JSON strings on the raw
`<lx-navigator>` tag). A placement opens without handing back a handle; use
Logic's `lx.surface.*` when you need to message, hide, or close the surface.

**`open-type`** — applies to `target="page"` without `as`, except that
`navigateBack`, `exit`, and `tel` work for any target:

| Value | Behavior |
|---|---|
| `navigate` (default) | Push a new page in the current lxapp |
| `redirect` | Replace the current page |
| `navigateBack` | Pop back by `delta`; with `target="lxapp"`, return from the opened lxapp |
| `reLaunch` | Restart the app at a new page |
| `switchTab` | Switch to a tab page |
| `exit` | Exit the current lxapp |
| `tel` | Trigger a phone call (use with `phone-number`) |

**Events:** `onSuccess` / `onFail` / `onComplete` — `event.detail` is
`{ success?: boolean; errMsg?: string }`.

```tsx
<LxNavigator page="detail" query={{ id: 42 }} onFail={actions.onNavFail}>
  <div>Open detail</div>
</LxNavigator>

<LxNavigator url="https://example.com" as="aside" edge="right">
  <div>Docs</div>
</LxNavigator>
```

For imperative navigation from Logic, use the `lx.navigateTo({...})` family — see [`./lx-api.md`](./lx-api.md).

---

## Two event paths (recap)

LingXia components support two delivery paths:

1. **Logic short path** (native → Rust → Logic JS, 3 hops). Used when the handler you pass is one of `useLxPage().actions`. The CLI auto-generates `pageFuncBindings` so events route to Logic directly, skipping the WebView roundtrip.
2. **View DOM path** (native → WebView `CustomEvent` → handler, 2 hops). Used when the handler is a local View function (e.g., a React `useState` setter).

You don't pick between them. Pass an `actions.foo` and you get the short path; pass a local function and you get the DOM path.

---

## Authoritative attribute types

For any attribute not covered above, the underlying types are exported from `@lingxia/elements`:
`LxPickerAttributes`, `LxVideoAttributes`, `LxMediaSwiperAttributes`, `LxNavigatorAttributes`, plus matching `*EventDetail` and `*Event` types.

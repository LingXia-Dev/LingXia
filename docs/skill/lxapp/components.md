# Native Components

LingXia ships two native-backed families:

- **Inline native island** (this page's player contract): `LxNativeRoot`, `LxNativeView`, `LxNativeCover`, `LxNativeText`, `LxNativeButton`, `LxNativeSlider`, and `LxVideo`. `LxVideo` must be a **direct child** of an explicit `LxNativeRoot`. There is no implicit root.
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
  LxNativeButton, LxNativeSlider, LxVideo,
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
| Island nodes (`LxNativeButton`, `LxNativeSlider`, `LxVideo`, Root) | **Payload first** in React/Vue; HTML still reads `CustomEvent.detail` | `onPress(({ source }) => …)`, `onValueCommit(({ value }) => …)`, `onTimeUpdate(({ currentTime }) => …)` |
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

**Mini-program attribute mapping** (for code ported from WeChat-style
`<input>`):

| Mini-program | Plain web equivalent |
|---|---|
| `bindinput` / `bindconfirm` | `onInput` / `onKeyDown` + `key === 'Enter'` |
| `confirm-type` | `enterkeyhint` attribute |
| `type="digit"` | `type="text" inputMode="decimal"` |
| `type="number"` | `type="number"` (or `inputMode="numeric"`) |
| `maxlength` | `maxLength` |
| `placeholder-style` | CSS `::placeholder` |
| `focus` (programmatic) | `ref.focus()` / `ref.blur()` driven by Logic state |
| `auto-height` (textarea) | CSS `field-sizing: content`, or on input: `el.style.height = 'auto'; el.style.height = el.scrollHeight + 'px'` |
| `bindlinechange` (textarea) | derive from `scrollHeight / lineHeight` in the same handler |

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

Native picker — modal column/date/time selection the web platform can't render natively. Full attribute list is the exported `LxPickerAttributes` (+ `LxPickerColumn`, `LxPickerCascadingColumns`) from `@lingxia/elements`; the doc-only behavior is the `mode` → value-type mapping and the callback reshaping below.

**Modes (`mode` attribute)** — the mode determines the confirm value type:

| Mode | Confirm value type |
|---|---|
| `selector` (default) | `string` |
| `multiSelector` | `string[]` |
| `cascading` | `string[]` |
| `date` | `string` (`YYYY-MM-DD`) |
| `time` | `string` (`HH:mm`) |

**Callback reshaping** — the wrappers unwrap the raw event, so `onConfirm` /
`onColumnChange` receive the resolved **value** directly (a `string` for
`selector` / `date` / `time`, a `string[]` for `multiSelector` / `cascading`).
`onConfirm` fires on the confirm button, `onColumnChange` on each column scroll,
`onCancel()` on cancel/dismiss (no argument).

```tsx
<LxPicker
  mode="multiSelector"
  columns={[
    ['China', 'USA'],
    ['Beijing', 'Shanghai'],
  ]}
  defaultIndex={[0, 0]}
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

`LxVideo` is a **direct** child of `LxNativeRoot`. Nested Root, DOM inside Root, a bare `LxVideo`, or Video inside View/Cover is `NATIVE_ROOT_INVALID_STRUCTURE`. `controls={true}` cannot share a Root with `LxNativeSlider` or a play/pause/mute/fullscreen `LxNativeButton`.

**Lifecycle:** the host owns the island container. Removing an `LxNativeRoot`, or
destroying/replacing its page WebView, unmounts every node owned by that Root, stops
its native video resources, and resets its lease before accepting a new commit. Navigating back therefore
cannot leave a stale video surface or an invisible touch-blocking overlay behind.

Cover and Button are author recipes: they expand to `view` / `tappable` before the host commit. Host factories are only `root`, `view`, `text`, `tappable`, `slider`, and `video`. `LxPicker` / `LxMediaSwiper` / `LxNavigator` stay on the presenter overlay channel.

## `LxVideo`

Native video player with quality/rate switching, fullscreen, and live mode. Always wrap it in `LxNativeRoot`.

Video is native-owned: a page `<video>` opens a second decode-and-surface stack
that shares no z-order, clip, fullscreen, or lifecycle with this one. `<video>`,
`<audio>`, and `new Audio()` are rejected by `lingxia build`; `video.srcObject`
is outside the contract as well. Live or pushed streams stay native-side through
`lx.createVideoContext(id).setStreamSource(...)`. Audio playback is not
available yet.

The full attribute list (`src`, `poster`, `objectFit`, `controls`, `qualities`,
`playbackRates`, …) is the exported `LxVideoAttributes` from `@lingxia/elements`;
every remote media URL (`src`, `poster`, watermark/quality URLs, and
`setStreamSource`) must be under `security.network.trustedDomains`. Two pieces of
behavior are doc-only: event reshaping and imperative control.

**Events** — React/Vue handlers receive the **payload** (`onTimeUpdate` →
`{ currentTime }`, `onError` → `{ code, message, recoverable }`,
`onLoadedMetadata` → `{ duration, width, height }`, `onFullscreenChange` →
`{ fullscreen }`). HTML still reads `CustomEvent.detail`. Lifecycle events
(`onPlayRequest`, `onPlay`, `onPlaying`, `onPause`, `onStop`, `onEnded`,
`onWaiting`) carry `{}`.

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

Declarative navigation — wraps content that, when tapped, navigates inside or
outside the lxapp. The full attribute list (`url`, `page`, `query`,
`delta`, `app-id`, `env-version`, `target-version`, `phone-number`, …) is the
exported `LxNavigatorAttributes` from `@lingxia/elements`. The doc-only behavior
is the routing it selects via `open-type` / `target`. `page` is the configured
page name from `lxapp.json`; full routes are not accepted through a `path`
attribute.

**`open-type` routing:**

| Value | Behavior |
|---|---|
| `navigate` (default) | Push a new page in the current lxapp |
| `redirect` | Replace the current page |
| `navigateBack` | Pop back; use `delta` for distance |
| `reLaunch` | Restart the app at a new page |
| `switchTab` | Switch to a tab page |
| `exit` | Exit the current lxapp |
| `openUrl` | Open an external URL (or another lxapp) |
| `tel` | Trigger a phone call (use with `phone-number`) |

**`target`:** `self` (default), `lxapp`, `browser` — auto-inferred from
`open-type` if omitted.

**Events:** `onSuccess` / `onFail` / `onComplete` — `event.detail` is
`{ success?: boolean; errMsg?: string }`.

```tsx
<LxNavigator page="detail" query='{"id":42}' onFail={actions.onNavFail}>
  <div>Open detail</div>
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

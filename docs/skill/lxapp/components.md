# Native Components

LingXia ships native-backed components for lxapp views: `LxPicker`, `LxVideo`, `LxMediaSwiper`, `LxNavigator` — reserved for capabilities the web platform cannot deliver. Text input is deliberately **not** a component: use plain `<input>` / `<textarea>` (see [Text inputs](#text-inputs--use-plain-input--textarea)).

The components live in `@lingxia/elements` (the pure-JS custom elements) and are re-exported as framework-friendly wrappers from `@lingxia/react`, `@lingxia/vue`, and `@lingxia/html`. **Almost always import from the framework package**, not from `@lingxia/elements`.

For framework wiring (event short-path vs. View DOM path, `useLxPage` shape) see [`./guide.md`](./guide.md).

---

## Import shape

```ts
// React
import { LxVideo, LxPicker, LxMediaSwiper, LxNavigator } from '@lingxia/react';

// Vue
import { LxVideo, LxPicker, LxMediaSwiper, LxNavigator } from '@lingxia/vue';

// HTML (custom-element registration runs automatically when @lingxia/html is loaded)
// Use the tag names directly in markup: <lx-video>, <lx-picker>, <lx-media-swiper>, <lx-navigator>
```

The React/Vue wrappers accept all the underlying attributes (camelCase or kebab-case where noted) plus the framework's standard `className` / `class` / `style` / `ref`.

---

## Callback shapes by component

A common source of confusion: not every component passes the same thing to its event handler. The framework wrappers unwrap or reshape some events; others come through as raw DOM `CustomEvent`. Keep this table handy:

| Component | What the handler receives | Example |
|---|---|---|
| `LxPicker` | **Resolved value directly** — `string \| string[]` | `onConfirm(value)`, `onColumnChange(value)` |
| `LxVideo` | **Raw DOM `Event`** | `onPlaying(event)` → `event.detail` |
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

Native picker with several modes.

**Modes (`mode` attribute):**

| Mode | Columns shape | Confirm value type |
|---|---|---|
| `selector` (default) | `string[]` (one column) | `string` |
| `multiSelector` | `string[][]` (parallel columns) | `string[]` |
| `cascading` | `LxPickerCascadingColumns` (tree) | `string[]` |
| `date` | configured via `fields`, `start`, `end` | `string` (`YYYY-MM-DD`) |
| `time` | hours/minutes | `string` (`HH:mm`) |

**Key attributes:**

| Attribute | Type | Notes |
|---|---|---|
| `mode` | one of above | |
| `columns` | `LxPickerColumn[] \| LxPickerCascadingColumns` | Required for selector / multiSelector / cascading. |
| `defaultIndex` | `number \| number[]` | Initial selected index(es). |
| `value` | `string` | Initial date/time for `date`/`time` modes. |
| `start` / `end` | `string` | Date/time range bounds. |
| `fields` | `'year' \| 'month' \| 'day' \| 'range'` | Date mode granularity. |
| `cancelText` / `confirmText` | `string` | Button labels. |
| `cancelButtonColor` / `confirmButtonColor` / `cancelTextColor` / `confirmTextColor` | `string` (hex) | Styling. |

**Event handler shapes** — all three callbacks receive the resolved **value**
(the wrappers unwrap the raw event): a `string` for `selector` / `date` /
`time`, a `string[]` for `multiSelector` / `cascading`.

| Event prop (React) | Vue event | Fires on |
|---|---|---|
| `onConfirm(value)` | `@confirm` | confirm button |
| `onCancel()` | `@cancel` | cancel button / dismiss |
| `onColumnChange(value)` | `@column-change` | a column scrolled, before confirm |

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

## `LxVideo`

Native video player.

**Attributes (`LxVideoAttributes`):**

| Attribute | Type | Notes |
|---|---|---|
| `id` | `string` | Pass to `lx.createVideoContext(id)` in Logic to imperatively control the player. |
| `src` | `string` | Video URL. Must be under `security.network.trustedDomains` if remote. |
| `poster` | `string` | Cover image URL. |
| `objectFit` | `'cover' \| 'contain' \| 'fill' \| 'fit'` | |
| `contentRotate` | `0 \| 90 \| 180 \| 270` | |
| `autoplay` / `loop` / `muted` | `boolean` | |
| `controls` | `boolean` | Show native controls UI. |
| `progressBar` | `boolean` | Show progress bar (subset of controls). |
| `live` | `boolean` | Live-stream mode (disables seek). |
| `volume` | `string \| number` | 0–1. |
| `qualities` | `LxVideoQuality[]` (`{ label, url? }`) | First item is the default quality. |
| `playbackRates` | `number[]` | First item is the default rate. |

**Events** — every handler receives a **raw DOM `Event`**. The native player encodes data on `event.detail`.

| Event prop | Meaning |
|---|---|
| `onPlayRequest` | user tapped play (before playback starts) |
| `onPlay` | playback started |
| `onPlaying` | playback resumed/buffering ended |
| `onPause` | paused |
| `onStop` | stopped |
| `onEnded` | reached end |
| `onTimeUpdate` | progress update — read `event.detail.currentTime` |
| `onError` | playback failed — read `event.detail.code` / `event.detail.message` |
| `onLoadedMetadata` | metadata available — `event.detail.duration`, `width`, `height` |
| `onFullscreenChange` | entered/exited fullscreen — `event.detail.fullScreen` |
| `onWaiting` | buffering |
| `onQualityChange` | user picked a different quality entry |
| `onRateChange` | user picked a different playback rate |

```tsx
<LxVideo
  id="hero"
  src="https://cdn.example.com/intro.mp4"
  poster="https://cdn.example.com/intro.jpg"
  controls
  qualities={[
    { label: '1080p', url: 'https://cdn.example.com/intro-1080.mp4' },
    { label: '720p',  url: 'https://cdn.example.com/intro-720.mp4' },
  ]}
  playbackRates={[1.0, 1.5, 2.0]}
  onTimeUpdate={actions.onProgress}     // (event) => { event.detail.currentTime }
  onError={actions.onVideoError}
/>
```

**Imperative control from Logic** (`pages/.../index.ts`):

```ts
const ctx = lx.createVideoContext('hero');
ctx.play();
ctx.pause();
ctx.seek(30);            // seconds
ctx.requestFullScreen();
ctx.exitFullScreen();
ctx.setStreamSource({ /* … */ });
```

---

## `LxMediaSwiper`

Carousel for images and videos with native paging.

**Attributes (`LxMediaSwiperAttributes`):**

| Attribute | Type | Notes |
|---|---|---|
| `items` | `LxMediaSwiperItem[]` | See item shape below. |
| `index` | `number` | Controlled current index. |
| `initialIndex` | `number` | Uncontrolled initial index. |
| `loop` | `boolean` | |
| `autoplay` / `interval` | `boolean` / `number (ms)` | |
| `animation` | `'slide' \| 'none'` | |
| `animationDuration` | `number` | ms |
| `direction` | `'horizontal' \| 'vertical'` | |
| `contentRotate` / `objectFit` | same as `LxVideo` | |
| `controls` / `muted` | `boolean` | Forwarded to video items. |
| `dots` | `boolean \| { color?, activeColor? }` | Page indicator. |
| `swipeEnabled` | `boolean` | |
| `peek` | `LxMediaSwiperPeek` | Show adjacent items. |

**Item shape:**

```ts
type LxMediaSwiperItem =
  | { id?: string; type: 'image'; src: string }
  | { id?: string; type: 'video'; src: string; poster?: string; controls?: boolean; muted?: boolean };
```

**Events** — handler receives `CustomEvent<...EventDetail>`. Read `event.detail`.

| Event | `event.detail` shape |
|---|---|
| `onChange` | `{ index, previousIndex, item, source: 'touch' \| 'autoplay' \| 'api' \| 'video' }` |
| `onTransitionEnd` | same as `onChange` |
| `onTap` | `{ index, item }` |
| `onVideoEnded` | `{ index, item }` |
| `onEndReached` | `{ index, item, source }` — fires when the user reaches the last item |
| `onError` | `{ index, item, code: 'not_found' \| 'network' \| 'decode' \| … }` |

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

Declarative navigation — wraps content that, when tapped, navigates inside or outside the lxapp.

**Open types** (`open-type`):

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

**Targets** (`target`): `self` (default), `lxapp`, `browser`.

**Attributes:**

| Attribute | Use |
|---|---|
| `url` | Browser URL for `openUrl` / `browser` target |
| `page` | Named page in `lxapp.json` |
| `path` | Raw page path; supports query string |
| `query` | JSON-encoded page query params |
| `open-type` | one of the above |
| `target` | one of the above (auto-inferred if omitted) |
| `delta` | how many pages to pop for `navigateBack` |
| `app-id` | target lxapp ID for cross-app navigation |
| `env-version` | `'release' \| 'preview' \| 'develop'` |
| `target-version` | exact target lxapp version |
| `phone-number` | required for `open-type="tel"` |

**Events:**

- `onSuccess` / `onFail` / `onComplete` — `event.detail` is `{ success?: boolean; errMsg?: string }`.

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

## Where these wrappers come from

- **Pure JS custom elements** live in `@lingxia/elements` (`registerVideoComponent`, `LxVideoElement`, etc.). Importing `@lingxia/elements` registers `<lx-video>`, `<lx-picker>`, … into `customElements`.
- **React wrappers** (`@lingxia/react`) wrap each custom element with prop-to-attribute translation and `pageBindings` injection.
- **Vue wrappers** (`@lingxia/vue`) do the same for Vue's reactivity.
- **HTML** views use the custom elements directly — `@lingxia/html` only handles page state / actions (`subscribe`, `getActions`).

For attributes not listed here (rare, mostly low-level styling escape hatches), the underlying types are exported from `@lingxia/elements`:
`LxPickerAttributes`, `LxVideoAttributes`, `LxMediaSwiperAttributes`, `LxNavigatorAttributes`, plus matching `*EventDetail` and `*Event` types.

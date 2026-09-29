# Native components

Two native-backed families, plus plain web text inputs:

- **Inline native island**: `LxNativeRoot` with `LxVideo`, `LxNativeView`,
  `LxNativeCover`, `LxNativeText`, `LxNativeButton`.
- **Presenters**: `LxPicker`, `LxMediaSwiper`, `LxNavigator`.
- **Text input** is plain `<input>` / `<textarea>`; there is no `LxInput`.

Attribute and event types are exported from `@lingxia/elements`
(`LxPickerAttributes`, `LxVideoAttributes`, `LxMediaSwiperAttributes`,
`LxNavigatorAttributes`, `*EventDetail`).

## Import

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

Wrappers take the element attributes plus `className` / `class` / `style` /
`ref`. An `actions.*` handler goes straight to Logic; a local function gets the
View event.

## Callback shapes

| Component | What the handler receives | Example |
|---|---|---|
| Island nodes (`LxNativeButton`, `LxVideo`, Root) | **Payload first** in React/Vue; HTML still reads `CustomEvent.detail` | `onPress(({ source }) => …)`, `onTimeUpdate(({ currentTime }) => …)` |
| `LxPicker` | **Resolved value directly** — `string \| string[]` | `onConfirm(value)`, `onColumnChange(value)` |
| `LxMediaSwiper` | **Payload first** in React/Vue; HTML reads `CustomEvent.detail` | `onChange(({ index }) => …)` |
| `LxNavigator` | Raw DOM `CustomEvent` | `onSuccess(event)` → `event.detail.success` |

## Text inputs

Use `<input>` / `<textarea>` with web attributes (`onInput`, `enterkeyhint`,
`inputMode`, `maxLength`, `field-sizing: content`). The keyboard never covers a
focused input in normal flow; test `position: fixed` inputs. To pin something
above the soft keyboard, measure it:

```ts
const onResize = () => {
  const h = Math.max(0, Math.round(window.innerHeight - visualViewport.height));
  // h > 0 while the keyboard is up
};
visualViewport?.addEventListener('resize', onResize);
```

## `LxPicker`

The wrapper props (`LxPickerProps`) are the contract. Column mode comes from
`columns`; set `mode` only for date/time.

| Wrapper configuration | Confirm value |
|---|---|
| `columns={[['A', 'B']]}` | `string` |
| Multiple independent arrays in `columns` | `string[]` |
| Cascading `columns`: `[parents, childrenByParent]` | `string[]` |
| `mode="date"` | Date string; `fields="range"` returns a pair of strings |
| `mode="time"` | Time string (`HH:mm`) |

`onConfirm` fires on confirm, `onColumnChange` on each column scroll,
`onCancel()` on dismiss.

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

## Inline native island

One native composition tree laid out by CSS, inside the page's single WebView:

```tsx
<LxNativeRoot className="player">
  <LxVideo src={data.src} aria-label={data.title} controls />
</LxNativeRoot>
```

- `LxVideo` is a **direct** child of an explicit `LxNativeRoot`. A bare
  `LxVideo`, nested Root, DOM inside Root, or Video inside View/Cover is
  `NATIVE_ROOT_INVALID_STRUCTURE`.
- Use the island for UI on native surfaces (a video menu); ordinary page UI
  stays HTML. Web component libraries cannot go inside a Root.
- `LxNativeButton` fires `onPress` (`@press` in Vue). `icon` is one of `close`,
  `play`, `pause`, `mute`, `unmute`, `fullscreen`, `more`: a symbol, not a
  command. Other values reject with `NATIVE_COMPONENT_INVALID_PROPS`.
- Give nodes stable `id`s to keep identity across a parent change. React Root
  refs expose `{ retry, getLayout }`.
- Root `onReady` fires once, when the Root becomes active. The React `fallback`
  prop / Vue `#fallback` slot shows DOM during setup or failure.
- Fullscreen belongs to `LxVideo`; Root siblings do not follow it into the
  fullscreen window.

### Style limits

`style` and classes follow the native rendering limits:

| Area | Supported contract |
|---|---|
| Layout | DOM-measured flex/grid, sizing, positioning, spacing, aspect ratio and overflow clipping |
| Paint | Solid background color, opacity, uniform solid border and a uniform circular corner radius in CSS pixels |
| Text | Font size/weight, line height, text alignment and color; `LxNativeText` also exposes these as props |
| Unsupported | Transforms, shadows, gradients, filters, masks, CSS animation/transitions, per-side borders and unequal/elliptical/percentage corner radii |

Unsupported styles report `NATIVE_ROOT_UNSUPPORTED_STYLE` /
`NATIVE_ROOT_UNSUPPORTED_LAYOUT` through Root `onError` and the console; the
Root keeps rendering the supported subset.

## `LxVideo`

Native video with quality/rate switching, fullscreen, and live mode, always
inside `LxNativeRoot`.

- `lingxia build` rejects `<video>`, `<audio>`, and `new Audio()`;
  `video.srcObject` is unsupported. Live streams use
  `lx.createVideoContext(id).setStreamSource(...)`. Audio playback is not
  available.
- Remote URLs (`src`, `poster`, quality/watermark URLs, stream sources) follow
  the host [network grant](../native/permissions.md); protocol-relative URLs
  are rejected.
- React/Vue handlers get payloads: `onTimeUpdate` → `{ currentTime }`,
  `onError` → `{ code, message, recoverable? }`, `onLoadedMetadata` →
  `{ duration, width?, height? }`, `onVolumeChange` → `{ volume, muted? }`,
  `onFullscreenChange` → `{ fullscreen }`. `onPlayRequest`, `onPlay`,
  `onPlaying`, `onPause`, `onStop`, `onEnded`, `onWaiting` carry `{}`. Type
  handlers with `LxVideoEventPayloads`.

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

Drive it from Logic by `id`:

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

## `LxMediaSwiper`

A native carousel of images and videos (loop, autoplay, dots, peek,
vertical/horizontal):

```ts
type LxMediaSwiperItem =
  | { id?: string; type: 'image'; src: string }
  | { id?: string; type: 'video'; src: string; poster?: string; controls?: boolean; muted?: boolean };
```

Handlers get the payload: `onChange` / `onTransitionEnd` →
`{ index, previousIndex, item, source }`; `onTap` / `onVideoEnded` →
`{ index, item }`; `onEndReached` at the last item; `onError` → `{ code }`.
The ref (React `LxMediaSwiperRef`, Vue `LxMediaSwiperHandle`) has `next()`,
`previous()` and `goToIndex(index)`.

```tsx
<LxMediaSwiper
  items={[
    { type: 'image', src: 'https://cdn.example.com/a.jpg' },
    { type: 'video', src: 'https://cdn.example.com/b.mp4', controls: true },
  ]}
  loop
  dots
  onChange={({ index }) => actions.onSlideChange({ index })}
  onEndReached={actions.loadMore}
/>
```

## `LxNavigator`

Wraps content that opens a page, another lxapp, or a URL when tapped. `page` is
a page name. `target` is inferred (`app-id` → `lxapp`, `url` → `url`, else
`page`); `as` picks where, as in `lx.surface.openUrl` / `openPage`:

| `target` | `as` | Result | Extra attributes |
|---|---|---|---|
| `url` | `external` (default) | System browser | — |
| `url` | `tab` | In-app browser tab | — |
| `url` | `aside` | Browser docked beside the main | `edge`, `size` |
| `page` | *(omitted)* | Runs `open-type` in the page stack | — |
| `page` | `float` | Page float over the current surface | `position`, `size`, `interaction` |
| `page` | `window` | Separate desktop window (rejected on mobile) | `chrome`, `size`, `interaction` |
| `lxapp` | *(not allowed)* | Opens `app-id` (optional `page`, `query`, `channel`, `target-version`) | — |

`open-type` applies to `target="page"` without `as`; `navigateBack`, `exit`,
and `tel` work for any target:

| Value | Behavior |
|---|---|
| `navigate` (default) | Push a new page in the current lxapp |
| `redirect` | Replace the current page |
| `navigateBack` | Pop back by `delta`; with `target="lxapp"`, return from the opened lxapp |
| `reLaunch` | Restart the app at a new page: every page, cached tab pages included, is unloaded and the target opens fresh |
| `switchTab` | Switch to a tab page |
| `exit` | Exit the current lxapp |
| `tel` | Trigger a phone call (use with `phone-number`) |

`size` and `interaction` are objects in React/Vue (JSON strings on the raw
tag). No handle comes back; use `lx.surface.*` from Logic to message or close
a surface. Events `onSuccess` / `onFail` / `onComplete` carry
`{ success?, errMsg? }`.

```tsx
<LxNavigator page="detail" query={{ id: 42 }} onFail={actions.onNavFail}>
  <div>Open detail</div>
</LxNavigator>

<LxNavigator url="https://example.com" as="aside" edge="right">
  <div>Docs</div>
</LxNavigator>
```

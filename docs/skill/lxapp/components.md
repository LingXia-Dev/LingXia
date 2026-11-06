# Native components

Native-backed presenters: `LxPicker`, `LxMediaSwiper`, `LxNavigator`. Text input is plain `<input>` / `<textarea>`; there is no `LxInput`. In WebView pages, play video and audio with plain HTML `<video>` / `<audio>`.

Attribute and event types are exported from `@lingxia/elements`
(`LxPickerAttributes`, `LxMediaSwiperAttributes`,
`LxNavigatorAttributes`, `*EventDetail`).

## Import

```ts
// React
import { LxPicker, LxMediaSwiper, LxNavigator } from '@lingxia/react';
```

Wrappers take the element attributes plus `className` / `class` / `style` /
`ref`. An `actions.*` handler goes straight to Logic; a local function gets the
View event.

## Callback shapes

| Component | What the handler receives | Example |
|---|---|---|
| `LxPicker` | **Resolved value directly** — `string \| string[]` | `onConfirm(value)`, `onColumnChange(value)` |
| `LxMediaSwiper` | **Payload first** in React; HTML reads `CustomEvent.detail` | `onChange(({ index }) => …)` |
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
The ref (`LxMediaSwiperRef`) has `next()`,
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

`size` and `interaction` are objects in React (JSON strings on the raw
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

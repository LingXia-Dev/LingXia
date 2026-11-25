# WebView native elements

Native-backed presenters: `LxPicker`, `LxMediaSwiper`, `LxNavigator`. Text input is plain `<input>` / `<textarea>`; there is no `LxInput`. In WebView pages, play video and audio with plain HTML `<video>` / `<audio>`.

Attribute and event types are exported from `@lingxia/elements`
(`LxPickerAttributes`, `LxMediaSwiperAttributes`, `LxNavigatorAttributes`, the picker and
swiper `*EventDetail`s, `LxNavigatorEvent`).

## Import

```ts
// React
import { LxPicker, LxMediaSwiper, LxNavigator } from '@lingxia/react';
```

Wrappers take the element attributes plus `className` / `style` / `ref` (`class` is for
the raw `<lx-*>` tags). An `actions.*` handler goes straight to Logic; a local function gets the
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
`{ index, item }`; `onEndReached` at the last item; `onError` → `{ index, code, message }`
(`code`: `not_found`, `network`, `decode`, `unsupported_format`, `permission_denied`, `unknown`).
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

Wraps content that goes somewhere when tapped. `to` takes the targets of
`lx.nav.go`, plus `{ back: n }`:

| `to` | Result | Also |
|---|---|---|
| `{ page, params? }` | Push the page | `replace`, `reset`; `as="float"` (`position`) or `"window"` (`chrome`), with `size`, `interaction` |
| `{ tab }` | Switch to the tab | `reset` |
| `{ app, page?, params?, channel?, version? }` | Open another lxapp | — |
| `{ url }` | In-app browser, placed by the shell | `as="main"` or `"aside"` (`edge`, `size`); `as="external"` hands it to the system. `tel:` places a call |
| `{ back: n }` | Pop `n` pages | — |

`replace` swaps the current page; `reset` unloads every page, cached tabs
included, and opens the target fresh. On the raw tag `to`, `size` and
`interaction` are JSON strings. No handle comes back; use `lx.surface.*` from
Logic to message or close a surface. Events `onSuccess` / `onFail` /
`onComplete` carry `{ success?, errMsg? }`.

```tsx
<LxNavigator to={{ page: 'detail', params: { id: 42 } }} onFail={actions.onNavFail}>
  <div>Open detail</div>
</LxNavigator>

<LxNavigator to={{ url: 'https://example.com' }} as="aside" edge="right">
  <div>Docs</div>
</LxNavigator>
```

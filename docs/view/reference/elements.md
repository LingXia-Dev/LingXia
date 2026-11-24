_Generated from the View schema (`schema.json`) and the compiler; do not edit._

# Elements

Every intrinsic element of the View language: its children, props and binding.
Styles each element accepts are in [styles.md](styles.md).

## Props every element takes

| Prop | Type | Notes |
| --- | --- | --- |
| `className` | `string` |  |
| `key` | `string` |  |
| `"data-testid"` | `string` |  |
| `dir` | `"ltr" \| "rtl" \| "auto"` |  |
| `id` | `string` | Unique on the page (`UI_ID`); what `popoverTarget` and `aria-actions` name. Build one with `useId()`. |
| `role` | `"heading" \| "list" \| "listitem" \| "dialog" \| "alert" \| "status" \| "grid" \| "gridcell" \| "listbox" \| "option" \| "menu" \| "menuitem" \| "progressbar" \| "alertdialog" \| "group" \| "tooltip" \| "tablist" \| "tab" \| "tabpanel" \| "radiogroup" \| "radio" \| "toolbar" \| "tree" \| "treeitem" \| "row" \| "menuitemcheckbox" \| "menuitemradio" \| "img" \| "combobox" \| "spinbutton"` | Semantic role; controls keep their own. Headings take `headingLevel` 1..6; `alertdialog` goes on a `<dialog>` that interrupts. Composites move focus among their items with the arrow keys and take one Tab stop: `tablist` › `tab`, `radiogroup` › `radio`, `toolbar` › any control, `listbox` › `option`, `menu` › `menuitem`/`menuitemcheckbox`/`menuitemradio`, `tree` › `treeitem`. Item roles go on a `<button>`. `img` on a `<view>` or `<text>` with an `aria-label` speaks that label in place of its content. `dialog` goes on a popover `<view>` (with `aria-modal` for a modal one); a `<dialog>` has it already. |
| `headingLevel` | `1 \| 2 \| 3 \| 4 \| 5 \| 6` |  |
| `"aria-valuenow"` | `number` | A progressbar's value between `aria-valuemin` (0) and `aria-valuemax` (100); leave it out while the amount is unknown. `aria-valuetext` replaces the spoken percentage. On an `onPointer` element it is the value arrows and a screen reader step. |
| `"aria-valuemin"` | `number` |  |
| `"aria-valuemax"` | `number` |  |
| `"aria-valuetext"` | `string` |  |
| `"aria-activedescendant"` | `string` | On an `<input>` or `<textarea>`: the id of the option the editor's keys point at while focus stays in the text; a screen reader hears that option as it changes. |
| `"aria-controls"` | `string` | With `role="combobox"`: the ids (1..8, space-separated) of the list it opens, and `"list"` when typing filters that list. Assistive tech and automation read both; an id that names no element is left out. |
| `"aria-autocomplete"` | `"list" \| "none"` |  |
| `"aria-hidden"` | `boolean` | Removes the node and its subtree from accessibility. |
| `"aria-modal"` | `boolean` | On a popover with `role="dialog"`: modal like a `<dialog>`, which always is. Tab stays inside, the rest of the page leaves accessibility, and the page shows over the host's bars. Without it the popover dialog is non-modal. |
| `"aria-live"` | `"off" \| "polite" \| "assertive"` | Announce changes; `alert` is assertive and `status` polite unless set. |
| `"aria-pressed"` | `boolean` | States assistive input announces; leave one out when the element has no such state. |
| `"aria-selected"` | `boolean` |  |
| `"aria-expanded"` | `boolean` |  |
| `"aria-busy"` | `boolean` |  |
| `"aria-invalid"` | `boolean` | A field's error and required states; the error text belongs in `aria-describedby`. |
| `"aria-required"` | `boolean` |  |
| `"aria-checked"` | `boolean \| "mixed"` | Checked state of a `radio`, `menuitemcheckbox` or `menuitemradio` item; `"mixed"` on a `menuitemcheckbox` that is partly checked. |
| `"aria-level"` | `number` | A `treeitem`'s depth, 1..32; Left and Right walk between levels. |
| `"aria-describedby"` | `string` | Ids of elements (1..8, space-separated) whose text is read after the name, as a hint or error. |

`style` takes the element's own style subset; `className` takes complete Tailwind utilities.

## `<view>`

Children: any elements and text. Renders as `column`. `open`, `onToggle`, `closeAfter`, `anchor`, `onCloseRequest` need `popover`; `onCloseRequest` not with `popover="manual"`: tsc and the build refuse them otherwise (`UI_PROP`). `dragSnapPoints`, `bind`, `snapIndex`, `onSnapChange`, `onDragStart`, `onDragEnd`, `aria-actions` need `drag`: tsc and the build refuse them otherwise (`UI_PROP`). `pointerAxis` need `onPointer`: tsc and the build refuse them otherwise (`UI_PROP`). At most one of `bind` or `snapIndex` (and its `onSnapChange`) (`UI_PROP`).

| Prop | Type | Required |
| --- | --- | --- |
| `bleed` | `BleedEdge` |  |
| `onPage` | `(direction: "previous" \| "next") => void` |  |
| `onContextMenu` | `(event: ContextMenuEvent) => void` |  |
| `anchor` | `string` |  |
| `onPointer` | `(event: PointerEvent) => void` |  |
| `pointerAxis` | `"x" \| "y" \| "both"` |  |
| `onLayout` | `(box: BoxSize) => void` |  |
| `box` | `Box` |  |
| `aria-label` | `string` |  |
| `popover` | `boolean \| "auto" \| "manual" \| "hint"` |  |
| `open` | `boolean` |  |
| `onToggle` | `(open: boolean) => void` |  |
| `drag` | `"x" \| "y"` |  |
| `dragSnapPoints` | `` readonly (number \| `${number}%`)[] `` |  |
| `bind` | `Local<number>` |  |
| `snapIndex` | `number` |  |
| `onSnapChange` | `(index: number) => void` |  |
| `onDragStart` | `() => void` |  |
| `onDragEnd` | `(info: { index: number; velocity: number }) => void` |  |
| `aria-actions` | `string` |  |
| `onPointerEnter` | `() => void` |  |
| `onPointerLeave` | `() => void` |  |
| `closeAfter` | `number` |  |
| `onCloseRequest` | `() => boolean \| void` |  |

## `<page>`

Children: any elements and text. The page root.

| Prop | Type | Required |
| --- | --- | --- |
| `title` | `string` |  |
| `headerActions` | `Child` |  |
| `aria-label` | `string` |  |

## `<text>`

Children: text, `<span>` and `<link>` runs.

| Prop | Type | Required |
| --- | --- | --- |
| `onContextMenu` | `(event: ContextMenuEvent) => void` |  |
| `onLayout` | `(box: BoxSize) => void` |  |
| `box` | `Box` |  |
| `aria-label` | `string` |  |

## `<button>`

Children: text, or noninteractive content elements (a composed control). `dragSnapPoints`, `bind`, `snapIndex`, `onSnapChange`, `onDragStart`, `onDragEnd`, `aria-actions` need `drag`: tsc and the build refuse them otherwise (`UI_PROP`). `pointerAxis` need `onPointer`: tsc and the build refuse them otherwise (`UI_PROP`). At most one of `bind` or `snapIndex` (and its `onSnapChange`) (`UI_PROP`).

| Prop | Type | Required |
| --- | --- | --- |
| `onContextMenu` | `(event: ContextMenuEvent) => void` |  |
| `onPointer` | `(event: PointerEvent) => void` |  |
| `pointerAxis` | `"x" \| "y" \| "both"` |  |
| `onClick` | `() => void` |  |
| `disabled` | `boolean` |  |
| `aria-label` | `string` |  |
| `popoverTarget` | `string` |  |
| `interestFor` | `string` |  |
| `keepOpen` | `boolean` |  |
| `drag` | `"x" \| "y"` |  |
| `dragSnapPoints` | `` readonly (number \| `${number}%`)[] `` |  |
| `bind` | `Local<number>` |  |
| `snapIndex` | `number` |  |
| `onSnapChange` | `(index: number) => void` |  |
| `onDragStart` | `() => void` |  |
| `onDragEnd` | `(info: { index: number; velocity: number }) => void` |  |
| `aria-actions` | `string` |  |
| `onPointerEnter` | `() => void` |  |
| `onPointerLeave` | `() => void` |  |

## `<scroll>`

Children: any elements and text. At most one of `bind` or `snapIndex` (and its `onSnapChange`) (`UI_PROP`). Not allowed inside a pressable control (`UI_BUTTON`).

| Prop | Type | Required |
| --- | --- | --- |
| `bleed` | `BleedEdge` |  |
| `onLayout` | `(box: BoxSize) => void` |  |
| `box` | `Box` |  |
| `onReachStart` | `() => void` |  |
| `onReachEnd` | `() => void` |  |
| `onScrollEdge` | `(event: ScrollEdgeEvent) => void` |  |
| `bind` | `Local<number>` |  |
| `snapIndex` | `number` |  |
| `onSnapChange` | `(index: number) => void` |  |

`bind` and `snapIndex` are the index of the snap point a `scrollSnapType` scroll rests on, not its offset: `bind={local}` with a number `useLocal` cell, or `snapIndex={n}` with `onSnapChange(index)` to follow it from the page's or Logic's state. Not both (`UI_PROP`). After a swipe the element waits for the page's answer; if the page keeps its index, the element returns to it. The answer may come through a Logic action `onSnapChange` starts; the wait ends at 400 ms. Only actions the handler calls directly count: one started after an `await` or in a timer is not waited for, and the page is taken to keep its index after the next frame. The same holds on a dragged element.

## `<List>`

Children: none. Not allowed inside a pressable control (`UI_BUTTON`).

A component from `@lingxia/view`, rendered as the `list` element.

| Prop | Type | Required |
| --- | --- | --- |
| `items` | `readonly T[]` | yes |
| `getKey` | `(item: T, index: number) => string` | yes |
| `renderItem` | `(item: T, index: number) => JSX.Element` | yes |
| `getItemHeight` | `(item: T, index: number) => number` |  |
| `bleed` | `BleedEdge` |  |
| `anchor` | `"end" \| { index: number; align: "start" \| "end"; seq?: number; duration?: number }` |  |
| `onReachStart` | `() => void` |  |
| `onReachEnd` | `() => void` |  |
| `onScrollEdge` | `(event: ScrollEdgeEvent) => void` |  |
| `aria-label` | `string` |  |

## `<image>`

Children: none.

| Prop | Type | Required |
| --- | --- | --- |
| `bleed` | `BleedEdge` |  |
| `src` | `string` | yes |
| `placeholder` | `string` |  |
| `alt` | `string` |  |
| `retryKey` | `string` |  |
| `onLoad` | `() => void` |  |
| `onError` | `() => void` |  |

## `<input>`

Children: none.

Binding: `bind={local}` with a `useLocal<string>` cell, or `value` plus `onInput(value: string)`; not both. `onInput` runs on every value change, a composition included.

| Prop | Type | Required |
| --- | --- | --- |
| `onKeyDown` | `EditorKeys` |  |
| `readOnly` | `boolean` |  |
| `onFocus` | `() => void` |  |
| `onBlur` | `() => void` |  |
| `aria-label` | `string` |  |
| `placeholder` | `string` |  |
| `type` | `"text" \| "password" \| "email" \| "number" \| "tel" \| "search" \| "date" \| "time"` |  |
| `inputMode` | `"text" \| "numeric" \| "decimal" \| "tel" \| "email"` |  |
| `autoFocus` | `boolean` |  |
| `disabled` | `boolean` |  |
| `onSubmit` | `(value: string) => void` |  |
| `maxLength` | `number` |  |
| `autoComplete` | `AutoComplete` |  |
| `enterKeyHint` | `"enter" \| "done" \| "go" \| "next" \| "previous" \| "search" \| "send"` |  |
| `autoCapitalize` | `"none" \| "sentences" \| "words" \| "characters" \| "off" \| "on"` |  |
| `spellCheck` | `boolean` |  |
| `min` | `number \| string` |  |
| `max` | `number \| string` |  |
| `step` | `number` |  |

## `<textarea>`

Children: none.

Binding: `bind={local}` with a `useLocal<string>` cell, or `value` plus `onInput(value: string)`; not both. `onInput` runs on every value change, a composition included.

| Prop | Type | Required |
| --- | --- | --- |
| `onKeyDown` | `EditorKeys` |  |
| `readOnly` | `boolean` |  |
| `onFocus` | `() => void` |  |
| `onBlur` | `() => void` |  |
| `rows` | `number` |  |
| `maxRows` | `number` |  |
| `aria-label` | `string` |  |
| `placeholder` | `string` |  |
| `inputMode` | `"text" \| "numeric" \| "decimal" \| "tel" \| "email"` |  |
| `autoFocus` | `boolean` |  |
| `disabled` | `boolean` |  |
| `onSubmit` | `(value: string) => void` |  |
| `maxLength` | `number` |  |
| `autoComplete` | `AutoComplete` |  |
| `enterKeyHint` | `"enter" \| "done" \| "go" \| "next" \| "previous" \| "search" \| "send"` |  |
| `autoCapitalize` | `"none" \| "sentences" \| "words" \| "characters" \| "off" \| "on"` |  |
| `spellCheck` | `boolean` |  |

## `<checkbox>`

Children: text, or noninteractive content elements (a composed control).

Binding: `bind={local}` with a `useLocal<boolean>` cell, or `value` plus `onChange(value: boolean)`; not both.

| Prop | Type | Required |
| --- | --- | --- |
| `indeterminate` | `boolean` |  |
| `defaultChecked` | `boolean` |  |
| `aria-label` | `string` |  |
| `disabled` | `boolean` |  |

`defaultChecked`: where a checkbox without `value` or `bind` starts; it applies once, and a later change does nothing.

## `<switch>`

Children: text, or noninteractive content elements (a composed control).

Binding: `bind={local}` with a `useLocal<boolean>` cell, or `value` plus `onChange(value: boolean)`; not both.

| Prop | Type | Required |
| --- | --- | --- |
| `defaultChecked` | `boolean` |  |
| `aria-label` | `string` |  |
| `disabled` | `boolean` |  |

`defaultChecked`: where a switch without `value` or `bind` starts; it applies once, and a later change does nothing.

## `<radio>`

Children: none.

Binding: `bind={local}` with a `useLocal<string>` cell, or `value` plus `onChange(value: string)`; not both.

| Prop | Type | Required |
| --- | --- | --- |
| `options` | `readonly ChoiceOption[]` | yes |
| `aria-label` | `string` |  |
| `disabled` | `boolean` |  |

## `<slider>`

Children: none.

Binding: `bind={local}` with a `useLocal<number>` cell, or `value` plus `onChange(value: number)`; not both.

`onChangeEnd(value)` runs once a change is finished: when the press that made it lifts, or at once for a key or assistive step.

Two values: `values={[lower, upper]}` with `thumbLabels={[name, name]}` makes a control with two thumbs that cannot cross; `onChange` and `onChangeEnd` then take `[lower, upper]`.

| Prop | Type | Required |
| --- | --- | --- |
| `min` | `number` |  |
| `max` | `number` |  |
| `step` | `number` |  |
| `defaultValue` | `number` |  |
| `secondaryValue` | `number` |  |
| `aria-label` | `string` | yes |
| `aria-valuetext` | `string` |  |
| `disabled` | `boolean` |  |
| `onPressChange` | `(pressed: boolean) => void` |  |

`defaultValue`: where a slider without `value` or `bind` starts, snapped to `step`; it applies once, and a later change does nothing.

## `<dialog>`

Children: any elements and text. Renders as `column`. `dragSnapPoints`, `bind`, `snapIndex`, `onSnapChange`, `onDragStart`, `onDragEnd` need `drag`: tsc and the build refuse them otherwise (`UI_PROP`). At most one of `bind` or `snapIndex` (and its `onSnapChange`) (`UI_PROP`).

| Prop | Type | Required |
| --- | --- | --- |
| `open` | `boolean` |  |
| `onClose` | `() => void` |  |
| `closedby` | `"any" \| "closerequest" \| "none"` |  |
| `aria-label` | `string` | yes |
| `drag` | `"x" \| "y"` |  |
| `dragSnapPoints` | `` readonly (number \| `${number}%`)[] `` |  |
| `bind` | `Local<number>` |  |
| `snapIndex` | `number` |  |
| `onSnapChange` | `(index: number) => void` |  |
| `onDragStart` | `() => void` |  |
| `onDragEnd` | `(info: { index: number; velocity: number }) => void` |  |

## `<plot>`

Children: none.

| Prop | Type | Required |
| --- | --- | --- |
| `onLayout` | `(box: BoxSize) => void` |  |
| `box` | `Box` |  |
| `kind` | `"bar" \| "line" \| "pie"` | yes |
| `values` | `readonly number[]` | yes |
| `colors` | `readonly ColorSeries[]` | yes |
| `aria-label` | `string` | yes |
| `smooth` | `boolean` |  |
| `area` | `boolean` |  |
| `selectedIndex` | `number` |  |
| `domainMin` | `number` |  |
| `domainMax` | `number` |  |
| `onSelect` | `(index: number) => void` |  |
| `pieLayout` | `PieLayout` |  |
| `markerEmphasisScale` | `number` |  |

## `<vector>`

Children: none. `pointerAxis` need `onPointer`: tsc and the build refuse them otherwise (`UI_PROP`).

| Prop | Type | Required |
| --- | --- | --- |
| `bleed` | `BleedEdge` |  |
| `onPointer` | `(event: PointerEvent) => void` |  |
| `pointerAxis` | `"x" \| "y" \| "both"` |  |
| `onLayout` | `(box: BoxSize) => void` |  |
| `box` | `Box` |  |
| `paths` | `ReadonlyArray<VectorPath>` | yes |
| `gradients` | `Readonly<Record<string, VectorGradient>>` |  |
| `viewBox` | `readonly [number, number, number, number]` |  |

## `<span>`

Children: text, `<span>` and `<link>` runs. Only inside `<text>`. It takes, of the props every element takes, only `style`, `className`, `key`, `data-testid`, `dir`, `aria-hidden`; the rest go on its `<text>` (`UI_PROP`). Its `data-testid` finds the run itself: its rect is the run's own words, so a tap lands on them.

No element-specific props.

## `<link>`

Inside `<text>`, a run: children text and `<span>` runs; links do not nest. Elsewhere, a block link (`link-box`): a pressable box whose children are text, or noninteractive content elements (a composed control), styled like `<button>`; a block link never sits inside another (`UI_LINK`). As a run it takes, of the props every element takes, only `style`, `className`, `key`, `data-testid`, `dir`, `aria-describedby`; the rest go on its `<text>` (`UI_PROP`). Its `data-testid` finds the run itself: its rect is the run's own words, so a tap lands on them. Needs `to`, `onClick`, `popoverTarget` or `interestFor`, disabled or not: `disabled` only stops the press (`UI_LINK`). Enter activates it; Space does not.

| Prop | Type | Required |
| --- | --- | --- |
| `onContextMenu` | `(event: ContextMenuEvent) => void` |  |
| `onClick` | `() => void` |  |
| `to` | `LinkTarget` |  |
| `aria-label` | `string` |  |
| `disabled` | `boolean` |  |
| `popoverTarget` | `string` |  |
| `interestFor` | `string` |  |

## `<select>`

Children: none.

Binding: `bind={local}` with a `useLocal<string>` cell, or `value` plus `onChange(value: string)`; not both.

| Prop | Type | Required |
| --- | --- | --- |
| `options` | `readonly ChoiceOption[]` | yes |
| `defaultValue` | `string` |  |
| `placeholder` | `string` |  |
| `aria-label` | `string` |  |
| `disabled` | `boolean` |  |

`defaultValue`: the option a select without `value` or `bind` starts on; it applies once, and a later change does nothing.

## `<video>`

Children: none. At most one of `src` or `sources` (`UI_PROP`).

| Prop | Type | Required |
| --- | --- | --- |
| `bleed` | `BleedEdge` |  |
| `src` | `string` |  |
| `sources` | `readonly VideoSource[]` |  |
| `player` | `Video` |  |
| `poster` | `string` |  |
| `autoPlay` | `boolean` |  |
| `paused` | `boolean` |  |
| `muted` | `boolean` |  |
| `loop` | `boolean` |  |
| `playbackRate` | `number` |  |
| `volume` | `number` |  |
| `aria-label` | `string` |  |
| `onLoadedMetadata` | `(event: VideoMetadata) => void` |  |
| `onPlay` | `() => void` |  |
| `onPlaying` | `() => void` |  |
| `onPause` | `(event: VideoPause) => void` |  |
| `onWaiting` | `() => void` |  |
| `onSeeked` | `(event: { readonly currentTime: number }) => void` |  |
| `onTimeUpdate` | `(event: { readonly currentTime: number }) => void` |  |
| `onEnded` | `() => void` |  |
| `onError` | `(error: VideoError) => void` |  |
| `onRateChange` | `(event: { readonly playbackRate: number }) => void` |  |
| `onVolumeChange` | `(event: { readonly volume: number; readonly muted: boolean }) => void` |  |
| `onQualityChange` | `(event: VideoQualityChange) => void` |  |
| `onFullscreenChange` | `(event: { readonly fullscreen: boolean }) => void` |  |

`src` plays one file; `sources` offers the same video at several qualities; not both (`UI_PROP`). Props apply when they change: a new `src` or `sources` starts over from the beginning, showing the last frame until the new first one; `paused`, `muted`, `loop`, `playbackRate` and `volume`: each applies on mount and when its value changes; writing the same value again does nothing, so a handle's `play()` or `pause()` stands until that prop itself changes. To act, use the player handle. A named player is the advanced case: `player={useVideo(name)}` on the `<video>` that gives the source makes it the page's player, and another `<video player>` with no source, such as a full-screen one, shows that same player. A component that forwards a source types it with `VideoSourceProps` and spreads it whole.

## `<webview>`

Children: none. Not allowed inside a pressable control (`UI_BUTTON`). Moves live with a translation. Under opacity, a mask, a blur, a scale or a rotation it shows a still picture of the page and takes no input until the effect ends. Scaling or rotating it outside entering and leaving fails the build (`UI_TRANSFORM`); a scale or rotation applied at run time shows the still picture. Its page's `prefers-color-scheme` follows the View page's appearance, live.

| Prop | Type | Required |
| --- | --- | --- |
| `bleed` | `BleedEdge` |  |
| `src` | `string` |  |
| `web` | `WebView` |  |
| `aria-label` | `string` | yes |
| `handlesDrags` | `boolean` |  |
| `onFullscreenChange` | `(event: { readonly fullscreen: boolean }) => void` |  |
| `onLoad` | `(event: WebViewLoad) => void` |  |
| `onMessage` | `(message: WebViewMessage) => void` |  |
| `onBlocked` | `(event: WebViewBlocked) => void` |  |
| `onError` | `(error: WebViewError) => void` |  |

`handlesDrags`: the web page handles its own drags (a map, a canvas); a drag starting on it never passes to the page. Off by default. When the screen takes a drag the page had started, the page gets `pointercancel` and `touchcancel`; on HarmonyOS they are synthetic (`isTrusted` false), and the page's own overflow scrolling, fixed when the finger lands, keeps scrolling.

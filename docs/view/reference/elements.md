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
| `role` | `"heading" \| "list" \| "listitem" \| "alert" \| "status" \| "grid" \| "gridcell" \| "listbox" \| "option" \| "menu" \| "menuitem" \| "progressbar" \| "alertdialog" \| "group" \| "tooltip" \| "tablist" \| "tab" \| "tabpanel" \| "radiogroup" \| "radio" \| "toolbar" \| "tree" \| "treeitem" \| "row" \| "menuitemcheckbox" \| "menuitemradio"` | Semantic role; controls keep their own. Headings take `headingLevel` 1..6; `alertdialog` goes on a `<dialog>` that interrupts. Composites move focus among their items with the arrow keys and take one Tab stop: `tablist` › `tab`, `radiogroup` › `radio`, `toolbar` › any control, `listbox` › `option`, `menu` › `menuitem`/`menuitemcheckbox`/`menuitemradio`, `tree` › `treeitem`. Item roles go on a `<button>`. |
| `headingLevel` | `1 \| 2 \| 3 \| 4 \| 5 \| 6` |  |
| `"aria-valuenow"` | `number` | A progressbar's value between `aria-valuemin` (0) and `aria-valuemax` (100); leave it out while the amount is unknown. `aria-valuetext` replaces the spoken percentage. |
| `"aria-valuemin"` | `number` |  |
| `"aria-valuemax"` | `number` |  |
| `"aria-valuetext"` | `string` |  |
| `"aria-hidden"` | `boolean` | Removes the node and its subtree from accessibility. |
| `"aria-live"` | `"off" \| "polite" \| "assertive"` | Announce changes; `alert` is assertive and `status` polite unless set. |
| `"aria-pressed"` | `boolean` | States assistive input announces; leave one out when the element has no such state. |
| `"aria-selected"` | `boolean` |  |
| `"aria-expanded"` | `boolean` |  |
| `"aria-busy"` | `boolean` |  |
| `"aria-invalid"` | `boolean` | A field's error and required states; the error text belongs in `aria-describedby`. |
| `"aria-required"` | `boolean` |  |
| `"aria-checked"` | `boolean` | Checked state of a `radio`, `menuitemcheckbox` or `menuitemradio` item. |
| `"aria-level"` | `number` | A `treeitem`'s depth, 1..32; Left and Right walk between levels. |
| `"aria-describedby"` | `string` | Ids of elements (1..8, space-separated) whose text is read after the name, as a hint or error. |

`style` takes the element's own style subset; `className` takes complete Tailwind utilities.

## `<view>`

Children: any elements and text. Renders as `column`.

| Prop | Type | Required |
| --- | --- | --- |
| `aria-label` | `string` |  |
| `popover` | `boolean \| "auto" \| "manual"` |  |
| `open` | `boolean` |  |
| `onToggle` | `(open: boolean) => void` |  |
| `drag` | `"x" \| "y"` |  |
| `dragSnapPoints` | `` readonly (number \| `${number}%`)[] `` |  |
| `bind` | `Local<number>` |  |
| `onDragStart` | `() => void` |  |
| `onDragEnd` | `(info: { index: number; velocity: number }) => void` |  |
| `aria-actions` | `string` |  |

## `<page>`

Children: any elements and text. The page root.

| Prop | Type | Required |
| --- | --- | --- |
| `aria-label` | `string` |  |

## `<text>`

Children: text, `<span>` and `<link>` runs.

| Prop | Type | Required |
| --- | --- | --- |
| `aria-label` | `string` |  |

## `<button>`

Children: text, or noninteractive content elements (a composed control).

| Prop | Type | Required |
| --- | --- | --- |
| `onClick` | `() => void` |  |
| `disabled` | `boolean` |  |
| `aria-label` | `string` |  |
| `popoverTarget` | `string` |  |
| `drag` | `"x" \| "y"` |  |
| `dragSnapPoints` | `` readonly (number \| `${number}%`)[] `` |  |
| `bind` | `Local<number>` |  |
| `onDragStart` | `() => void` |  |
| `onDragEnd` | `(info: { index: number; velocity: number }) => void` |  |
| `aria-actions` | `string` |  |

## `<scroll>`

Children: any elements and text. Not allowed inside a pressable control (`UI_BUTTON`).

| Prop | Type | Required |
| --- | --- | --- |
| `onReachStart` | `() => void` |  |
| `onReachEnd` | `() => void` |  |
| `bind` | `Local<number>` |  |

## `<List>`

Children: none. Not allowed inside a pressable control (`UI_BUTTON`).

A component from `@lingxia/view`, rendered as the `list` element.

| Prop | Type | Required |
| --- | --- | --- |
| `items` | `readonly T[]` | yes |
| `getKey` | `(item: T, index: number) => string` | yes |
| `renderItem` | `(item: T, index: number) => JSX.Element` | yes |
| `getItemHeight` | `(item: T, index: number) => number` |  |
| `anchor` | `"end" \| { index: number; align: "start" \| "end"; seq?: number; duration?: number }` |  |
| `onReachStart` | `() => void` |  |
| `onReachEnd` | `() => void` |  |

## `<image>`

Children: none.

| Prop | Type | Required |
| --- | --- | --- |
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
| `aria-label` | `string` |  |
| `disabled` | `boolean` |  |

## `<switch>`

Children: text only.

Binding: `bind={local}` with a `useLocal<boolean>` cell, or `value` plus `onChange(value: boolean)`; not both.

| Prop | Type | Required |
| --- | --- | --- |
| `aria-label` | `string` |  |
| `disabled` | `boolean` |  |

## `<radio>`

Children: none.

Binding: `bind={local}` with a `useLocal<string>` cell, or `value` plus `onChange(value: string)`; not both.

| Prop | Type | Required |
| --- | --- | --- |
| `options` | `readonly RadioOption[]` | yes |
| `aria-label` | `string` |  |
| `disabled` | `boolean` |  |

## `<slider>`

Children: none.

Binding: `bind={local}` with a `useLocal<number>` cell, or `value` plus `onChange(value: number)`; not both.

| Prop | Type | Required |
| --- | --- | --- |
| `min` | `number` |  |
| `max` | `number` |  |
| `step` | `number` |  |
| `secondaryValue` | `number` |  |
| `aria-label` | `string` | yes |
| `aria-valuetext` | `string` |  |
| `disabled` | `boolean` |  |

## `<dialog>`

Children: any elements and text. Renders as `column`.

| Prop | Type | Required |
| --- | --- | --- |
| `open` | `boolean` |  |
| `onClose` | `() => void` |  |
| `closedby` | `"any" \| "closerequest" \| "none"` |  |
| `aria-label` | `string` | yes |
| `drag` | `"x" \| "y"` |  |
| `dragSnapPoints` | `` readonly (number \| `${number}%`)[] `` |  |
| `bind` | `Local<number>` |  |
| `onDragStart` | `() => void` |  |
| `onDragEnd` | `(info: { index: number; velocity: number }) => void` |  |

## `<plot>`

Children: none.

| Prop | Type | Required |
| --- | --- | --- |
| `kind` | `"bar" \| "line" \| "pie"` | yes |
| `values` | `readonly number[]` | yes |
| `colors` | `readonly string[]` | yes |
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

Children: none.

| Prop | Type | Required |
| --- | --- | --- |
| `paths` | `ReadonlyArray<VectorPath>` | yes |
| `gradients` | `Readonly<Record<string, VectorGradient>>` |  |
| `viewBox` | `readonly [number, number, number, number]` |  |

## `<span>`

Children: text, `<span>` and `<link>` runs. Only inside `<text>`.

No element-specific props.

## `<link>`

Children: text and `<span>` runs; links do not nest. Only inside `<text>`.

| Prop | Type | Required |
| --- | --- | --- |
| `onClick` | `() => void` |  |
| `aria-label` | `string` |  |
| `disabled` | `boolean` |  |

## `<select>`

Children: none.

Binding: `bind={local}` with a `useLocal<string>` cell, or `value` plus `onChange(value: string)`; not both.

| Prop | Type | Required |
| --- | --- | --- |
| `options` | `readonly RadioOption[]` | yes |
| `placeholder` | `string` |  |
| `aria-label` | `string` |  |
| `disabled` | `boolean` |  |

## `<video>`

Children: none.

| Prop | Type | Required |
| --- | --- | --- |
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

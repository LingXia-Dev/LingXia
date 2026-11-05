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
| `role` | `"heading" \| "list" \| "listitem" \| "alert" \| "status" \| "menu" \| "menuitem"` | Semantic role; controls keep their own. Headings take `headingLevel` 1..6. |
| `headingLevel` | `1 \| 2 \| 3 \| 4 \| 5 \| 6` |  |
| `"aria-hidden"` | `boolean` | Removes the node and its subtree from accessibility. |
| `"aria-live"` | `"off" \| "polite" \| "assertive"` | Announce changes; `alert` is assertive and `status` polite unless set. |
| `"aria-pressed"` | `boolean` | States assistive input announces; leave one out when the element has no such state. |
| `"aria-selected"` | `boolean` |  |
| `"aria-expanded"` | `boolean` |  |
| `"aria-busy"` | `boolean` |  |

`style` takes the element's own style subset; `className` takes complete Tailwind utilities.

## `<view>`

Children: any elements and text. Renders as `column`. May sit inside a composed `<button>` or `<checkbox>`.

| Prop | Type | Required |
| --- | --- | --- |
| `aria-label` | `string` |  |
| `id` | `string` |  |
| `popover` | `boolean \| "auto" \| "manual"` |  |
| `open` | `boolean` |  |
| `onToggle` | `(open: boolean) => void` |  |
| `drag` | `"x" \| "y"` |  |
| `dragSnapPoints` | `` readonly (number \| `${number}%`)[] `` |  |
| `bind` | `Local<number>` |  |
| `onDragStart` | `() => void` |  |
| `onDragEnd` | `(info: { index: number; velocity: number }) => void` |  |
| `aria-actions` | `string` |  |

## `<column>`

Children: any elements and text. May sit inside a composed `<button>` or `<checkbox>`.

| Prop | Type | Required |
| --- | --- | --- |
| `aria-label` | `string` |  |
| `id` | `string` |  |
| `popover` | `boolean \| "auto" \| "manual"` |  |
| `open` | `boolean` |  |
| `onToggle` | `(open: boolean) => void` |  |
| `drag` | `"x" \| "y"` |  |
| `dragSnapPoints` | `` readonly (number \| `${number}%`)[] `` |  |
| `bind` | `Local<number>` |  |
| `onDragStart` | `() => void` |  |
| `onDragEnd` | `(info: { index: number; velocity: number }) => void` |  |
| `aria-actions` | `string` |  |

## `<row>`

Children: any elements and text. May sit inside a composed `<button>` or `<checkbox>`.

| Prop | Type | Required |
| --- | --- | --- |
| `aria-label` | `string` |  |
| `id` | `string` |  |
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

Children: any elements and text. The page root. Not allowed inside a composed `<button>` or `<checkbox>`.

| Prop | Type | Required |
| --- | --- | --- |
| `aria-label` | `string` |  |

## `<text>`

Children: text, `<span>` and `<link>` runs. May sit inside a composed `<button>` or `<checkbox>`.

| Prop | Type | Required |
| --- | --- | --- |
| `aria-label` | `string` |  |

## `<button>`

Children: text, or noninteractive content elements (a composed control). Not allowed inside a composed `<button>` or `<checkbox>`.

| Prop | Type | Required |
| --- | --- | --- |
| `onClick` | `() => void` |  |
| `disabled` | `boolean` |  |
| `aria-label` | `string` |  |
| `popoverTarget` | `string` |  |
| `id` | `string` |  |
| `drag` | `"x" \| "y"` |  |
| `dragSnapPoints` | `` readonly (number \| `${number}%`)[] `` |  |
| `bind` | `Local<number>` |  |
| `onDragStart` | `() => void` |  |
| `onDragEnd` | `(info: { index: number; velocity: number }) => void` |  |
| `aria-actions` | `string` |  |

## `<spacer>`

Children: none. May sit inside a composed `<button>` or `<checkbox>`.

No element-specific props.

## `<scroll>`

Children: any elements and text. Not allowed inside a composed `<button>` or `<checkbox>`.

| Prop | Type | Required |
| --- | --- | --- |
| `onReachStart` | `() => void` |  |
| `onReachEnd` | `() => void` |  |
| `bind` | `Local<number>` |  |

## `<List>`

Children: none. Not allowed inside a composed `<button>` or `<checkbox>`.

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

Children: none. May sit inside a composed `<button>` or `<checkbox>`.

| Prop | Type | Required |
| --- | --- | --- |
| `src` | `string` | yes |
| `placeholder` | `string` |  |
| `alt` | `string` |  |
| `retryKey` | `string` |  |
| `onLoad` | `() => void` |  |
| `onError` | `() => void` |  |

## `<input>`

Children: none. Not allowed inside a composed `<button>` or `<checkbox>`.

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

Children: none. Not allowed inside a composed `<button>` or `<checkbox>`.

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

Children: text, or noninteractive content elements (a composed control). Not allowed inside a composed `<button>` or `<checkbox>`.

Binding: `bind={local}` with a `useLocal<boolean>` cell, or `value` plus `onChange(value: boolean)`; not both.

| Prop | Type | Required |
| --- | --- | --- |
| `aria-label` | `string` |  |
| `disabled` | `boolean` |  |

## `<switch>`

Children: text only. Not allowed inside a composed `<button>` or `<checkbox>`.

Binding: `bind={local}` with a `useLocal<boolean>` cell, or `value` plus `onChange(value: boolean)`; not both.

| Prop | Type | Required |
| --- | --- | --- |
| `aria-label` | `string` |  |
| `disabled` | `boolean` |  |

## `<radio>`

Children: none. Not allowed inside a composed `<button>` or `<checkbox>`.

Binding: `bind={local}` with a `useLocal<string>` cell, or `value` plus `onChange(value: string)`; not both.

| Prop | Type | Required |
| --- | --- | --- |
| `options` | `readonly RadioOption[]` | yes |
| `aria-label` | `string` |  |
| `disabled` | `boolean` |  |

## `<slider>`

Children: none. Not allowed inside a composed `<button>` or `<checkbox>`.

Binding: `bind={local}` with a `useLocal<number>` cell, or `value` plus `onChange(value: number)`; not both.

| Prop | Type | Required |
| --- | --- | --- |
| `min` | `number` |  |
| `max` | `number` |  |
| `step` | `number` |  |
| `aria-label` | `string` | yes |
| `disabled` | `boolean` |  |

## `<grid>`

Children: any elements and text. May sit inside a composed `<button>` or `<checkbox>`.

| Prop | Type | Required |
| --- | --- | --- |
| `id` | `string` |  |
| `popover` | `boolean \| "auto" \| "manual"` |  |
| `open` | `boolean` |  |
| `onToggle` | `(open: boolean) => void` |  |
| `drag` | `"x" \| "y"` |  |
| `dragSnapPoints` | `` readonly (number \| `${number}%`)[] `` |  |
| `bind` | `Local<number>` |  |
| `onDragStart` | `() => void` |  |
| `onDragEnd` | `(info: { index: number; velocity: number }) => void` |  |
| `aria-actions` | `string` |  |

## `<stack>`

Children: any elements and text. May sit inside a composed `<button>` or `<checkbox>`.

| Prop | Type | Required |
| --- | --- | --- |
| `id` | `string` |  |
| `popover` | `boolean \| "auto" \| "manual"` |  |
| `open` | `boolean` |  |
| `onToggle` | `(open: boolean) => void` |  |
| `drag` | `"x" \| "y"` |  |
| `dragSnapPoints` | `` readonly (number \| `${number}%`)[] `` |  |
| `bind` | `Local<number>` |  |
| `onDragStart` | `() => void` |  |
| `onDragEnd` | `(info: { index: number; velocity: number }) => void` |  |
| `aria-actions` | `string` |  |

## `<dialog>`

Children: any elements and text. Renders as `column`. May sit inside a composed `<button>` or `<checkbox>`.

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

Children: none. Not allowed inside a composed `<button>` or `<checkbox>`.

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

Children: none. May sit inside a composed `<button>` or `<checkbox>`.

| Prop | Type | Required |
| --- | --- | --- |
| `paths` | `ReadonlyArray<VectorPath>` | yes |
| `gradients` | `Readonly<Record<string, VectorGradient>>` |  |
| `viewBox` | `readonly [number, number, number, number]` |  |

## `<span>`

Children: text, `<span>` and `<link>` runs. Only inside `<text>`. May sit inside a composed `<button>` or `<checkbox>`.

No element-specific props.

## `<link>`

Children: text and `<span>` runs; links do not nest. Only inside `<text>`. Not allowed inside a composed `<button>` or `<checkbox>`.

| Prop | Type | Required |
| --- | --- | --- |
| `onClick` | `() => void` |  |
| `aria-label` | `string` |  |
| `disabled` | `boolean` |  |

## `<select>`

Children: none. Not allowed inside a composed `<button>` or `<checkbox>`.

Binding: `bind={local}` with a `useLocal<string>` cell, or `value` plus `onChange(value: string)`; not both.

| Prop | Type | Required |
| --- | --- | --- |
| `options` | `readonly RadioOption[]` | yes |
| `placeholder` | `string` |  |
| `aria-label` | `string` |  |
| `disabled` | `boolean` |  |

_Generated from the View schema (`schema.json`) and the compiler; do not edit._

# Elements

Every intrinsic element of the View language: its children, props and binding.
Styles each element accepts are in [styles.md](styles.md).

## Props every element takes

| Prop | Type | Notes |
| --- | --- | --- |
| `className` | `string` |  |
| `key` | `string` |  |
| `testId` | `string` |  |
| `dir` | `"ltr" \| "rtl" \| "auto"` |  |
| `role` | `"heading" \| "list" \| "listitem" \| "dialog" \| "alert" \| "status"` | Semantic role; controls keep their own. Headings take `headingLevel` 1..6. |
| `headingLevel` | `1 \| 2 \| 3 \| 4 \| 5 \| 6` |  |
| `accessibilityHidden` | `boolean` | Removes the node and its subtree from accessibility. |
| `accessibilityLiveRegion` | `"off" \| "polite" \| "assertive"` | Announce changes; `alert` is assertive and `status` polite unless set. |

`style` takes the element's own style subset; `className` takes complete Tailwind utilities.

## `<view>`

Children: any elements and text. Renders as `column`. May sit inside a composed `<button>` or `<checkbox>`.

| Prop | Type | Required |
| --- | --- | --- |
| `accessibilityLabel` | `string` |  |

## `<column>`

Children: any elements and text. May sit inside a composed `<button>` or `<checkbox>`.

| Prop | Type | Required |
| --- | --- | --- |
| `accessibilityLabel` | `string` |  |

## `<row>`

Children: any elements and text. May sit inside a composed `<button>` or `<checkbox>`.

| Prop | Type | Required |
| --- | --- | --- |
| `accessibilityLabel` | `string` |  |

## `<page>`

Children: any elements and text. The page root. Not allowed inside a composed `<button>` or `<checkbox>`.

| Prop | Type | Required |
| --- | --- | --- |
| `accessibilityLabel` | `string` |  |

## `<text>`

Children: text, `<span>` and `<link>` runs. May sit inside a composed `<button>` or `<checkbox>`.

| Prop | Type | Required |
| --- | --- | --- |
| `accessibilityLabel` | `string` |  |

## `<button>`

Children: text, or noninteractive content elements (a composed control). Not allowed inside a composed `<button>` or `<checkbox>`.

| Prop | Type | Required |
| --- | --- | --- |
| `onClick` | `() => void` |  |
| `disabled` | `boolean` |  |
| `accessibilityLabel` | `string` |  |

## `<spacer>`

Children: none. May sit inside a composed `<button>` or `<checkbox>`.

No element-specific props.

## `<scroll>`

Children: any elements and text. Not allowed inside a composed `<button>` or `<checkbox>`.

| Prop | Type | Required |
| --- | --- | --- |
| `onReachStart` | `() => void` |  |
| `onReachEnd` | `() => void` |  |

## `<list>`

Children: none. Not allowed inside a composed `<button>` or `<checkbox>`.

| Prop | Type | Required |
| --- | --- | --- |
| `count` | `number` | yes |
| `row` | `(index: number) => JSX.Element` | yes |
| `height` | `(index: number) => number` |  |
| `anchor` | `"end" \| { index: number; align: "start" \| "end"; seq?: number }` |  |
| `itemKeys` | `readonly string[]` |  |
| `onReachStart` | `() => void` |  |
| `onReachEnd` | `() => void` |  |

## `<image>`

Children: none. May sit inside a composed `<button>` or `<checkbox>`.

| Prop | Type | Required |
| --- | --- | --- |
| `src` | `string` | yes |
| `alt` | `string` |  |
| `retryKey` | `string` |  |
| `onLoad` | `() => void` |  |
| `onError` | `() => void` |  |

## `<input>`

Children: none. Not allowed inside a composed `<button>` or `<checkbox>`.

Binding: `bind={local}` with a `useLocal<string>` cell, or `value` plus `onInput(value: string)`; not both.

| Prop | Type | Required |
| --- | --- | --- |
| `accessibilityLabel` | `string` |  |
| `placeholder` | `string` |  |
| `secure` | `boolean` |  |
| `keyboard` | `"text" \| "number" \| "email" \| "phone" \| "date" \| "time"` |  |
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

Binding: `bind={local}` with a `useLocal<string>` cell, or `value` plus `onInput(value: string)`; not both.

| Prop | Type | Required |
| --- | --- | --- |
| `accessibilityLabel` | `string` |  |
| `placeholder` | `string` |  |
| `secure` | `boolean` |  |
| `keyboard` | `"text" \| "number" \| "email" \| "phone"` |  |
| `autoFocus` | `boolean` |  |
| `disabled` | `boolean` |  |
| `submitOnEnter` | `boolean` |  |
| `onSubmit` | `(value: string) => void` |  |
| `maxLength` | `number` |  |
| `autoComplete` | `AutoComplete` |  |
| `enterKeyHint` | `"enter" \| "done" \| "go" \| "next" \| "previous" \| "search" \| "send"` |  |
| `autoCapitalize` | `"none" \| "sentences" \| "words" \| "characters" \| "off" \| "on"` |  |
| `spellCheck` | `boolean` |  |

## `<search>`

Children: none. Not allowed inside a composed `<button>` or `<checkbox>`.

Binding: `bind={local}` with a `useLocal<string>` cell, or `value` plus `onInput(value: string)`; not both.

| Prop | Type | Required |
| --- | --- | --- |
| `accessibilityLabel` | `string` |  |
| `placeholder` | `string` |  |
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
| `accessibilityLabel` | `string` |  |
| `disabled` | `boolean` |  |

## `<switch>`

Children: text only. Not allowed inside a composed `<button>` or `<checkbox>`.

Binding: `bind={local}` with a `useLocal<boolean>` cell, or `value` plus `onChange(value: boolean)`; not both.

| Prop | Type | Required |
| --- | --- | --- |
| `accessibilityLabel` | `string` |  |
| `disabled` | `boolean` |  |

## `<radio>`

Children: none. Not allowed inside a composed `<button>` or `<checkbox>`.

Binding: `bind={local}` with a `useLocal<string>` cell, or `value` plus `onChange(value: string)`; not both.

| Prop | Type | Required |
| --- | --- | --- |
| `options` | `readonly RadioOption[]` | yes |
| `accessibilityLabel` | `string` |  |
| `disabled` | `boolean` |  |

## `<slider>`

Children: none. Not allowed inside a composed `<button>` or `<checkbox>`.

Binding: `bind={local}` with a `useLocal<number>` cell, or `value` plus `onChange(value: number)`; not both.

| Prop | Type | Required |
| --- | --- | --- |
| `min` | `number` |  |
| `max` | `number` |  |
| `step` | `number` |  |
| `accessibilityLabel` | `string` | yes |
| `disabled` | `boolean` |  |

## `<grid>`

Children: any elements and text. May sit inside a composed `<button>` or `<checkbox>`.

No element-specific props.

## `<stack>`

Children: any elements and text. May sit inside a composed `<button>` or `<checkbox>`.

No element-specific props.

## `<plot>`

Children: none. Not allowed inside a composed `<button>` or `<checkbox>`.

| Prop | Type | Required |
| --- | --- | --- |
| `kind` | `"bar" \| "line" \| "pie"` | yes |
| `values` | `readonly number[]` | yes |
| `colors` | `readonly string[]` | yes |
| `accessibilityLabel` | `string` | yes |
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
| `onPress` | `() => void` |  |
| `accessibilityLabel` | `string` |  |
| `disabled` | `boolean` |  |

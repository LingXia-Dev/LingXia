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
| `"aria-hidden"` | `boolean` | Removes the node and its subtree from accessibility. |
| `"aria-live"` | `"off" \| "polite" \| "assertive"` | Announce changes; `alert` is assertive and `status` polite unless set. |
| `"aria-pressed"` | `boolean` | States assistive input announces; leave one out when the element has no such state. |
| `"aria-selected"` | `boolean` |  |
| `"aria-expanded"` | `boolean` |  |
| `"aria-busy"` | `boolean` |  |
| `"aria-invalid"` | `boolean` | A field's error and required states; the error text belongs in `aria-describedby`. |
| `"aria-required"` | `boolean` |  |
| `"aria-describedby"` | `string` | Ids of elements (1..8, space-separated) whose text is read after the name, as a hint or error. |

`style` takes the element's own style subset; `className` takes complete Tailwind utilities.

## `<view>`

Children: any elements and text. Renders as `column`. `open`, `onToggle`, `closeAfter`, `anchor`, `onCloseRequest` need `popover`; `onCloseRequest` not with `popover="manual"`: tsc and the build refuse them otherwise (`UI_PROP`). `dragSnapPoints`, `bind`, `snapIndex`, `onSnapChange`, `onDragStart`, `onDragEnd`, `aria-actions` need `drag`: tsc and the build refuse them otherwise (`UI_PROP`). `pointerAxis` need `onPointer`: the build refuses them otherwise (`UI_PROP`). At most one of `bind` or `snapIndex` (and its `onSnapChange`) (`UI_PROP`).

| Prop | Type | Required | Notes |
| --- | --- | --- | --- |
| `headingLevel` | `1 \| 2 \| 3 \| 4 \| 5 \| 6` |  | A heading's level, 1..6, with `role="heading"`. |
| `aria-valuenow` | `number` |  | The value of a `progressbar`, a `spinbutton` or an `onPointer` element, between `aria-valuemin` and `aria-valuemax`; leave it out while a progressbar's amount is unknown. On an `onPointer` element it is the value arrows and a screen reader step. |
| `aria-valuemin` | `number` |  | The lowest value; 0 unless set. |
| `aria-valuemax` | `number` |  | The highest value; 100 unless set. |
| `aria-valuetext` | `string` |  | Spoken in place of the number, such as "1:20 of 3:05". |
| `aria-modal` | `boolean` |  | On a popover with `role="dialog"`: modal like a `<dialog>`. Tab stays inside, the rest of the page leaves accessibility, and the page shows over the host's bars; without it the popover dialog is non-modal. |
| `bleed` | `BleedEdge` |  | On a direct child of `<page>`: extend under the safe-area insets to the screen edge, on every side or the named ones. |
| `onPage` | `(direction: "previous" \| "next") => void` |  | Called for PageUp and PageDown on a `role="grid"`. |
| `onContextMenu` | `(event: ContextMenuEvent) => void` |  | Called for a long press or a secondary click, with where it happened. |
| `anchor` | `string` |  | The id of the element a popover is placed against. |
| `onPointer` | `(event: PointerEvent) => void` |  | Reports a single pointer over the element: down, move, up, enter, leave and cancel. |
| `pointerAxis` | `"x" \| "y" \| "both"` |  | The axis `onPointer` keeps; the other is left to the page's scroll. |
| `onLayout` | `(box: BoxSize) => void` |  | Called with its laid-out size after each change, one render behind. |
| `box` | `Box` |  | A `useBox` handle this element fills with its laid-out size. |
| `aria-label` | `string` |  | The accessible name, spoken in place of its text. |
| `popover` | `boolean \| "auto" \| "manual" \| "hint"` |  | Makes it a popover in the top layer: `auto` light-dismisses, `manual` closes only when told, `hint` for a hover hint. |
| `open` | `boolean` |  | Whether the popover is shown; with `onToggle` to follow its own closing. |
| `onToggle` | `(open: boolean) => void` |  | Called with the new state when the popover opens or closes. |
| `drag` | `"x" \| "y"` |  | Lets the finger move it and its subtree along one axis, between its `dragSnapPoints`. |
| `dragSnapPoints` | `` readonly (number \| `${number}%`)[] `` |  | Up to 8 offsets, in pixels or percents of its size, a drag comes to rest at. |
| `bind` | `Local<number>` |  | A number `useLocal` cell holding the index of the snap point it rests on. |
| `snapIndex` | `number` |  | The index of the snap point it rests on, with `onSnapChange`; not with `bind`. |
| `onSnapChange` | `(index: number) => void` |  | Called with the snap point index it came to rest at after a drag or swipe. |
| `onDragStart` | `() => void` |  | Called when a drag starts. |
| `onDragEnd` | `(info: { index: number; velocity: number }) => void` |  | Called when a drag is released, with the snap index it heads for and its velocity. |
| `aria-actions` | `string` |  | Ids of buttons (space-separated) a screen reader offers as this element's actions, as for a swipe row. |
| `onPointerEnter` | `() => void` |  | Called when a mouse, stylus or trackpad pointer enters it. |
| `onPointerLeave` | `() => void` |  | Called when that pointer leaves it. |
| `closeAfter` | `number` |  | On a `popover="manual"`: close it this many milliseconds (1..60000) after it opens, as a toast. |
| `onCloseRequest` | `() => boolean \| void` |  | Called when Escape, back or a screen reader asks the popover to close; return `false` to keep it open. |

## `<page>`

Children: any elements and text. The page root.

| Prop | Type | Required | Notes |
| --- | --- | --- | --- |
| `title` | `string` |  | The title the host navigation bar shows for the page; the page's only title writer, so Logic changes it through page data, not `navigationBar.update`. |
| `aria-label` | `string` |  | The accessible name, spoken in place of its text. |

## `<text>`

Children: text, `<span>` and `<link>` runs.

| Prop | Type | Required | Notes |
| --- | --- | --- | --- |
| `headingLevel` | `1 \| 2 \| 3 \| 4 \| 5 \| 6` |  | A heading's level, 1..6, with `role="heading"`. |
| `onContextMenu` | `(event: ContextMenuEvent) => void` |  | Called for a long press or a secondary click, with where it happened. |
| `onLayout` | `(box: BoxSize) => void` |  | Called with its laid-out size after each change, one render behind. |
| `box` | `Box` |  | A `useBox` handle this element fills with its laid-out size. |
| `aria-label` | `string` |  | The accessible name, spoken in place of its text. |

## `<button>`

Children: text, or noninteractive content elements (a composed control). `dragSnapPoints`, `bind`, `snapIndex`, `onSnapChange`, `onDragStart`, `onDragEnd`, `aria-actions` need `drag`: tsc and the build refuse them otherwise (`UI_PROP`). `pointerAxis` need `onPointer`: the build refuses them otherwise (`UI_PROP`). At most one of `bind` or `snapIndex` (and its `onSnapChange`) (`UI_PROP`).

| Prop | Type | Required | Notes |
| --- | --- | --- | --- |
| `aria-level` | `number` |  | A `treeitem`'s depth, 1..32; Left and Right walk between levels. |
| `aria-controls` | `string` |  | With `role="combobox"`: the ids (1..8, space-separated) of the list it opens. Assistive tech and automation read them; an id that names no element is left out. |
| `aria-valuetext` | `string` |  | With `role="combobox"`: what is chosen, spoken after the name. |
| `aria-checked` | `boolean \| "mixed"` |  | Checked state of a `radio`, `menuitemcheckbox` or `menuitemradio` item; `"mixed"` on a `menuitemcheckbox` that is partly checked. |
| `onContextMenu` | `(event: ContextMenuEvent) => void` |  | Called for a long press or a secondary click, with where it happened. |
| `onPointer` | `(event: PointerEvent) => void` |  | Reports a single pointer over the element: down, move, up, enter, leave and cancel. |
| `pointerAxis` | `"x" \| "y" \| "both"` |  | The axis `onPointer` keeps; the other is left to the page's scroll. |
| `onClick` | `() => void` |  | Called when it is pressed: a tap, a click, Enter or Space, or a screen reader's activation. |
| `disabled` | `boolean` |  | Refuses input and shows the disabled style; it stays in accessibility, marked unavailable. |
| `aria-label` | `string` |  | The accessible name, spoken in place of its text. |
| `popoverTarget` | `string` |  | The id of a popover a press opens or closes. |
| `interestFor` | `string` |  | The id of a `popover="hint"` shown while the pointer rests on it or it has focus. |
| `aria-current` | `"page" \| "step" \| boolean` |  | The current item of a set: `"page"` in navigation, `"step"` in a progress of steps, `true` otherwise; `false` or absent for the rest. |
| `keepOpen` | `boolean` |  | On a menu item: run `onClick` and leave the menu open. |
| `keepsFocus` | `boolean` |  | A press leaves the focused editor editing: its caret, selection and keyboard stay, as a format bar above the keyboard needs. |
| `drag` | `"x" \| "y"` |  | Lets the finger move it and its subtree along one axis, between its `dragSnapPoints`. |
| `dragSnapPoints` | `` readonly (number \| `${number}%`)[] `` |  | Up to 8 offsets, in pixels or percents of its size, a drag comes to rest at. |
| `bind` | `Local<number>` |  | A number `useLocal` cell holding the index of the snap point it rests on. |
| `snapIndex` | `number` |  | The index of the snap point it rests on, with `onSnapChange`; not with `bind`. |
| `onSnapChange` | `(index: number) => void` |  | Called with the snap point index it came to rest at after a drag or swipe. |
| `onDragStart` | `() => void` |  | Called when a drag starts. |
| `onDragEnd` | `(info: { index: number; velocity: number }) => void` |  | Called when a drag is released, with the snap index it heads for and its velocity. |
| `aria-actions` | `string` |  | Ids of buttons (space-separated) a screen reader offers as this element's actions, as for a swipe row. |
| `onPointerEnter` | `() => void` |  | Called when a mouse, stylus or trackpad pointer enters it. |
| `onPointerLeave` | `() => void` |  | Called when that pointer leaves it. |

## `<scroll>`

Children: any elements and text. At most one of `bind` or `snapIndex` (and its `onSnapChange`) (`UI_PROP`). Not allowed inside a pressable control (`UI_BUTTON`).

| Prop | Type | Required | Notes |
| --- | --- | --- | --- |
| `bleed` | `BleedEdge` |  | On a direct child of `<page>`: extend under the safe-area insets to the screen edge, on every side or the named ones. |
| `onLayout` | `(box: BoxSize) => void` |  | Called with its laid-out size after each change, one render behind. |
| `box` | `Box` |  | A `useBox` handle this element fills with its laid-out size. |
| `onReachStart` | `() => void` |  | Called when the scroll reaches its start, as for loading older items. |
| `onReachEnd` | `() => void` |  | Called when the scroll reaches its end, as for loading more items. |
| `onScrollEdge` | `(event: ScrollEdgeEvent) => void` |  | Called when the scroll reaches or leaves either edge, and on mount with the edges it starts at. |
| `bind` | `Local<number>` |  | A number `useLocal` cell holding the index of the snap point a `scrollSnapType` scroll rests on. |
| `snapIndex` | `number` |  | The index of the snap point a `scrollSnapType` scroll rests on, with `onSnapChange`; not with `bind`. |
| `onSnapChange` | `(index: number) => void` |  | Called with the snap point index it came to rest at after a drag or swipe. |

`bind` and `snapIndex` are the index of the snap point a `scrollSnapType` scroll rests on, not its offset: `bind={local}` with a number `useLocal` cell, or `snapIndex={n}` with `onSnapChange(index)` to follow it from the page's or Logic's state. Not both (`UI_PROP`). After a swipe the element waits for the page's answer; if the page keeps its index, the element returns to it. The answer may come through a Logic action `onSnapChange` starts; the wait ends at 400 ms. Only actions the handler calls directly count: one started after an `await` or in a timer is not waited for, and the page is taken to keep its index after the next frame. The same holds on a dragged element.

## `<List>`

Children: none. Not allowed inside a pressable control (`UI_BUTTON`).

A component from `@lingxia/view`, rendered as the `list` element.

| Prop | Type | Required | Notes |
| --- | --- | --- | --- |
| `items` | `readonly T[]` | yes | The items, one row each; only rows near the screen are rendered. |
| `getKey` | `(item: T, index: number) => string` | yes | A stable, short key for an item, so state and position follow it when items move. |
| `renderItem` | `(item: T, index: number) => JSX.Element` | yes | Renders the row for an item. |
| `getItemHeight` | `(item: T, index: number) => number` |  | The height of an item's row, when rows differ; rows are measured otherwise. |
| `bleed` | `BleedEdge` |  | On a direct child of `<page>`: extend under the safe-area insets to the screen edge, on every side or the named ones. |
| `anchor` | `"end" \| { index: number; align: "start" \| "end"; seq?: number; duration?: number }` |  | `"end"` starts at the last row; `{ index, align, seq }` scrolls there, again whenever `seq` changes. |
| `onReachStart` | `() => void` |  | Called when the scroll reaches its start, as for loading older items. |
| `onReachEnd` | `() => void` |  | Called when the scroll reaches its end, as for loading more items. |
| `onScrollEdge` | `(event: ScrollEdgeEvent) => void` |  | Called when the scroll reaches or leaves either edge, and on mount with the edges it starts at. |
| `aria-label` | `string` |  | The accessible name, spoken in place of its text. |

## `<image>`

Children: none.

| Prop | Type | Required | Notes |
| --- | --- | --- | --- |
| `bleed` | `BleedEdge` |  | On a direct child of `<page>`: extend under the safe-area insets to the screen edge, on every side or the named ones. |
| `src` | `string` | yes | The picture: a package path, an `lx://` file, or a URL on an allowed host; PNG or JPEG. |
| `placeholder` | `string` |  | A picture shown until `src` has loaded, and if it fails. |
| `alt` | `string` |  | What the image shows, spoken by a screen reader; empty for a decorative image. |
| `retryKey` | `string` |  | Change it to load the same `src` again after an error. |
| `onLoad` | `() => void` |  | Called once per `src` and `retryKey`, when the picture has loaded. |
| `onError` | `() => void` |  | Called once per `src` and `retryKey`, when the picture fails to load. |

## `<input>`

Children: none.

Binding: `bind={local}` with a `useLocal<string>` cell, or `value` plus `onInput(value: string)`; not both. `onInput` runs on every value change, a composition included.

| Prop | Type | Required | Notes |
| --- | --- | --- | --- |
| `aria-valuenow` | `number` |  | The value of a `progressbar`, a `spinbutton` or an `onPointer` element, between `aria-valuemin` and `aria-valuemax`; leave it out while a progressbar's amount is unknown. On an `onPointer` element it is the value arrows and a screen reader step. |
| `aria-valuemin` | `number` |  | The lowest value; 0 unless set. |
| `aria-valuemax` | `number` |  | The highest value; 100 unless set. |
| `aria-valuetext` | `string` |  | Spoken in place of the number, such as "1:20 of 3:05". |
| `aria-activedescendant` | `string` |  | The id of the option the editor's keys point at while focus stays in the text; a screen reader hears that option as it changes. |
| `aria-controls` | `string` |  | With `role="combobox"`: the ids (1..8, space-separated) of the list it opens. Assistive tech and automation read them; an id that names no element is left out. |
| `aria-autocomplete` | `"list" \| "none"` |  | With `role="combobox"`: `"list"` when typing filters the list it opens. |
| `onKeyDown` | `EditorKeys` |  | Keys the editor handles before typing: a map from key name to a handler returning `true` when it took the key. |
| `readOnly` | `boolean` |  | The text can be selected and copied but not changed. |
| `onFocus` | `() => void` |  | Called when the editor gains focus. |
| `onBlur` | `() => void` |  | Called when the editor loses focus. |
| `aria-label` | `string` |  | The accessible name, spoken in place of its text. |
| `placeholder` | `string` |  | Shown while the value is empty. |
| `type` | `"text" \| "password" \| "email" \| "number" \| "tel" \| "search" \| "date" \| "time"` |  | The kind of value: picks the keyboard, masking, and the date or time picker. |
| `inputMode` | `"text" \| "numeric" \| "decimal" \| "tel" \| "email"` |  | The on-screen keyboard to show, without changing what is accepted. |
| `autoFocus` | `boolean` |  | Focus the editor when it first appears. |
| `disabled` | `boolean` |  | Refuses input and shows the disabled style; it stays in accessibility, marked unavailable. |
| `onSubmit` | `(value: string) => void` |  | Called with the text for the keyboard's action key, or Enter on a single-line editor. |
| `maxLength` | `number` |  | The most characters it holds; typing stops there. |
| `autoComplete` | `AutoComplete` |  | What the value is, so the system can offer saved or received values, such as a one-time code. |
| `enterKeyHint` | `"enter" \| "done" \| "go" \| "next" \| "previous" \| "search" \| "send"` |  | The label of the keyboard's action key. |
| `autoCapitalize` | `"none" \| "sentences" \| "words" \| "characters" \| "off" \| "on"` |  | Which letters the keyboard capitalises on its own. |
| `spellCheck` | `boolean` |  | Whether misspellings are marked and corrected. |
| `min` | `number \| string` |  | The lowest value: a number for `type="number"`, a date or time string for a picker. |
| `max` | `number \| string` |  | The highest value: a number for `type="number"`, a date or time string for a picker. |
| `step` | `number` |  | The interval a number snaps to. |

## `<textarea>`

Children: none.

Binding: `bind={local}` with a `useLocal<string>` cell, or `value` plus `onInput(value: string)`; not both. `onInput` runs on every value change, a composition included.

| Prop | Type | Required | Notes |
| --- | --- | --- | --- |
| `onKeyDown` | `EditorKeys` |  | Keys the editor handles before typing: a map from key name to a handler returning `true` when it took the key. |
| `readOnly` | `boolean` |  | The text can be selected and copied but not changed. |
| `onFocus` | `() => void` |  | Called when the editor gains focus. |
| `onBlur` | `() => void` |  | Called when the editor loses focus. |
| `rows` | `number` |  | Lines it shows when empty; it grows with its text up to `maxRows`. |
| `maxRows` | `number` |  | Lines it grows to before it scrolls. |
| `aria-label` | `string` |  | The accessible name, spoken in place of its text. |
| `placeholder` | `string` |  | Shown while the value is empty. |
| `inputMode` | `"text" \| "numeric" \| "decimal" \| "tel" \| "email"` |  | The on-screen keyboard to show, without changing what is accepted. |
| `autoFocus` | `boolean` |  | Focus the editor when it first appears. |
| `disabled` | `boolean` |  | Refuses input and shows the disabled style; it stays in accessibility, marked unavailable. |
| `onSubmit` | `(value: string) => void` |  | Called with the text for the keyboard's action key, or Enter on a single-line editor. |
| `maxLength` | `number` |  | The most characters it holds; typing stops there. |
| `autoComplete` | `AutoComplete` |  | What the value is, so the system can offer saved or received values, such as a one-time code. |
| `enterKeyHint` | `"enter" \| "done" \| "go" \| "next" \| "previous" \| "search" \| "send"` |  | The label of the keyboard's action key. |
| `autoCapitalize` | `"none" \| "sentences" \| "words" \| "characters" \| "off" \| "on"` |  | Which letters the keyboard capitalises on its own. |
| `spellCheck` | `boolean` |  | Whether misspellings are marked and corrected. |

## `<checkbox>`

Children: text, or noninteractive content elements (a composed control).

Binding: `bind={local}` with a `useLocal<boolean>` cell, or `value` plus `onChange(value: boolean)`; not both.

| Prop | Type | Required | Notes |
| --- | --- | --- | --- |
| `indeterminate` | `boolean` |  | Shows a mixed state, as for a parent of partly checked items; a press clears it. |
| `defaultChecked` | `boolean` |  | Where an unbound control starts; it applies once. |
| `aria-label` | `string` |  | The accessible name, spoken in place of its text. |
| `disabled` | `boolean` |  | Refuses input and shows the disabled style; it stays in accessibility, marked unavailable. |

`defaultChecked`: where a checkbox without `value` or `bind` starts; it applies once, and a later change does nothing.

## `<switch>`

Children: text, or noninteractive content elements (a composed control).

Binding: `bind={local}` with a `useLocal<boolean>` cell, or `value` plus `onChange(value: boolean)`; not both.

| Prop | Type | Required | Notes |
| --- | --- | --- | --- |
| `defaultChecked` | `boolean` |  | Where an unbound control starts; it applies once. |
| `aria-label` | `string` |  | The accessible name, spoken in place of its text. |
| `disabled` | `boolean` |  | Refuses input and shows the disabled style; it stays in accessibility, marked unavailable. |

`defaultChecked`: where a switch without `value` or `bind` starts; it applies once, and a later change does nothing.

## `<radio>`

Children: none.

Binding: `bind={local}` with a `useLocal<string>` cell, or `value` plus `onChange(value: string)`; not both.

| Prop | Type | Required | Notes |
| --- | --- | --- | --- |
| `options` | `readonly ChoiceOption[]` | yes | The choices, each a value and a label. |
| `aria-label` | `string` |  | The accessible name, spoken in place of its text. |
| `disabled` | `boolean` |  | Refuses input and shows the disabled style; it stays in accessibility, marked unavailable. |

## `<slider>`

Children: none.

Binding: `bind={local}` with a `useLocal<number>` cell, or `value` plus `onChange(value: number)`; not both.

`onChangeEnd(value)` runs once a change is finished: when the press that made it lifts, or at once for a key or assistive step.

Two values: `values={[lower, upper]}` with `thumbLabels={[name, name]}` makes a control with two thumbs that cannot cross; `onChange` and `onChangeEnd` then take `[lower, upper]`.

| Prop | Type | Required | Notes |
| --- | --- | --- | --- |
| `min` | `number` |  | The lowest value it accepts. |
| `max` | `number` |  | The highest value it accepts. |
| `step` | `number` |  | The interval values snap to. |
| `defaultValue` | `number` |  | Where an unbound slider starts, snapped to `step`; it applies once. |
| `secondaryValue` | `number` |  | A second range drawn behind the value, such as buffered media. |
| `aria-label` | `string` | yes | The accessible name, spoken in place of its text. |
| `aria-valuetext` | `string` |  | Spoken in place of the number, such as "1:20 of 3:05". |
| `disabled` | `boolean` |  | Refuses input and shows the disabled style; it stays in accessibility, marked unavailable. |
| `onPressChange` | `(pressed: boolean) => void` |  | Called with `true` when a finger or pointer takes the thumb and `false` when it lets go. |

`defaultValue`: where a slider without `value` or `bind` starts, snapped to `step`; it applies once, and a later change does nothing.

## `<dialog>`

Children: any elements and text. Renders as `column`. `dragSnapPoints`, `bind`, `snapIndex`, `onSnapChange`, `onDragStart`, `onDragEnd` need `drag`: tsc and the build refuse them otherwise (`UI_PROP`). At most one of `bind` or `snapIndex` (and its `onSnapChange`) (`UI_PROP`).

| Prop | Type | Required | Notes |
| --- | --- | --- | --- |
| `aria-modal` | `boolean` |  | Always true for a `<dialog>`; `false` is refused. |
| `open` | `boolean` |  | Whether the dialog is shown; with `onClose` to follow a close request. |
| `onClose` | `() => void` |  | Called for a request to close: Escape, back, a backdrop tap or a sheet dragged off; it closes when you set `open` to false. |
| `closedby` | `"any" \| "closerequest" \| "none"` |  | The close requests it takes: `closerequest` (default) Escape and back, `any` also a backdrop tap, `none` none. |
| `aria-label` | `string` | yes | The accessible name, spoken in place of its text. |
| `drag` | `"x" \| "y"` |  | Lets the finger move it and its subtree along one axis, between its `dragSnapPoints`. |
| `dragSnapPoints` | `` readonly (number \| `${number}%`)[] `` |  | Up to 8 offsets, in pixels or percents of its size, a drag comes to rest at. |
| `bind` | `Local<number>` |  | A number `useLocal` cell holding the index of the snap point it rests on. |
| `snapIndex` | `number` |  | The index of the snap point it rests on, with `onSnapChange`; not with `bind`. |
| `onSnapChange` | `(index: number) => void` |  | Called with the snap point index it came to rest at after a drag or swipe. |
| `onDragStart` | `() => void` |  | Called when a drag starts. |
| `onDragEnd` | `(info: { index: number; velocity: number }) => void` |  | Called when a drag is released, with the snap index it heads for and its velocity. |

## `<plot>`

Children: none.

| Prop | Type | Required | Notes |
| --- | --- | --- | --- |
| `onLayout` | `(box: BoxSize) => void` |  | Called with its laid-out size after each change, one render behind. |
| `box` | `Box` |  | A `useBox` handle this element fills with its laid-out size. |
| `kind` | `"bar" \| "line" \| "pie"` | yes | The chart: bars, a line, or a pie. |
| `values` | `readonly number[]` | yes | The data, one number per bar, point or slice. |
| `colors` | `readonly ColorSeries[]` | yes | Chart colour roles, 1..16, cycled over bars and slices; a line uses the first. |
| `aria-label` | `string` | yes | The accessible name, spoken in place of its text. |
| `smooth` | `boolean` |  | Draw a line chart as a smooth curve. |
| `area` | `boolean` |  | Fill under a line chart. |
| `selectedIndex` | `number` |  | The highlighted bar, point or slice; pass back what `onSelect` reports. |
| `domainMin` | `number` |  | The lowest value the axis shows; it must contain zero and every value. |
| `domainMax` | `number` |  | The highest value the axis shows; it must contain zero and every value. |
| `onSelect` | `(index: number) => void` |  | Called with the index of the bar, point or slice a press picks. |
| `pieLayout` | `PieLayout` |  | The pie's centre and radii, as fractions of the box. |
| `markerEmphasisScale` | `number` |  | How much larger the selected point's marker is drawn. |

## `<vector>`

Children: none. `pointerAxis` need `onPointer`: the build refuses them otherwise (`UI_PROP`).

| Prop | Type | Required | Notes |
| --- | --- | --- | --- |
| `bleed` | `BleedEdge` |  | On a direct child of `<page>`: extend under the safe-area insets to the screen edge, on every side or the named ones. |
| `onPointer` | `(event: PointerEvent) => void` |  | Reports a single pointer over the element: down, move, up, enter, leave and cancel. |
| `pointerAxis` | `"x" \| "y" \| "both"` |  | The axis `onPointer` keeps; the other is left to the page's scroll. |
| `onLayout` | `(box: BoxSize) => void` |  | Called with its laid-out size after each change, one render behind. |
| `box` | `Box` |  | A `useBox` handle this element fills with its laid-out size. |
| `paths` | `ReadonlyArray<VectorPath>` | yes | The shapes to draw: SVG path data with a fill and a stroke each. |
| `gradients` | `Readonly<Record<string, VectorGradient>>` |  | Up to 8 named gradients the paths use as `fill: 'url(#id)'`. |
| `viewBox` | `readonly [number, number, number, number]` |  | The drawing's coordinate box, `[x, y, width, height]`, scaled to fit the element. |

## `<span>`

Children: text, `<span>` and `<link>` runs. Only inside `<text>`. It takes, of the props every element takes, only `style`, `className`, `key`, `data-testid`, `dir`, `aria-hidden`; the rest go on its `<text>` (`UI_PROP`). Its `data-testid` finds the run itself: its rect is the run's own words, so a tap lands on them.

No element-specific props.

## `<link>`

Inside `<text>`, a run: children text and `<span>` runs; links do not nest. Elsewhere, a block link (`link-box`): a pressable box whose children are text, or noninteractive content elements (a composed control), styled like `<button>`; a block link never sits inside another (`UI_LINK`). As a run it takes, of the props every element takes, only `style`, `className`, `key`, `data-testid`, `dir`, `aria-describedby`; the rest go on its `<text>` (`UI_PROP`). Its `data-testid` finds the run itself: its rect is the run's own words, so a tap lands on them. Needs `to`, `onClick`, `popoverTarget` or `interestFor`, disabled or not: `disabled` only stops the press (`UI_LINK`). Enter activates it; Space does not.

| Prop | Type | Required | Notes |
| --- | --- | --- | --- |
| `onContextMenu` | `(event: ContextMenuEvent) => void` |  | Called for a long press or a secondary click, with where it happened. |
| `onClick` | `() => void` |  | Called when it is pressed: a tap, a click, Enter or Space, or a screen reader's activation. |
| `to` | `LinkTarget` |  | The page a press navigates to, with its params. |
| `aria-label` | `string` |  | The accessible name, spoken in place of its text. |
| `disabled` | `boolean` |  | Refuses input and shows the disabled style; it stays in accessibility, marked unavailable. |
| `popoverTarget` | `string` |  | The id of a popover a press opens or closes. |
| `interestFor` | `string` |  | The id of a `popover="hint"` shown while the pointer rests on it or it has focus. |
| `aria-current` | `"page" \| "step" \| boolean` |  | The current item of a set: `"page"` in navigation, `"step"` in a progress of steps, `true` otherwise; `false` or absent for the rest. |

## `<select>`

Children: none.

Binding: `bind={local}` with a `useLocal<string>` cell, or `value` plus `onChange(value: string)`; not both.

| Prop | Type | Required | Notes |
| --- | --- | --- | --- |
| `options` | `readonly ChoiceOption[]` | yes | The choices, each a value and a label. |
| `defaultValue` | `string` |  | The option an unbound select starts on; it applies once. |
| `placeholder` | `string` |  | Shown while the value is empty. |
| `aria-label` | `string` |  | The accessible name, spoken in place of its text. |
| `disabled` | `boolean` |  | Refuses input and shows the disabled style; it stays in accessibility, marked unavailable. |

`defaultValue`: the option a select without `value` or `bind` starts on; it applies once, and a later change does nothing.

## `<video>`

Children: none. At most one of `src` or `sources` (`UI_PROP`).

| Prop | Type | Required | Notes |
| --- | --- | --- | --- |
| `bleed` | `BleedEdge` |  | On a direct child of `<page>`: extend under the safe-area insets to the screen edge, on every side or the named ones. |
| `src` | `string` |  | The video to play; not with `sources`. |
| `sources` | `readonly VideoSource[]` |  | The same video at several qualities; not with `src`. |
| `player` | `Video` |  | A `useVideo` player: control it from code, or show the same player in another `<video>`. |
| `poster` | `string` |  | An image shown until the first frame. |
| `autoPlay` | `boolean` |  | Start playing once it can. |
| `paused` | `boolean` |  | Pause or play; applies on mount and when it changes. |
| `muted` | `boolean` |  | Mute or unmute; applies on mount and when it changes. |
| `loop` | `boolean` |  | Start over at the end. |
| `playbackRate` | `number` |  | Playback speed, 1 normal. |
| `volume` | `number` |  | Volume, 0..1. |
| `aria-label` | `string` |  | The accessible name, spoken in place of its text. |
| `onLoadedMetadata` | `(event: VideoMetadata) => void` |  | Called when the duration and size are known. |
| `onPlay` | `() => void` |  | Called when playback is asked to start. |
| `onPlaying` | `() => void` |  | Called when frames start moving, after a start or a stall. |
| `onPause` | `(event: VideoPause) => void` |  | Called when playback pauses, with why. |
| `onWaiting` | `() => void` |  | Called when playback stalls for data. |
| `onSeeked` | `(event: { readonly currentTime: number }) => void` |  | Called when a seek has finished, with the new time. |
| `onTimeUpdate` | `(event: { readonly currentTime: number }) => void` |  | Called as the playback time moves, a few times a second. |
| `onEnded` | `() => void` |  | Called when playback reaches the end. |
| `onError` | `(error: VideoError) => void` |  | Called when the video fails to load or play. |
| `onRateChange` | `(event: { readonly playbackRate: number }) => void` |  | Called when the playback speed changes. |
| `onVolumeChange` | `(event: { readonly volume: number; readonly muted: boolean }) => void` |  | Called when the volume or mute changes. |
| `onQualityChange` | `(event: VideoQualityChange) => void` |  | Called when a new quality is on screen. |
| `onFullscreenChange` | `(event: { readonly fullscreen: boolean }) => void` |  | Called when it enters or leaves full screen. |

`src` plays one file; `sources` offers the same video at several qualities; not both (`UI_PROP`). Props apply when they change: a new `src` or `sources` starts over from the beginning, showing the last frame until the new first one; `paused`, `muted`, `loop`, `playbackRate` and `volume`: each applies on mount and when its value changes; writing the same value again does nothing, so a handle's `play()` or `pause()` stands until that prop itself changes. To act, use the player handle. A named player is the advanced case: `player={useVideo(name)}` on the `<video>` that gives the source makes it the page's player, and another `<video player>` with no source, such as a full-screen one, shows that same player. A component that forwards a source types it with `VideoSourceProps` and spreads it whole.

## `<webview>`

Children: none. Not allowed inside a pressable control (`UI_BUTTON`). Moves live with a translation. Under opacity, a mask, a blur, a scale or a rotation it shows a still picture of the page and takes no input until the effect ends. Scaling or rotating it outside entering and leaving fails the build (`UI_TRANSFORM`); a scale or rotation applied at run time shows the still picture. Its page's `prefers-color-scheme` follows the View page's appearance, live.

| Prop | Type | Required | Notes |
| --- | --- | --- | --- |
| `bleed` | `BleedEdge` |  | On a direct child of `<page>`: extend under the safe-area insets to the screen edge, on every side or the named ones. |
| `src` | `string` |  | The page to load. |
| `web` | `WebView` |  | A `useWebView` document: pass the same one to each `<webview>` that shows it, keeping one live page. |
| `aria-label` | `string` | yes | The accessible name, spoken in place of its text. |
| `handlesDrags` | `boolean` |  | The web page handles its own drags (a map, a canvas); a drag starting on it never passes to the page. |
| `onFullscreenChange` | `(event: { readonly fullscreen: boolean }) => void` |  | Called when it enters or leaves full screen. |
| `onLoad` | `(event: WebViewLoad) => void` |  | Called when a page has loaded, with its address and title. |
| `onMessage` | `(message: WebViewMessage) => void` |  | Called with what the web page posts through `window.lingxia.postMessage`. |
| `onBlocked` | `(event: WebViewBlocked) => void` |  | Called with a navigation the host cancelled: outside trustedDomains, or not https. |
| `onError` | `(error: WebViewError) => void` |  | Called when a page fails to load. |

`handlesDrags`: the web page handles its own drags (a map, a canvas); a drag starting on it never passes to the page. Off by default. When the screen takes a drag the page had started, the page gets `pointercancel` and `touchcancel`; on HarmonyOS they are synthetic (`isTrusted` false), and the page's own overflow scrolling, fixed when the finger lands, keeps scrolling.

_Generated from the View schema (`schema.json`) and the compiler; do not edit._

# Semantics

The accessibility tree the platform adapter exposes. Elements get their role from what they are;
the `role` prop sets one of the authored roles on any non-control element.

## Authored roles

| `role` | Goes on | Exposed as |
| --- | --- | --- |
| `heading` | any non-control element | `heading` |
| `list` | any non-control element | `list` |
| `listitem` | any non-control element | `list-item` |
| `dialog` | `<view>` with `popover` | `dialog` |
| `alert` | any non-control element | `alert` |
| `status` | any non-control element | `status` |
| `grid` | any non-control element | `grid` |
| `gridcell` | any non-control element | `grid-cell` |
| `listbox` | any non-control element | `list-box` |
| `option` | any non-control element | `option` |
| `menu` | any non-control element | `menu` |
| `menuitem` | `<button>` or `<link>` | `menu-item` |
| `progressbar` | any non-control element | `progress-bar` |
| `alertdialog` | `<dialog>` | `alert-dialog` |
| `group` | any non-control element | `group` |
| `tooltip` | any non-control element | `tooltip` |
| `tablist` | `<view>` | `tab-list` |
| `tab` | `<button>` | `tab` |
| `tabpanel` | `<view>` | `tab-panel` |
| `radiogroup` | `<view>` | `radio-group` |
| `radio` | `<button>` | `undefined` |
| `toolbar` | `<view>` | `toolbar` |
| `tree` | `<view>` or `<scroll>` or `<list>` | `tree` |
| `treeitem` | `<button>` | `tree-item` |
| `row` | `<view>` | `row` |
| `menuitemcheckbox` | `<button>` | `menu-item-check-box` |
| `menuitemradio` | `<button>` | `menu-item-radio` |
| `img` | `<view>` or `<text>` with `aria-label` | `image` |
| `combobox` | `<input>` or `<button>` | `combo-box` |
| `spinbutton` | `<input>` | `undefined` |

A `<text>` takes only `heading`, `listitem`, `alert`, `status`, `tooltip`, `gridcell`, `option`, `img` (`UI_PROP`).
A `<button>` takes only `tab`, `radio`, `option`, `gridcell`, `treeitem`, `menuitem`, `menuitemcheckbox`, `menuitemradio`, `combobox` (`UI_PROP`).
A `<scroll>` takes only `list`, `listbox`, `grid`, `menu`, `group`, `tree` (`UI_PROP`).
A `<List>` takes only `list`, `listbox`, `grid`, `menu`, `group`, `tree` (`UI_PROP`).
A `<input>` takes only `combobox`, `spinbutton` (`UI_PROP`).
A block `<link>` takes only `menuitem` (`UI_PROP`).
A `<dialog>` takes only `alertdialog` (`UI_PROP`).

`<page>`, `<image>`, `<vector>`, `<plot>`, `<textarea>`, `<checkbox>`, `<switch>`, `<radio>`, `<slider>`, `<select>`, `<video>`, `<webview>`, `<link>` keep their own role and take no `role` (`UI_PROP`).

## Props with conditions

A literal combination that breaks one of these fails the build; a computed one fails at render (`UI_PROP`).

| Prop | Needs |
| --- | --- |
| `open` | on `<view>`: `open belongs to a popover; add popover` |
| `onToggle` | on `<view>`: `onToggle belongs to a popover; add popover` |
| `closeAfter` | on `<view>`: `closeAfter belongs to a popover; add popover` |
| `anchor` | on `<view>`: `anchor belongs to a popover; add popover` |
| `onCloseRequest` | on `<view>`: `onCloseRequest belongs to a popover; add popover` · on `<view>`: `onCloseRequest belongs on a popover that is not manual; nothing but your code closes a manual one` |
| `dragSnapPoints` | on `<view>`, `<button>`, `<dialog>`: `dragSnapPoints belongs to an element with drag; add drag="x" or drag="y"` |
| `bind` | on `<view>`, `<button>`, `<dialog>`: `bind belongs to an element with drag; add drag="x" or drag="y"` · on `<view>`, `<button>`, `<dialog>`, `<scroll>`: `bind and snapIndex both set the snap index; bind a useLocal cell, or pass snapIndex and update it from onSnapChange` |
| `snapIndex` | on `<view>`, `<button>`, `<dialog>`: `snapIndex belongs to an element with drag; add drag="x" or drag="y"` · on `<view>`, `<button>`, `<dialog>`, `<scroll>`: `snapIndex and bind both set the snap index; bind a useLocal cell, or pass snapIndex and update it from onSnapChange` · on `<view>`, `<button>`, `<dialog>`, `<scroll>`: `snapIndex controls the position, so it needs onSnapChange to hear where a swipe settled; use bind to let the element keep the index itself` |
| `onSnapChange` | on `<view>`, `<button>`, `<dialog>`: `onSnapChange belongs to an element with drag; add drag="x" or drag="y"` · on `<view>`, `<button>`, `<dialog>`, `<scroll>`: `onSnapChange reports the snap index you pass in snapIndex; add snapIndex, or bind a useLocal cell instead` |
| `onDragStart` | on `<view>`, `<button>`, `<dialog>`: `onDragStart belongs to an element with drag; add drag="x" or drag="y"` |
| `onDragEnd` | on `<view>`, `<button>`, `<dialog>`: `onDragEnd belongs to an element with drag; add drag="x" or drag="y"` |
| `aria-actions` | on `<view>`, `<button>`, `<dialog>`: `aria-actions belongs to an element with drag; add drag="x" or drag="y"` |
| `pointerAxis` | on `<view>`, `<button>`, `<vector>`: `pointerAxis needs onPointer on the same element; add onPointer or remove pointerAxis` |
| `src` | on `<video>`: `src and sources both name the video; give src for one file, or sources for a choice of quality` |
| `sources` | on `<video>`: `sources and src both name the video; give src for one file, or sources for a choice of quality` |
| `aria-valuenow` | `role="combobox" takes aria-valuetext only, a string` · `aria-valuenow, -valuemin, -valuemax and -valuetext belong on role="progressbar", "spinbutton" or "combobox", or an onPointer element` |
| `aria-valuemin` | `role="combobox" takes aria-valuetext only, a string` · `aria-valuenow, -valuemin, -valuemax and -valuetext belong on role="progressbar", "spinbutton" or "combobox", or an onPointer element` |
| `aria-valuemax` | `role="combobox" takes aria-valuetext only, a string` · `aria-valuenow, -valuemin, -valuemax and -valuetext belong on role="progressbar", "spinbutton" or "combobox", or an onPointer element` |
| `aria-valuetext` | `aria-valuenow, -valuemin, -valuemax and -valuetext belong on role="progressbar", "spinbutton" or "combobox", or an onPointer element` |
| `headingLevel` | `headingLevel belongs on role="heading"` |
| `aria-level` | `aria-level belongs on role="treeitem"` |
| `aria-controls` | `aria-controls and aria-autocomplete belong on role="combobox"` |
| `aria-autocomplete` | `aria-controls and aria-autocomplete belong on role="combobox"` |
| `aria-checked` | `aria-checked="mixed" goes on role="menuitemcheckbox"; a <checkbox> takes indeterminate` · `aria-checked goes on role="radio", "menuitemcheckbox" or "menuitemradio"; a checkbox has its own checked` |
| `aria-modal` | `aria-modal belongs on a popover with role="dialog"` · `a <dialog> is always modal; a non-modal dialog is a popover with role="dialog"` |
| `onPage` | `onPage belongs to a <view role="grid">` |
| `keepOpen` | `keepOpen belongs on a menu item: role="menuitem", "menuitemcheckbox" or "menuitemradio"` |

## All exposed roles

`button`, `check-box`, `switch`, `radio-group`, `radio-button`, `combo-box`, `text-field`, `scroll-view`, `horizontal-scroll-view`, `image`, `slider`, `text`, `link`, `heading`, `list`, `list-item`, `dialog`, `alert`, `status`, `grid`, `grid-cell`, `list-box`, `option`, `menu`, `menu-item`, `video`, `progress-bar`, `alert-dialog`, `group`, `tooltip`, `tab-list`, `tab`, `tab-panel`, `toolbar`, `tree`, `tree-item`, `row`, `menu-item-check-box`, `menu-item-radio`, `web-view`

## Live regions

`aria-live`: `off`, `polite`, `assertive`. Announced politeness: `polite`, `assertive`.

## Text actions

Selectable text supports `copy`, `cut`, `paste`, `set-selection`.

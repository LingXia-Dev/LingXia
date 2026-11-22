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
| `menuitem` | `<button>` | `menu-item` |
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
| `tree` | `<view>` | `tree` |
| `treeitem` | `<button>` | `tree-item` |
| `row` | `<view>` | `row` |
| `menuitemcheckbox` | `<button>` | `menu-item-check-box` |
| `menuitemradio` | `<button>` | `menu-item-radio` |
| `img` | `<view>` or `<text>` with `aria-label` | `image` |
| `combobox` | `<input>` or `<button>` | `combo-box` |
| `spinbutton` | `<input>` | `undefined` |

A `<dialog>` takes only `alertdialog` (`UI_PROP`).

`headingLevel` (1..6) applies only to `role="heading"`.

## All exposed roles

`button`, `check-box`, `switch`, `radio-group`, `radio-button`, `combo-box`, `text-field`, `scroll-view`, `horizontal-scroll-view`, `image`, `slider`, `text`, `link`, `heading`, `list`, `list-item`, `dialog`, `alert`, `status`, `grid`, `grid-cell`, `list-box`, `option`, `menu`, `menu-item`, `video`, `progress-bar`, `alert-dialog`, `group`, `tooltip`, `tab-list`, `tab`, `tab-panel`, `toolbar`, `tree`, `tree-item`, `row`, `menu-item-check-box`, `menu-item-radio`, `web-view`

## Live regions

`aria-live`: `off`, `polite`, `assertive`. Announced politeness: `polite`, `assertive`.

## Text actions

Selectable text supports `copy`, `cut`, `paste`, `set-selection`.

_Generated from the View schema (`schema.json`) and the compiler; do not edit._

# Semantics

The accessibility tree the platform adapter exposes. Elements get their role from what they are;
the `role` prop sets one of the authored roles on any non-control element.

## Authored roles

| `role` | Exposed as |
| --- | --- |
| `heading` | `heading` |
| `list` | `list` |
| `listitem` | `list-item` |
| `alert` | `alert` |
| `status` | `status` |
| `grid` | `grid` |
| `gridcell` | `grid-cell` |
| `listbox` | `list-box` |
| `option` | `option` |
| `menu` | `menu` |
| `menuitem` | `menu-item` |
| `progressbar` | `progress-bar` |
| `alertdialog` | `alert-dialog` |
| `group` | `group` |
| `tooltip` | `tooltip` |
| `tablist` | `tab-list` |
| `tab` | `tab` |
| `tabpanel` | `tab-panel` |
| `radiogroup` | `radio-group` |
| `radio` | `undefined` |
| `toolbar` | `toolbar` |
| `tree` | `tree` |
| `treeitem` | `tree-item` |
| `row` | `row` |
| `menuitemcheckbox` | `menu-item-check-box` |
| `menuitemradio` | `menu-item-radio` |
| `img` | `image` |
| `combobox` | `combo-box` |
| `spinbutton` | `undefined` |

`headingLevel` (1..6) applies only to `role="heading"`.

## All exposed roles

`button`, `check-box`, `switch`, `radio-group`, `radio-button`, `combo-box`, `text-field`, `scroll-view`, `horizontal-scroll-view`, `image`, `slider`, `text`, `link`, `heading`, `list`, `list-item`, `dialog`, `alert`, `status`, `grid`, `grid-cell`, `list-box`, `option`, `menu`, `menu-item`, `video`, `progress-bar`, `alert-dialog`, `group`, `tooltip`, `tab-list`, `tab`, `tab-panel`, `toolbar`, `tree`, `tree-item`, `row`, `menu-item-check-box`, `menu-item-radio`

## Live regions

`aria-live`: `off`, `polite`, `assertive`. Announced politeness: `polite`, `assertive`.

## Text actions

Selectable text supports `copy`, `cut`, `paste`, `set-selection`.

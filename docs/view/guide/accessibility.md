# Accessibility

```tsx
import { useLocal } from '@lingxia/view';

const bills = ['Water', 'Power', 'Internet'];

export function render() {
  const paid = useLocal<string[]>('paid', []);
  const status = paid.value.length ? `${paid.value.length} of ${bills.length} paid` : 'Nothing paid yet';
  return (
    <page className="p-4 gap-3">
      <text role="heading" headingLevel={1} className="text-2xl font-semibold">Bills</text>
      <view role="list" aria-label="Bills due" className="gap-2">
        {bills.map(bill => (
          <view key={bill} role="listitem" className="flex-row items-center justify-between">
            <text className="text-sm">{bill}</text>
            <button disabled={paid.value.includes(bill)} onClick={() => paid.set([...paid.value, bill])}
              aria-label={`Pay ${bill}`} className="rounded-lg bg-accent px-3 py-1 text-sm text-on-accent">
              Pay
            </button>
          </view>
        ))}
      </view>
      <text role="status" className="text-sm text-muted">{status}</text>
    </page>
  );
}
```

The host builds an accessibility tree from each presented frame and hands it to the platform screen
reader. Every element has a role from what it is; you add names, structural roles and live regions.

## Names

| Element | Named by |
| --- | --- |
| `<button>`, `<checkbox>` | `aria-label`, else the text and image `alt` inside it, in order |
| `<switch>`, `<link>` | `aria-label`, else its text |
| `<input>`, `<textarea>` | `aria-label`; the placeholder is a hint, the value is the text |
| `<radio>` | `aria-label` for the group; each option by its `label` |
| `<slider>`, `<plot>` | `aria-label`, which is required |
| `<image>` | `alt`; `alt=""` hides a decorative image |
| `<text>` | its text |

Give icon-only buttons an `aria-label`. Labels are non-empty, at most 1 KiB, and do not
change what is painted.

## Roles

Controls, editors, scrolls, images and text get their roles automatically. Any other element can
take one of the structural roles with `role`; on a control, `role` is ignored except the item roles
below (`tab`, `radio`, `option`, `menuitem`, `menuitemcheckbox`, `menuitemradio`, `treeitem`) and
`gridcell`, which a button may take.

| `role` | Use |
| --- | --- |
| `heading` | a section title; `headingLevel` 1–6 (default 2) |
| `list`, `listitem` | a group of items; each item reports its position among the `listitem`s of its nearest `list` |
| `menu`, `menuitem` | a popover of actions and its buttons |
| `alert` | an urgent message, announced assertively when its text changes |
| `status` | a result or progress message, announced politely when its text changes |
| `grid`, `gridcell` | a two-dimensional set of cells, such as a month of days; name each cell in full |
| `listbox`, `option` | a set of choices to pick one from; mark the chosen one with `aria-selected` |
| `tablist`, `tab`, `tabpanel` | tabs and the content they show: mark the current tab with `aria-selected`; Left and Right select the previous or next tab (swapped in right-to-left layouts) |
| `radiogroup`, `radio` | buttons where one choice is on: mark it with `aria-checked`; the arrow keys move to the previous or next choice and turn it on |
| `toolbar` | a row of controls; Left and Right move focus between them without pressing them |
| `grid`, `row`, `gridcell` | cells in rows, such as a calendar's days: Left and Right move along the row, Up and Down to the same column of the next row, Home and End to the row's ends. A cell that takes no focus is passed over, and focus stays when there is none beyond. PageUp and PageDown call the grid's `onPage("previous" \| "next")` when it has one, else go to the column's first and last cell |
| `tree`, `treeitem` | nested items, such as folders: give each its depth with `aria-level` (1–32) and its open state with `aria-expanded`; Right presses a closed item open and moves into an open one, Left presses an open item shut and otherwise moves to the item above it (swapped in a right-to-left page); the item's `onClick` flips `aria-expanded` |
| `menuitemcheckbox`, `menuitemradio` | menu items that are on or off (`aria-checked`); in a `menuitemradio` set only one is on |
| `progressbar` | an amount done: `aria-valuenow` within `aria-valuemin` (0) and `aria-valuemax` (100), spoken as a percentage unless `aria-valuetext` is set; leave the value out while indeterminate. The kit `<Progress>` draws one as a bar or a ring |
| `combobox` | on a `<button>` that opens a list: `aria-expanded`, `aria-controls` and an `aria-valuetext` naming what is chosen. On an `<input>` that filters a list: also `aria-autocomplete="list"`, and `aria-activedescendant` naming the option its `onKeyDown` arrows point at, which is spoken as it changes while focus stays in the text |
| `spinbutton` | on a number `<input>`: `aria-valuenow`, `-valuemin`, `-valuemax` and an `aria-valuetext` with the unit; a screen reader's increment and decrement call the `ArrowUp` and `ArrowDown` handlers of its `onKeyDown` |

A modal is the `<dialog>` element, not a role: it keeps focus inside and hides the page behind it
([Overlays](overlays.md#focus-and-assistive-tech)). `role="menu"` names a popover of actions and
`role="menuitem"` its buttons.

Tab reaches a `tablist`, `radiogroup`, `toolbar`, `listbox`, `tree` or `grid` once, on its selected or
checked item, else the item focused there last, else its first item; Tab again leaves it. Inside,
the arrow keys move between items and Home and End go to the first and last (not in a radio group).
A `menu` takes the same arrow keys.

The full role list is in [semantics](../reference/semantics.md).

A `<select>` reports its chosen label as its value, or its placeholder before a choice. An editor's
selection offers copy, cut and paste to assistive technology; a `type="password"` editor offers none of them.

## Live regions

`role="alert"`, `role="status"` or `aria-live="polite" | "assertive"` make a region
announce its text when it changes from one frame to the next. The region speaks its own text, or the
text of its visible descendants in order. `aria-live="off"` silences an alert or status.

A region announces changes, not its first appearance: keep it mounted and change its text. A status
element that is inserted together with its message is not announced.

## Hiding

- `aria-hidden` removes an element and its subtree from the accessibility tree. It still paints
  and still receives taps. Use it for decoration and for icons inside labeled controls.
- `hidden` (`display: none`) removes an element from layout, paint, hit testing and accessibility.
- `opacity-0` hides nothing from the screen reader.

## Order and focus

The screen reader and the hardware keyboard follow tree order. `order-*` and `flex-*-reverse` change only
the visual order, so keep the tree in reading order. A control is enabled unless you disable it: one with no
handler, such as a `<select>` without `onChange`, is announced and focusable but does nothing.

## Text size

Text scales with the system font size ([Text](text.md)). Fixed heights do not scale: let text boxes
grow with `min-h-*`, and test pages at the largest font size.

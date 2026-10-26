# Accessibility

```tsx
import { useLocal } from '@lingxia/ui';

const bills = ['Water', 'Power', 'Internet'];

export function render() {
  const paid = useLocal<string[]>('paid', []);
  const status = paid.value.length ? `${paid.value.length} of ${bills.length} paid` : 'Nothing paid yet';
  return (
    <page className="p-4 gap-3">
      <text role="heading" headingLevel={1} className="text-2xl font-semibold">Bills</text>
      <column role="list" accessibilityLabel="Bills due" className="gap-2">
        {bills.map(bill => (
          <row key={bill} role="listitem" className="items-center justify-between">
            <text className="text-sm">{bill}</text>
            <button disabled={paid.value.includes(bill)} onClick={() => paid.set([...paid.value, bill])}
              accessibilityLabel={`Pay ${bill}`} className="rounded-lg px-3 py-1 text-sm">
              Pay
            </button>
          </row>
        ))}
      </column>
      <text role="status" className="text-sm text-neutral-600">{status}</text>
    </page>
  );
}
```

The host builds an accessibility tree from each presented frame and hands it to the platform screen
reader. Every element has a role from what it is; you add names, structural roles and live regions.

## Names

| Element | Named by |
| --- | --- |
| `<button>`, `<checkbox>` | `accessibilityLabel`, else the text and image `alt` inside it, in order |
| `<switch>`, `<link>` | `accessibilityLabel`, else its text |
| `<input>`, `<textarea>`, `<search>` | `accessibilityLabel`; the placeholder is a hint, the value is the text |
| `<radio>` | `accessibilityLabel` for the group; each option by its `label` |
| `<slider>`, `<plot>` | `accessibilityLabel`, which is required |
| `<image>` | `alt`; `alt=""` hides a decorative image |
| `<text>` | its text |

Give icon-only buttons an `accessibilityLabel`. Labels are non-empty, at most 1 KiB, and do not
change what is painted.

## Roles

Controls, editors, scrolls, images and text get their roles automatically. Any other element can
take one of the structural roles with `role`; on a control, `role` is ignored.

| `role` | Use |
| --- | --- |
| `heading` | a section title; `headingLevel` 1–6 (default 2) |
| `list`, `listitem` | a group of items; each item reports its position among the `listitem`s of its nearest `list` |
| `dialog` | a sheet or confirmation, named by `accessibilityLabel` |
| `alert` | an urgent message, announced assertively when its text changes |
| `status` | a result or progress message, announced politely when its text changes |

`role="dialog"` describes the region; it does not trap focus or block what is behind it. Hide or disable
the content behind a modal yourself.

The full role list is in [semantics](../reference/semantics.md).

## Live regions

`role="alert"`, `role="status"` or `accessibilityLiveRegion="polite" | "assertive"` make a region
announce its text when it changes from one frame to the next. The region speaks its own text, or the
text of its visible descendants in order. `accessibilityLiveRegion="off"` silences an alert or status.

A region announces changes, not its first appearance: keep it mounted and change its text. A status
element that is inserted together with its message is not announced.

## Hiding

- `accessibilityHidden` removes an element and its subtree from the accessibility tree. It still paints
  and still receives taps. Use it for decoration and for icons inside labeled controls.
- `hidden` (`display: none`) removes an element from layout, paint, hit testing and accessibility.
- `opacity-0` hides nothing from the screen reader.

## Order and focus

The screen reader and the hardware keyboard follow tree order. `order-*` and `flex-*-reverse` change only
the visual order, so keep the tree in reading order. Disabled controls are exposed as disabled.

## Text size

Text scales with the system font size ([Text](text.md)). Fixed heights do not scale: let text boxes
grow with `min-h-*`, and test pages at the largest font size.

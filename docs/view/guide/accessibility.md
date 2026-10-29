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
      <column role="list" aria-label="Bills due" className="gap-2">
        {bills.map(bill => (
          <row key={bill} role="listitem" className="items-center justify-between">
            <text className="text-sm">{bill}</text>
            <button disabled={paid.value.includes(bill)} onClick={() => paid.set([...paid.value, bill])}
              aria-label={`Pay ${bill}`} className="rounded-lg bg-accent px-3 py-1 text-sm text-on-accent">
              Pay
            </button>
          </row>
        ))}
      </column>
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
take one of the structural roles with `role`; on a control, `role` is ignored.

| `role` | Use |
| --- | --- |
| `heading` | a section title; `headingLevel` 1–6 (default 2) |
| `list`, `listitem` | a group of items; each item reports its position among the `listitem`s of its nearest `list` |
| `dialog` | a sheet or confirmation, named by `aria-label` |
| `alert` | an urgent message, announced assertively when its text changes |
| `status` | a result or progress message, announced politely when its text changes |

`role="dialog"` describes the region; it does not trap focus or block what is behind it. Hide or disable
the content behind a modal yourself.

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
the visual order, so keep the tree in reading order. Disabled controls are exposed as disabled.

## Text size

Text scales with the system font size ([Text](text.md)). Fixed heights do not scale: let text boxes
grow with `min-h-*`, and test pages at the largest font size.

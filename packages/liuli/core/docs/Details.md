<!-- Generated from the doc file by scripts/generate.mjs; do not edit. -->

# Details

A summary row that shows and hides the content under it. Status: preview.

```ts
import { Details, DetailsGroup } from '@liulidesign/core/Details';
```

Details hides secondary content behind a row the person opens when they need it: an answer, advanced settings, the fine print. DetailsGroup stacks rows in one bordered list, and can keep a single row open as an accordion.

## Examples

### One row

```tsx
import { Details } from '@liulidesign/core/Details';

const surface = 'gap-1 rounded-container border border-text/12 bg-surface px-5 pt-4 pb-1';

/** One disclosure under the text it qualifies: the summary opens and closes the content under it. */
export function Single() {
  return (
    <view className={surface}>
      <text className="text-body text-text">Team is €8 a month for each active member.</text>
      <Details label="What counts as an active member?" data-testid="active">
        <text className="text-body text-muted">Anyone who opened a shared notebook in the last 30 days. Guests with view access are free.</text>
      </Details>
    </view>
  );
}
```

### An accordion

```tsx
import { Details, DetailsGroup } from '@liulidesign/core/Details';

/** An accordion: opening one row closes the one that was open. */
export function Accordion() {
  return (
    <DetailsGroup isExclusive defaultValue="refunds" data-testid="faq">
      <Details value="refunds" label="Can I get a refund?" data-testid="refunds">
        <text className="text-body text-muted">Within 14 days of a payment, in full, from Billing.</text>
      </Details>
      <Details value="export" label="How do I export my notes?" data-testid="export">
        <text className="text-body text-muted">Settings, then Export: Markdown files in one archive, with attachments.</text>
      </Details>
      <Details value="delete" label="What happens when I delete my account?" data-testid="delete">
        <text className="text-body text-muted">Your notes are removed from every device after 30 days. Shared notebooks pass to their next member.</text>
      </Details>
    </DetailsGroup>
  );
}
```

### Rows that open on their own

```tsx
import { Details, DetailsGroup } from '@liulidesign/core/Details';

/** Several rows open at once. */
export function Independent() {
  return (
    <DetailsGroup>
      <Details label="Shipping" isDefaultOpen>
        <text className="text-body text-muted">Free over €40. Two to four working days.</text>
      </Details>
      <Details label="Returns" isDefaultOpen>
        <text className="text-body text-muted">30 days, in the original packaging.</text>
      </Details>
    </DetailsGroup>
  );
}
```

## Props

| Prop | Type | Default | Description |
| --- | --- | --- | --- |
| `label` (required) | `string` |  | The summary row's text; also what the toggle is named. |
| `children` (required) | `Child` |  | The body, which exists only while open. |
| `value` | `string` |  | Identifies the row in an exclusive `DetailsGroup`. |
| `isDefaultOpen` | `boolean` | `false` | Open on first render, uncontrolled. |
| `isOpen` | `boolean` |  | Controlled open state; pass `onOpenChange` with it. |
| `onOpenChange` | `((isOpen: boolean) => void)` |  |  |
| `className` | `string` |  |  |
| `data-testid` | `string` |  | On the disclosure. |

## Anatomy

| Part | Element | Required | Description |
| --- | --- | --- | --- |
| group | `<view>` | no | DetailsGroup: rows in one bordered list, divided by hairlines. |
| summary | `<button>` | yes | The toggle: a button named by `label` that announces whether it is expanded. |
| chevron | `<vector>` | yes | Turns over as the row opens. |
| body | `<view>` | no | The content, which exists only while open. |

## Accessibility

- Pattern: button.
- Name: label.
- Description: none.
- States: expanded (from isOpen, isDefaultOpen, or the group's open row).
- Enter, Space: Opens or closes the row..
- Press target: lg 48 px.
- Closed content does not exist, so a search or a screen reader reaches it only once it is open.
- In an exclusive group, closing the open row leaves none open.

| Contrast | Pair | Minimum | Light | Dark |
| --- | --- | --- | --- | --- |
| summary text | `text` on `surface` | 4.5:1 | 16.26:1 | 13.87:1 |

Ratios are computed from @lingxia/view/theme and rounded down.

## Guidance

- Do: Write the summary as the question or the name of what is inside.
- Do: Use an exclusive group when the rows are alternatives, such as answers in an FAQ.
- Don't: Hide what most people need; show it.
- Don't: Put a form someone must fill in behind a closed row.

## Not yet

| Missing | Needs | Meanwhile |
| --- | --- | --- |
| Liuli on the Details primitive. | a `<Summary>` that can drop or move its start marker, and whose inner row stretches to the button's width | Liuli builds the same button with aria-expanded, so the chevron can sit at the end. |
| A body that grows open instead of appearing. | height transitions, or a layout motion that animates size | The body fades and drops in; the rows below jump. |

Related: Item, Card.

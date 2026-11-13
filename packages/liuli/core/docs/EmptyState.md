<!-- Generated from the doc file by scripts/generate.mjs; do not edit. -->

# EmptyState

What a page or list shows when it has nothing in it yet, and the way forward. Status: preview.

```ts
import { EmptyState } from '@liulidesign/core/EmptyState';
```

EmptyState fills the space a list, page or panel would use when there is nothing to show: before the first item, after a search finds nothing, or when content is out of reach. It is a title, an optional description and actions, and an optional picture the app supplies. It draws no fill, border or illustration of its own.

## Examples

### First run, with the action that starts it

```tsx
import { useLocal, type VectorPath } from '@lingxia/view';
import { Button } from '@liulidesign/core/Button';
import { EmptyState } from '@liulidesign/core/EmptyState';

const notebook: VectorPath[] = [
  { d: 'M6 3.5h11a1.5 1.5 0 0 1 1.5 1.5v14a1.5 1.5 0 0 1-1.5 1.5H6zM6 3.5v17M9.5 8h5.5M9.5 11.5h5.5', fill: 'none', stroke: 'currentColor', strokeWidth: 1.5, strokeLinecap: 'round', strokeLinejoin: 'round' },
];

/** A first-run screen: what will be here, and the one action that starts it. */
export function FirstRun() {
  const created = useLocal('created', false);
  return (
    <EmptyState
      title={created.value ? 'Notebook created' : 'No notebooks yet'}
      description="Notebooks keep related notes together, and sync to your other devices."
      media={<vector aria-hidden paths={notebook} viewBox={[0, 0, 24, 24]} className="size-full" />}
      actions={<Button label="New notebook" variant="primary" onClick={() => created.set(true)} data-testid="create" />}
      data-testid="first-run"
    />
  );
}
```

### No results, with a way back

```tsx
import { useLocal, type VectorPath } from '@lingxia/view';
import { Button } from '@liulidesign/core/Button';
import { EmptyState } from '@liulidesign/core/EmptyState';

const search: VectorPath[] = [
  { d: 'M10.5 4a6.5 6.5 0 1 0 0 13a6.5 6.5 0 1 0 0-13zM15.5 15.5L20 20', fill: 'none', stroke: 'currentColor', strokeWidth: 1.5, strokeLinecap: 'round' },
];

/** No results: say what was searched, and offer a way back. */
export function NoResults() {
  const query = useLocal('query', 'invoice 2019');
  return (
    <EmptyState
      title="No matching notes"
      description={`Nothing matches "${query.value}". Check the spelling, or search all folders.`}
      media={<vector aria-hidden paths={search} viewBox={[0, 0, 24, 24]} className="size-full" />}
      actions={
        <>
          <Button label="Clear search" variant="ghost" onClick={() => query.set('')} />
          <Button label="Search all folders" />
        </>
      }
      data-testid="no-results"
    />
  );
}
```

### Small, inside a card

```tsx
import { Card } from '@liulidesign/core/Card';
import { EmptyState } from '@liulidesign/core/EmptyState';

/** Small, inside a card: no picture, one line. */
export function InCard() {
  return (
    <Card>
      <text role="heading" headingLevel={3} className="text-callout font-semibold text-text">Shared with you</text>
      <EmptyState size="sm" headingLevel={4} title="Nothing shared yet" description="Notes others share with you appear here." data-testid="in-card" />
    </Card>
  );
}
```

## Props

| Prop | Type | Default | Description |
| --- | --- | --- | --- |
| `title` (required) | `string` |  | What is empty, said plainly: "No notes yet". Drawn as a heading. |
| `description` | `string` |  | What to do about it, in one or two sentences. |
| `media` | `Child` |  | A picture above the title: an icon or the app's own illustration; hidden from assistive tech. Liuli draws none. |
| `actions` | `Child` |  | One or two Buttons under the text; the first is the way forward. |
| `headingLevel` | `1 \| 2 \| 3 \| 4 \| 5 \| 6` | `2` | The title's heading level in the page outline. |
| `size` | `EmptyStateSize` | `'md'` | `sm` sits inside a card or a list; `md` fills a page or a panel. |
| `className` | `string` |  |  |
| `data-testid` | `string` |  |  |

## Anatomy

| Part | Element | Required | Description |
| --- | --- | --- | --- |
| root | `<view>` | yes | A centred column with no fill or border, so it sits in whatever holds it. |
| media | `<view>` | no | `media`: the app's icon or illustration, in the faint text colour; hidden from assistive tech. |
| title | `<text>` | yes | A heading at `headingLevel`. |
| description | `<text>` | no | What to do about it. |
| actions | `<view>` | no | `actions`: one or two Buttons, wrapping onto a second line on a narrow screen. |

## Accessibility

- Pattern: none.
- Name: none; its content is read.
- Description: none.
- The title is a heading, so a screen reader can jump to it; set `headingLevel` to fit the page outline.
- The picture is decorative and hidden; say everything that matters in the title and description.
- It is not announced when it appears. After a search, the app reports the count in its own status line if it needs to be spoken.

| Contrast | Pair | Minimum | Light | Dark |
| --- | --- | --- | --- | --- |
| title | `text` on `surface` | 4.5:1 | 16.26:1 | 13.87:1 |
| title on the page | `text` on `canvas` | 4.5:1 | 15.19:1 | 15.31:1 |
| description | `text-muted` on `surface` | 4.5:1 | 5.89:1 | 6.64:1 |
| description on the page | `text-muted` on `canvas` | 4.5:1 | 5.50:1 | 7.33:1 |

Ratios are computed from @lingxia/view/theme and rounded down.

## Guidance

- Do: Say what is empty and why, then what to do: "No notebooks yet", "Notebooks keep related notes together".
- Do: Offer the action that fills it, and make it the first button.
- Do: Use `sm` inside a card or a list section, and `md` for a whole page or panel.
- Don't: Show an empty state while content is loading; show Skeleton until the answer is known.
- Don't: Use it for an error the person can fix in place; use a Banner with the fix.
- Don't: Put more than two actions in it.

Related: Skeleton, Banner, Button.

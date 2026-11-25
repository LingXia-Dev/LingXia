<!-- Generated from the doc file by scripts/generate.mjs; do not edit. -->

# JinshuDiff

What changed between two versions of a Jinshu document, block by block, with words struck and inserted. Status: preview.

```ts
import { JinshuDiff } from '@liulidesign/jinshu/JinshuDiff';
```

JinshuDiff compares two versions of a document and shows what changed: blocks added, removed, moved and modified, each in a card tinted by its kind, with the unchanged runs between them folded. A summary counts each kind and steps between the changes. A modified block shows its words struck and inserted in place, or the whole block before and after when the edit is too heavy to read word by word; `mode` chooses.

## Examples

### A draft against the saved version

```tsx
import { createDocument, heading, paragraph } from '@liulidesign/jinshu/schema';
import { JinshuDiff } from '@liulidesign/jinshu/JinshuDiff';

const saved = createDocument({
  blocks: [heading(2, 'Limits', 'h1'), paragraph('A limit says where a function goes.', 'p1'), paragraph('Old note.', 'p2'), paragraph('Unchanged.', 'p3')],
});
const draft = createDocument({
  blocks: [heading(2, 'Limits', 'h1'), paragraph('A limit says where a function heads.', 'p1'), paragraph('Unchanged.', 'p3'), paragraph('New exercise.', 'p4')],
});

/** A draft against the saved version: one block changed, one removed, one added. */
export function Revision() {
  return <JinshuDiff a={saved} b={draft} data-testid="diff" />;
}
```

### Inline: words, always

```tsx
import { callout, createDocument, heading, paragraph } from '@liulidesign/jinshu/schema';
import { JinshuDiff } from '@liulidesign/jinshu/JinshuDiff';

const policy = createDocument({
  blocks: [
    heading(2, 'Refunds', 'r1'),
    paragraph('Refunds are issued within 30 days of purchase.', 'r2'),
    callout('info', [paragraph('Digital items are not refundable once downloaded.', 'r3p')], { title: 'Note', id: 'r3' }),
  ],
});
const revised = createDocument({
  blocks: [
    heading(2, 'Refunds', 'r1'),
    paragraph('Refunds are issued within 14 days of purchase, to the original payment method.', 'r2'),
    callout('warning', [paragraph('Digital items are not refundable once downloaded.', 'r3p')], { title: 'Note', id: 'r3' }),
  ],
});

/** Inline: every changed text shows its words struck and inserted in place; a change with no text, like a callout's tone, shows before and after. */
export function Inline() {
  return <JinshuDiff a={policy} b={revised} mode="inline" data-testid="inline" />;
}
```

### Block: before and after, always

```tsx
import { callout, createDocument, heading, paragraph } from '@liulidesign/jinshu/schema';
import { JinshuDiff } from '@liulidesign/jinshu/JinshuDiff';

const policy = createDocument({
  blocks: [
    heading(2, 'Refunds', 'r1'),
    paragraph('Refunds are issued within 30 days of purchase.', 'r2'),
    callout('info', [paragraph('Digital items are not refundable once downloaded.', 'r3p')], { title: 'Note', id: 'r3' }),
  ],
});
const revised = createDocument({
  blocks: [
    heading(2, 'Refunds', 'r1'),
    paragraph('Refunds are issued within 14 days of purchase, to the original payment method.', 'r2'),
    callout('warning', [paragraph('Digital items are not refundable once downloaded.', 'r3p')], { title: 'Note', id: 'r3' }),
  ],
});

/** Block: each changed block shown before and after, for edits too heavy to read word by word. */
export function Block() {
  return <JinshuDiff a={policy} b={revised} mode="block" data-testid="block" />;
}
```

### No changes

```tsx
import { createDocument, heading, paragraph } from '@liulidesign/jinshu/schema';
import { JinshuDiff } from '@liulidesign/jinshu/JinshuDiff';

const saved = createDocument({
  blocks: [heading(2, 'Limits', 'h1'), paragraph('A limit says where a function goes.', 'p1'), paragraph('Old note.', 'p2'), paragraph('Unchanged.', 'p3')],
});

/** Two equal documents. */
export function NoChanges() {
  return <JinshuDiff a={saved} b={saved} data-testid="same" />;
}
```

## Props

| Prop | Type | Default | Description |
| --- | --- | --- | --- |
| `a` | `JinshuDocument` |  | The earlier document. |
| `b` | `JinshuDocument` |  | The later document. |
| `diff` | `DocumentDiff` |  | A diff computed in Logic with `diffDocuments`, in place of `a` and `b`; for a long document. |
| `mode` | `DiffMode` | `'smart'` | `smart` shows words for a light edit and the whole block for a heavy one; `inline` always words; `block` always whole blocks. |
| `className` | `string` |  |  |
| `data-testid` | `string` |  |  |

## Anatomy

| Part | Element | Required | Description |
| --- | --- | --- | --- |
| root | `<view>` | yes | The summary, then the changes in document order. |
| summary | `<view>` | yes | A bar with the number of changes, a badge per kind, and the stepper. |
| counts | `<view>` | yes | The total and one badge each for added, changed, removed and moved. |
| stepper | `<view>` | no | Previous and Next, with the position: "2 / 5"; each step scrolls to that change and rings it. |
| entries | `<view>` | yes | The changes and the folded runs between them. |
| expand | `<button>` | no | A dashed row standing for unchanged blocks; pressing it shows them. |
| change | `<view>` | no | One change, in a card tinted by its kind. |
| changeHeader | `<view>` | no | The change's kind and the block's type. |
| words | `<text>` | no | A modified block's text with removed words struck through and inserted words tinted. |
| sides | `<view>` | no | A heavily modified block, before then after, each labelled. |
| attrs | `<view>` | no | Attribute changes that have no text: a callout's tone, an image's size. |
| empty | `<text>` | no | One line when the versions are the same. |

## What an app may override

- `className`: classes merged after the recipe's, so they win.
- The recipe, `jinshuDiffRecipe` from `@liulidesign/jinshu/JinshuDiff/recipe`: the classes of each part, to style an element of your own to match. The component does not read a changed copy.
- Scale tokens it reads, set in the app's stylesheet under `@theme`: `--text-caption`, `--text-footnote`, `--text-body`, `--radius-control`, `--radius-container`.
- Colour roles it paints with, set by the theme pack: `accent`, `danger`, `danger-soft`, `info`, `info-soft`, `success`, `success-soft`, `surface`, `surface-sunken`, `text`, `text-muted`.

## Accessibility

- Pattern: none.
- Name: none; its content is read.
- Description: none.
- Each card names its kind in words (added, removed, moved, changed), so the tint is never the only sign.
- Removed words are struck through as well as tinted; the before and after sides of a heavy edit are labelled.
- The stepper's buttons are named Previous and Next, and the position is read beside them.

| Contrast | Pair | Minimum | Light | Dark |
| --- | --- | --- | --- | --- |
| removed words | `danger` on `danger-soft` | 4.5:1 | 4.53:1 | 4.77:1 |
| inserted words | `success` on `success-soft` | 4.5:1 | 4.68:1 | 6.45:1 |
| text in an added card | `text` on `success-soft/60` over `canvas` | 4.5:1 | 14.67:1 | 13.30:1 |
| text in a removed card | `text-muted` on `danger-soft/60` over `canvas` | 4.5:1 | 5.26:1 | 6.53:1 |
| before label | `danger` on `danger-soft/50` over `canvas` | 4.5:1 | 4.69:1 | 5.45:1 |
| after label | `success` on `success-soft/50` over `canvas` | 4.5:1 | 4.82:1 | 7.50:1 |
| step position | `text-muted` on `surface` | 4.5:1 | 5.89:1 | 6.64:1 |

Ratios are computed from @lingxia/view/theme and rounded down.

## Guidance

- Do: Leave `mode` at `smart` for review: light edits read as words, rewrites as before and after.
- Do: Compute the diff in Logic with `diffDocuments` and pass `diff` for a long document, so the page does not.
- Do: Use it beside the history, or before an agent's save, to show what will be kept.
- Don't: Use it to review an agent's change as it happens; the editor marks those blocks and offers undo for the batch.

## Limits

- Inserted words are told apart by their tint alone; a screen reader reads the block's new text without marking them.
- It compares two versions; it does not merge them or accept a change.

Related: JinshuEditor, JinshuView, Badge.

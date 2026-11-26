<!-- Generated from the doc file by scripts/generate.mjs; do not edit. -->

# JinshuOutline

A Jinshu document's headings as a contents list that moves the reader and shows each section's progress. Status: preview.

```ts
import { useJinshuNav, extractOutline, JinshuOutline } from '@liulidesign/jinshu/JinshuOutline';
```

JinshuOutline lists a document's headings, indented by level, as a contents list. Shared with a JinshuView or JinshuEditor through `useJinshuNav()`, pressing a heading scrolls the document to it and tints that heading as the one being read. An app can mark each section done, current or locked, as a course does.

## Examples

### With each section's progress

```tsx
import { createDocument, heading, paragraph } from '@liulidesign/jinshu/schema';
import { JinshuOutline, useJinshuNav } from '@liulidesign/jinshu/JinshuOutline';

const doc = createDocument({
  blocks: [
    heading(1, 'Limits', 'h1'),
    paragraph('Where a function heads.', 'p1'),
    heading(2, 'Definition', 'h2'),
    heading(2, 'Exercises', 'h3'),
    heading(3, '', 'h4'),
  ],
});

/** Headings by level, with each section's progress; the one being read is tinted. */
export function Progress() {
  const nav = useJinshuNav('outline-example');
  return <JinshuOutline doc={doc} nav={nav} status={{ h1: 'done', h2: 'current', h3: 'locked' }} data-testid="outline" />;
}
```

### Beside the document

```tsx
import { createDocument, heading, paragraph } from '@liulidesign/jinshu/schema';
import { JinshuView } from '@liulidesign/jinshu/JinshuView';
import { JinshuOutline, useJinshuNav } from '@liulidesign/jinshu/JinshuOutline';

const doc = createDocument({
  blocks: [
    heading(1, 'Limits', 'h1'),
    paragraph('Where a function heads.', 'p1'),
    heading(2, 'Definition', 'h2'),
    heading(2, 'Exercises', 'h3'),
    heading(3, '', 'h4'),
  ],
});
const guide = createDocument({
  title: 'Onboarding guide',
  blocks: [
    heading(2, 'Your first day', 'g1'),
    paragraph('Collect your badge at reception and set up your laptop with IT.', 'g1p'),
    heading(2, 'Your first week', 'g2'),
    paragraph('Meet your team, read the handbook and pick a starter task.', 'g2p'),
    heading(3, 'Accounts to request', 'g3'),
    paragraph('Email, chat, the code host and the design library.', 'g3p'),
    heading(2, 'Your first month', 'g4'),
    paragraph('Ship your starter task and book a check-in with your manager.', 'g4p'),
  ],
});

/** In a side column beside the document, sharing one nav handle: pressing a heading scrolls the reader to it. */
export function BesideTheDocument() {
  const nav = useJinshuNav('outline-beside');
  return (
    <view className="flex-row items-start gap-6">
      <JinshuOutline doc={guide} nav={nav} className="w-48 shrink-0" />
      <JinshuView doc={guide} nav={nav} className="grow min-w-0" />
    </view>
  );
}
```

### Behind a Contents button

```tsx
import { Button } from '@liulidesign/core/Button';
import { Popover } from '@liulidesign/core/Popover';
import { createDocument, heading, paragraph } from '@liulidesign/jinshu/schema';
import { JinshuView } from '@liulidesign/jinshu/JinshuView';
import { JinshuOutline, useJinshuNav } from '@liulidesign/jinshu/JinshuOutline';

const doc = createDocument({
  blocks: [
    heading(1, 'Limits', 'h1'),
    paragraph('Where a function heads.', 'p1'),
    heading(2, 'Definition', 'h2'),
    heading(2, 'Exercises', 'h3'),
    heading(3, '', 'h4'),
  ],
});
const guide = createDocument({
  title: 'Onboarding guide',
  blocks: [
    heading(2, 'Your first day', 'g1'),
    paragraph('Collect your badge at reception and set up your laptop with IT.', 'g1p'),
    heading(2, 'Your first week', 'g2'),
    paragraph('Meet your team, read the handbook and pick a starter task.', 'g2p'),
    heading(3, 'Accounts to request', 'g3'),
    paragraph('Email, chat, the code host and the design library.', 'g3p'),
    heading(2, 'Your first month', 'g4'),
    paragraph('Ship your starter task and book a check-in with your manager.', 'g4p'),
  ],
});

/** On a phone: the outline in a popover from a Contents button, over the document. */
export function InPopover() {
  const nav = useJinshuNav('outline-popover');
  return (
    <view className="gap-3">
      <Popover label="Contents" content={<JinshuOutline doc={guide} nav={nav} title="On this page" />}>
        <Button label="Contents" startIcon="list" variant="secondary" size="sm" />
      </Popover>
      <JinshuView doc={guide} nav={nav} />
    </view>
  );
}
```

## Props

| Prop | Type | Default | Description |
| --- | --- | --- | --- |
| `doc` | `JinshuDocument` |  | The document whose headings it lists. |
| `items` | `readonly OutlineItem[]` |  | Headings taken out in Logic with `extractOutline`, in place of `doc`. |
| `nav` | `JinshuNavHandle` |  | Shared with the JinshuView or JinshuEditor it moves, from `useJinshuNav()`. |
| `status` | `Readonly<Record<string, OutlineSectionStatus>>` |  | Each section's progress by heading id: `done`, `current` or `locked`, shown at the row's end. |
| `title` | `string` |  | The outline's heading and accessible name. Defaults to "Contents" in the page's language. |
| `onSelect` | `((headingId: string) => void)` |  | A heading was pressed; with `nav`, the reader also scrolls to it. |
| `className` | `string` |  |  |
| `data-testid` | `string` |  |  |

## Anatomy

| Part | Element | Required | Description |
| --- | --- | --- | --- |
| root | `<view>` | yes | A group named by `title`. |
| title | `<text>` | yes | The list's heading: "Contents" in the page's language unless `title` is set. |
| item | `<button>` | yes | A heading, indented by its level; the one being read is tinted in accent. |
| itemText | `<text>` | yes | The heading's text on one line, or "(Untitled)" for an empty heading. |
| status | `<text>` | no | With `status`: a tick for done, a dot for current, a ring for locked. |

## What an app may override

- `className`: classes merged after the recipe's, so they win.
- The recipe, `jinshuOutlineRecipe` from `@liulidesign/jinshu/JinshuOutline/recipe`: the classes of each part, to style an element of your own to match. The component does not read a changed copy.
- Scale tokens it reads, set in the app's stylesheet under `@theme`: `--text-caption`, `--text-footnote`, `--radius-control`.
- Colour roles it paints with, set by the theme pack: `accent`, `accent-soft`, `success`, `text`, `text-muted`.

## Accessibility

- Pattern: none.
- Name: title.
- Description: none.
- Each heading is a button named by its text.
- The status mark is a glyph read as it is drawn; say what done and locked mean in the page around it when it matters.

| Contrast | Pair | Minimum | Light | Dark |
| --- | --- | --- | --- | --- |
| heading | `text` on `canvas` | 4.5:1 | 15.19:1 | 15.31:1 |
| nested heading | `text-muted` on `canvas` | 4.5:1 | 5.50:1 | 7.33:1 |
| heading being read | `accent` on `accent-soft` | 4.5:1 | 4.93:1 | 5.02:1 |
| done | `success` on `canvas` | 3:1 | 4.96:1 | 8.40:1 |
| locked | `text-muted` on `canvas` | 3:1 | 5.50:1 | 7.33:1 |

Ratios are computed from @lingxia/view/theme and rounded down.

## Guidance

- Do: Share one `useJinshuNav()` handle between the outline and the document it moves.
- Do: Put it in a side column on a wide screen, and behind a Contents button on a phone.
- Do: Take the headings out in Logic with `extractOutline` and pass `items` when the page does not hold the document.
- Don't: Use it for a document with one or two headings; there is nothing to move between.

## Limits

- The tinted heading is the one last chosen; scrolling the document by hand does not move it.
- A locked section stays pressable; the app decides what opening it does.
- An empty document draws nothing.

Related: JinshuView, JinshuEditor, Popover.

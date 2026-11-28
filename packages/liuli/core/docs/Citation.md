<!-- Generated from the doc file by scripts/generate.mjs; do not edit. -->

# Citation

Where an answer's claims come from. Status: preview.

```ts
import { CitationGroup, Citation } from '@liulidesign/core/Citation';
```

CitationGroup holds an answer and the sources it rests on. A Citation puts a source's number after the claim it backs, among the words of a `<text>`; resting a pointer on it, focusing it or tapping it shows the source's card: its title, where it is from and the words it was cited for. Under the answer the sources sit as numbered pills, and a pill opens its source through `onOpen`. Numbers follow the order of `sources`, so the same source keeps its number wherever it is cited.

## Examples

### An assistant's answer citing three sources

```tsx
import { useLocal } from '@lingxia/view';
import { MessageBubble } from '@liulidesign/core/MessageBubble';
import { Avatar } from '@liulidesign/core/Avatar';
import { Citation, CitationGroup, type CitationSource } from '@liulidesign/core/Citation';

const sources: CitationSource[] = [
  { id: 'ferry', title: 'Ferry timetable, summer', site: 'ferries.example.com', excerpt: 'Saturday sailings: 09:30, 14:10 and 16:40. Crossing time about 40 minutes.' },
  { id: 'forecast', title: 'Coast forecast for Saturday', site: 'weather.example.com', excerpt: 'Dry until mid-afternoon; rain from about four, heavier in the evening.' },
  { id: 'notes', title: 'Trip notes', excerpt: 'Tent stays in the car if it looks like rain.' },
];

/** An assistant's answer with three sources: a mark after each claim, the sources as pills under it. */
export function Answer() {
  const opened = useLocal('');
  return (
    <view className="gap-2">
      <MessageBubble avatar={<Avatar name="Assistant" initials="A" size="sm" isDecorative />} author="Assistant">
        <CitationGroup sources={sources} onOpen={source => opened.set(source.title)} data-testid="answer">
          <text className="text-body">
            Take the 14:10 ferry: it crosses in about 40 minutes<Citation source="ferry" /> and lands before the rain starts around four<Citation source="forecast" />. Leave the tent in the car<Citation source="notes" />.
          </text>
        </CitationGroup>
      </MessageBubble>
      {opened.value !== '' && <text className="text-footnote text-muted" data-testid="opened">{`Opened ${opened.value}`}</text>}
    </view>
  );
}
```

## Props

| Prop | Type | Default | Description |
| --- | --- | --- | --- |
| `sources` (required) | `readonly CitationSource[]` |  | The sources, in the order they are numbered. |
| `children` (required) | `Child` |  | The cited content: `<text>` with a Citation after each claim, a Markdown, a MessageBubble's text. |
| `onOpen` (required) | `(source: CitationSource) => void` |  | Runs when a source's pill is pressed: the app opens the page or the document. |
| `label` | `string` |  | Names the row of sources for assistive tech. Defaults to "Sources" in the page's language. |
| `className` | `string` |  |  |
| `data-testid` | `string` |  | On the group; a pill gets `<testid>-source-<id>`, a mark `<testid>-mark-<id>`, a card `<testid>-card-<id>`. |

## Citation props

| Prop | Type | Default | Description |
| --- | --- | --- | --- |
| `source` (required) | `string` |  | The `id` of one of the group's sources. |
| `data-testid` | `string` |  | On the mark; defaults to the group's `<testid>-mark-<source>`. |

## CitationSource props

| Prop | Type | Default | Description |
| --- | --- | --- | --- |
| `id` (required) | `string` |  | What a Citation names it by. |
| `title` (required) | `string` |  | The page or document's title: "Ferry timetable, summer". |
| `site` | `string` |  | Where it is from, short: "example.com", "Trip notes". The pill shows it, else the title. |
| `excerpt` | `string` |  | The words it was cited for, shown on its card. |

## Anatomy

| Part | Element | Required | Description |
| --- | --- | --- | --- |
| mark | `<link>` | yes | The source's number among the words, on the accent's soft step; it shows the card. |
| sources | `<view>` | no | The sources under the answer: a group named "Sources". |
| source | `<button>` | no | One source's pill: its number and its site, else its title; it opens the source. |
| card | `<view>` | no | The source's card, shown from its mark: title, site and the cited words. |

## What an app may override

- `className`: classes merged after the recipe's, so they win.
- The recipe, `citationRecipe` from `@liulidesign/core/Citation/recipe`: the classes of each part, to style an element of your own to match. The component does not read a changed copy.
- Scale tokens it reads, set in the app's stylesheet under `@theme`: `--text-caption`, `--text-footnote`, `--text-callout`, `--shadow-raised`, `--radius-container`.
- Colour roles it paints with, set by the theme pack: `accent`, `accent-soft`, `focus`, `line`, `on-accent-soft`, `surface`, `surface-overlay`, `text`, `text-muted`.

## Accessibility

- Pattern: none.
- Name: label.
- Description: none.
- Tab: To each mark among the words, then each source's pill.
- Enter: On a mark, shows its card; on a pill, opens the source.
- Press target: .
- A mark reads its number and is described by its card, so a screen reader hears which source backs the claim without opening anything.
- Each pill is named "Source 1: its title, its site".

| Contrast | Pair | Minimum | Light | Dark |
| --- | --- | --- | --- | --- |
| mark | `accent` on `surface` | 4.5:1 | 5.62:1 | 6.19:1 |
| pill number | `on-accent-soft` on `accent-soft` | 4.5:1 | 8.54:1 | 9.49:1 |
| pill words | `text-muted` on `surface` | 4.5:1 | 5.89:1 | 6.64:1 |

Ratios are computed from @lingxia/view/theme and rounded down.

## Guidance

- Do: Put the mark straight after the claim it backs, before the full stop.
- Do: Give each source an `excerpt`: the words it was cited for, so the card shows why.
- Don't: Cite a source no claim uses; leave it out of `sources`.
- Don't: Use it for links the user is meant to follow as part of a task; that is a Link.

## Limits

- The card shows text only; a picture of the page is not drawn.
- Marks are numbers; a source named in the words is a Link.

Related: MessageBubble, Markdown, Link, HoverCard.

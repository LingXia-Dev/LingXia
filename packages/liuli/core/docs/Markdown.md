<!-- Generated from the doc file by scripts/generate.mjs; do not edit. -->

# Markdown

Markdown text drawn as Liuli type: a message, a note, a help page. Status: preview.

```ts
import { Markdown } from '@liulidesign/core/Markdown';
```

Markdown shows text someone wrote in Markdown: a chat reply, a note, release notes, help. It draws headings, paragraphs, bold, italic, struck and code runs, links, bulleted, numbered and task lists, quotes, code blocks, rules and tables. It only shows text; it does not edit it. Links do nothing on their own: the app opens an address in `onLinkClick`.

## Examples

### A note with tasks and a link

```tsx
import { useLocal } from '@lingxia/view';
import { Markdown } from '@liulidesign/core/Markdown';

const note = `# Trip to the coast

We leave on **Friday at 7:30** and are back *Sunday evening*. The house is
[ten minutes from the beach](https://example.com/house), and the key is in the lockbox.

## To pack

- [x] Tent and pegs
- [x] Stove, with a spare canister
- [ ] Rain jackets
- [ ] Charger for the camera

## On the way

1. Fuel before the motorway
2. Lunch in Setúbal
3. Call ahead if we are later than six
`;

/** A note: headings, emphasis, a link, a task list and a numbered list. */
export function Note() {
  const opened = useLocal('opened', '');
  return (
    <view className="gap-3">
      <Markdown source={note} onLinkClick={opened.set} taskLabels={{ done: 'Done', open: 'Not done' }} data-testid="note" />
      {opened.value !== '' && <text role="status" className="text-footnote text-muted">{`Opened ${opened.value}`}</text>}
    </view>
  );
}
```

### Code, a table and a quote

```tsx
import { Markdown } from '@liulidesign/core/Markdown';

const reference = `Set the token before the first call, then check the reply's \`status\`.

\`\`\`ts
const reply = await fetch('/api/notes', { headers: { authorization: token } });
if (reply.status === 401) await signIn();
\`\`\`

| Status | Meaning | Retry |
|:--|:--|--:|
| 200 | The notes | no |
| 401 | Signed out | once |
| 429 | Too many calls | after 30 s |

> A reply over 512 KiB is streamed. Read it once.
`;

/** Reference text: inline code, a code block that scrolls sideways, a table and a quote. */
export function Reference() {
  return <Markdown source={reference} data-testid="reference" />;
}
```

### Small

```tsx
import { Markdown } from '@liulidesign/core/Markdown';

/** Small, for a comment or a card. */
export function Comment() {
  return <Markdown size="sm" source={'Looks right to me. Two things:\n\n- the total should use ~~gross~~ **net**\n- see `summary.ts`, line 40'} data-testid="comment" />;
}
```

## Props

| Prop | Type | Default | Description |
| --- | --- | --- | --- |
| `source` (required) | `string` |  | The Markdown text: headings, paragraphs, emphasis, links, lists with tasks, quotes, code, rules and tables. |
| `onLinkClick` | `((href: string) => void)` |  | Runs with a pressed link's address; the app opens it. Without it links are drawn as plain text. |
| `topHeadingLevel` | `1 \| 2 \| 3 \| 4 \| 5 \| 6` | `2` | The heading level a `#` gets, so the text fits under the page's own headings. |
| `taskLabels` | `{ done: string; open: string; }` |  | What a screen reader hears for a task's mark: `{ done: 'Done', open: 'Not done' }`. Without it the marks are not read. |
| `isSelectable` | `boolean` | `true` | Lets people select and copy the text. |
| `size` | `MarkdownSize` | `'md'` | `sm` for a comment or a card. |
| `className` | `string` |  |  |
| `data-testid` | `string` |  |  |

## Anatomy

| Part | Element | Required | Description |
| --- | --- | --- | --- |
| root | `<view>` | yes | The blocks in a column; the text can be selected unless `isSelectable` is off. |
| heading | `<text>` | no | A heading, read as one; `#` is at `topHeadingLevel` and each further `#` one level down. |
| paragraph | `<text>` | no | A paragraph of runs. A line end in the source is a space; two spaces before it break the line. |
| run | `<span>` | no | Bold, italic, struck or code text inside a paragraph. |
| link | `<link>` | no | A link, pressable when `onLinkClick` is set. |
| list | `<view>` | no | A bulleted or numbered list; lists nest. |
| marker | `<text>` | no | An item's bullet or number, hidden from assistive tech, which counts the items itself. |
| task | `<view>` | no | A task item's mark, empty or ticked. It shows the state; it cannot be pressed. |
| quote | `<view>` | no | A quotation, set off by a rule at its start edge. |
| code | `<scroll>` | no | A code block in the fixed-width face on a tint; long lines scroll sideways instead of wrapping. |
| rule | `<view>` | no | A hairline between sections. |

## What an app may override

- `className`: classes merged after the recipe's, so they win.
- The recipe, `markdownRecipe` from `@liulidesign/core/Markdown/recipe`: the classes of each part, to style an element of your own to match. The component does not read a changed copy.
- Scale tokens it reads, set in the app's stylesheet under `@theme`: `--text-footnote`, `--text-body`, `--text-callout`, `--text-title`, `--radius-control`.
- Colour roles it paints with, set by the theme pack: `accent`, `line-strong`, `on-accent`, `text`, `text-muted`.

## Accessibility

- Pattern: none.
- Name: none; its content is read.
- Description: none.
- Tab: Moves through the links, when `onLinkClick` is set.
- Press target: .
- Headings are read as headings at their level, lists as lists whose items report their position, links as links named by their text.
- A task's mark is read as `taskLabels.done` or `taskLabels.open` before its text; without `taskLabels` the mark is not read, so pass them wherever tasks can appear.
- A table is a Table: each cell is read with its column's heading.
- Links are underlined as well as coloured, so they do not depend on colour.
- Without `onLinkClick` a link is plain text and not a control: nothing looks pressable that is not.

| Contrast | Pair | Minimum | Light | Dark |
| --- | --- | --- | --- | --- |
| text | `text` on `surface` | 4.5:1 | 16.26:1 | 13.87:1 |
| markers, struck text and small headings | `text-muted` on `surface` | 4.5:1 | 5.89:1 | 6.64:1 |
| links | `accent` on `surface` | 4.5:1 | 5.62:1 | 6.19:1 |
| inline code on its tint | `text` on `text/8` over `surface` | 4.5:1 | 13.91:1 | 11.29:1 |
| code block on its tint | `text` on `text/6` over `surface` | 4.5:1 | 14.47:1 | 11.94:1 |
| ticked task mark | `on-accent` on `accent` | 3:1 | 5.62:1 | 7.12:1 |
| empty task mark | `line-strong` on `surface` | 3:1 | 3.66:1 | 3.69:1 |

Ratios are computed from @lingxia/view/theme and rounded down.

## Guidance

- Do: Set `topHeadingLevel` to one below the heading the text sits under.
- Do: Open links in `onLinkClick`, after checking the address is one the app means to open.
- Do: Use `sm` inside a card or a comment thread.
- Don't: Build a page's own layout from Markdown; use the components.
- Don't: Pass text from strangers and open whatever it links to.
- Don't: Use it for one styled line; a `<text>` with a `<span>` does that.

## Not yet

| Missing | Needs | Meanwhile |
| --- | --- | --- |
| Images. | nothing from the View | An image is drawn as its alternative text. |
| Code coloured by its language. | nothing from the View | One colour, in the fixed-width face. |
| Editing: bold and lists while typing. | an editor that holds styled text | Viewing only; TextArea edits the plain source. |
| Raw HTML, footnotes and definition lists. | nothing from the View | Shown as the text they were written as. |
| A paragraph of more than 256 styled runs. | a higher run limit on `<text>` | Not handled; split the paragraph. |

Related: Table, Item, TextArea.

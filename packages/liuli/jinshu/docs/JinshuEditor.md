<!-- Generated from the doc file by scripts/generate.mjs; do not edit. -->

# JinshuEditor

A Jinshu document, edited block by block, with history, saving, and a review of what an agent changed or asks. Status: preview.

```ts
import { JinshuEditor } from '@liulidesign/jinshu/JinshuEditor';
```

JinshuEditor edits a Jinshu document where it is read: each block keeps the reader's geometry, so moving between reading and editing does not jump. Text blocks edit in place; tables, code, folds, callouts and media have their own editors, and `/` inserts any block. On a live `channel` the source keeps the history, the save state and the review: an agent's change is marked at each block it touched, with a bar to view or undo that batch; its suggestions and save requests wait in one queue; and an agent at work shows a mark the person can stop. Pasted paragraphs or Markdown become blocks, and in a Markdown document (`format`) the menus offer only what Markdown holds. Source mode edits the whole document as Markdown text and applies only the changed blocks, in one undo step.

## Examples

### A draft the page holds

```tsx
import { useLocal } from '@lingxia/view';
import { callout, codeBlock, createDocument, heading, list, listItem, paragraph, table } from '@liulidesign/jinshu/schema';
import { applyOps } from '@liulidesign/jinshu/schema';
import type { JinshuDocument } from '@liulidesign/jinshu/schema';
import { JinshuEditor } from '@liulidesign/jinshu/JinshuEditor';

const draft = createDocument({
  title: 'Limits',
  blocks: [
    heading(2, 'What a limit says', 'h1'),
    paragraph('A limit describes where a function heads.', 'p1'),
    list('bullet', [listItem('Definition', { id: 'i1' }), listItem('Exercises', { id: 'i2' })], { id: 'l1' }),
    callout('tip', [paragraph('Sketch the graph before you compute.', 'c1p')], { title: 'Tip', id: 'c1' }),
    table(['Week', 'Topic', 'Due'], [['3', 'Limits', 'Friday'], ['4', 'Continuity', 'Next Friday']], { id: 't1' }),
    codeBlock('ts', 'const f = (x: number) => x * x;\nf(2);', { id: 'k1', filename: 'square.ts' }),
  ],
});

/** The editor on a document the page holds: each edit comes back as ops the page applies. */
export function Draft() {
  // `useLocal` types only closed data, and a document's `meta` and extension attrs are open records.
  const doc = useLocal<any>('editor-example-doc', draft) as { value: JinshuDocument; set(next: JinshuDocument): void };
  return <JinshuEditor doc={doc.value} onOps={ops => doc.set(applyOps(doc.value, ops).doc)} hasLeaveGuard={false} data-testid="draft" />;
}
```

### Handing blocks to the app's agent

```tsx
import { useLocal } from '@lingxia/view';
import { createDocument, heading, paragraph } from '@liulidesign/jinshu/schema';
import { applyOps } from '@liulidesign/jinshu/schema';
import type { JinshuDocument } from '@liulidesign/jinshu/schema';
import { JinshuEditor } from '@liulidesign/jinshu/JinshuEditor';

const notes = createDocument({
  title: 'Meeting notes',
  blocks: [heading(2, 'Decisions', 'n1'), paragraph('Ship the reader first; the editor follows in May.', 'n2'), paragraph('Open: who owns the migration?', 'n3')],
});

/** Each block's menu offers to hand it to the app's agent; the app opens its own conversation with it. */
export function AskAgent() {
  const doc = useLocal<any>('editor-ask-doc', notes) as { value: JinshuDocument; set(next: JinshuDocument): void };
  const asked = useLocal<string>('editor-ask-ids', '');
  return (
    <view className="gap-3">
      <JinshuEditor
        doc={doc.value}
        onOps={ops => doc.set(applyOps(doc.value, ops).doc)}
        onAskAgent={ids => asked.set(ids.join(', '))}
        hasLeaveGuard={false}
        data-testid="ask-agent"
      />
      {asked.value !== '' && <text className="text-footnote text-muted">{`Handed to the assistant: ${asked.value}`}</text>}
    </view>
  );
}
```

### Read-only

```tsx
import { callout, codeBlock, createDocument, heading, list, listItem, paragraph, table } from '@liulidesign/jinshu/schema';
import { JinshuEditor } from '@liulidesign/jinshu/JinshuEditor';

const draft = createDocument({
  title: 'Limits',
  blocks: [
    heading(2, 'What a limit says', 'h1'),
    paragraph('A limit describes where a function heads.', 'p1'),
    list('bullet', [listItem('Definition', { id: 'i1' }), listItem('Exercises', { id: 'i2' })], { id: 'l1' }),
    callout('tip', [paragraph('Sketch the graph before you compute.', 'c1p')], { title: 'Tip', id: 'c1' }),
    table(['Week', 'Topic', 'Due'], [['3', 'Limits', 'Friday'], ['4', 'Continuity', 'Next Friday']], { id: 't1' }),
    codeBlock('ts', 'const f = (x: number) => x * x;\nf(2);', { id: 'k1', filename: 'square.ts' }),
  ],
});

/** Read-only: the document with no editing chrome. */
export function ReadOnly() {
  return <JinshuEditor doc={draft} isReadOnly data-testid="read-only" />;
}
```

## Props

| Prop | Type | Default | Description |
| --- | --- | --- | --- |
| `channel` | `JinshuChannelAction` |  | The page's `syncDoc` action, which Logic answers with `source.serve`: edits, history, saving, the review queue and agent changes all go through the source. |
| `docKey` | `string` |  | Which document `channel` serves. |
| `doc` | `JinshuDocument` |  | The document, held by the page, when there is no `channel`; each edit comes back through `onOps`. |
| `onOps` | `((ops: readonly AnyJinshuOp[]) => void)` |  | The ops of each edit to `doc`, for the page to apply with `applyOps`. |
| `isReadOnly` | `boolean` |  | Read-only: hides editing chrome, keeps selection and copy. |
| `isToolbarSticky` | `boolean` | `true` | Pins the single-row icon formatting ribbon at the top of the scroll container. |
| `toolbarActions` | `Child` |  | Extra quiet action(s) docked on the trailing side of the sticky ribbon. |
| `onAskAgent` | `((blockIds: readonly string[]) => void)` |  | Hands blocks to the app's agent as context; offered in each block's menu when set. |
| `hasLeaveGuard` | `boolean` | `true` | Enable `useLeaveGuard` when draft has unsaved changes. |
| `isDirty` | `boolean` |  | Unsaved changes, for the leave guard and the toolbar's Save; falls back to `saveStatus === 'dirty'`. |
| `saveStatus` | `"dirty" \| "failed" \| "saved" \| "saving"` |  | Where saving stands, as the page hears it from Logic; on a live `channel` the toolbar shows it, with Save or Retry. |
| `nav` | `JinshuNavHandle` |  | Shared with a `JinshuOutline` from `useJinshuNav()`. |
| `openAt` | `string` |  | A top-level block to open at, once: where an agent's change landed (a tool result's `page`). |
| `format` | `JinshuFormat` |  | The document's file format; on a live `channel` the source's. In `markdown` the editor offers only what Markdown holds. |
| `filePolicy` | `FilePolicy` |  | As on JinshuView. |
| `mediaPolicy` | `MediaPolicy` |  | As on JinshuView. |
| `resolveSrc` | `((src: string) => string)` |  | As on JinshuView. |
| `customBlocks` | `Readonly<Record<string, CustomBlockRenderer>>` |  | Renderers for the app's own block types, keyed by `x-scope-name`; they edit through their attrs. |
| `customInlines` | `Readonly<Record<string, InlineAtomRenderer>>` |  | Renderers for the app's own inline atoms, keyed by `x-scope-name`. |
| `onLinkPress` | `((info: { readonly blockId: string; readonly href: string; }) => void)` |  | As on JinshuView. |
| `onFileAction` | `((info: { readonly blockId: string; readonly src: string; readonly action: "download"; }) => void)` |  | As on JinshuView. |
| `className` | `string` |  |  |
| `data-testid` | `string` |  |  |

## Anatomy

| Part | Element | Required | Description |
| --- | --- | --- | --- |
| root | `<view>` | yes | The editor: its header, then the document. |
| header | `<view>` | yes | Stays in view while scrolling: the toolbar, then whatever needs the person (activity, the change bar, the review queue). |
| toolbar | `<view>` | no | The document's toolbar: insert, the app's `toolbarActions` and find; on a live `channel` also undo, redo, history and the save state. Absent when read-only. |
| saved | `<view>` | no | The quiet save state at the toolbar's end, from `saveStatus`; Save or Retry takes its place while there is something to save. |
| wordCount | `<text>` | no | How long the document is, beside the save state: Chinese, Japanese and Korean by character, other text by word. Not shown on a phone. |
| findBar | `<view>` | no | Find and replace across the document, with the match count. |
| history | `<view>` | no | The edit history: who changed what, in the reader's language, with undo and redo of each step. |
| activity | `<view>` | no | An agent reading or writing here, named, with Stop; its next edit is refused once stopped. |
| agentBar | `<view>` | no | The change bar: who changed how many blocks while the person was here or away, with View (steps through the changes) and Undo for that batch only. |
| queue | `<view>` | no | One item at a time of what waits for the person: a conflict, an agent's save request, its suggestions in document order. |
| canvas | `<view>` | yes | The document, at the reader's measure. |
| source | `<view>` | no | Source mode: the whole document as one Markdown text, parsed back into blocks on leaving, with any warnings or a refusal above it. |
| title | `<button>` | no | The document's title; pressing it edits it. |
| block | `<view>` | yes | One block, with a start-edge mark when an agent changed it. |
| gutter | `<view>` | no | On a desktop, beside the hovered block: insert below, and the block menu (move, duplicate, turn into, properties, ask the agent, delete). |
| textEditor | `<view>` | no | The field a text block becomes while edited, in the block's own type. |
| objectBlock | `<view>` | no | A table, media, divider or `x-` block. On a phone a tap selects it, outlined in accent. |
| selectionBar | `<view>` | no | Under a selected block on a phone: move, replace, properties, preview, delete, more, and deselect. |
| suggestions | `<view>` | no | An agent's suggestions for a block, each with its note, accept and reject. |
| slashMenu | `<view>` | no | The `/` menu of block types, filtered as the person types. |
| formatBar | `<view>` | no | Bold, italic, code, highlight, link and math for the text being edited; above the keyboard on a phone. |
| trailing | `<button>` | no | The empty line after the last block; pressing it starts a new paragraph. |

## What an app may override

- `className`: classes merged after the recipe's, so they win.
- The recipe, `jinshuEditorRecipe` from `@liulidesign/jinshu/JinshuEditor/recipe`: the classes of each part, to style an element of your own to match. The component does not read a changed copy.
- Scale tokens it reads, set in the app's stylesheet under `@theme`: `--text-caption`, `--text-footnote`, `--shadow-raised`, `--radius-control`, `--radius-container`, `--radius-checkbox`.
- Colour roles it paints with, set by the theme pack: `accent`, `accent-soft`, `canvas`, `danger`, `danger-soft`, `info`, `info-soft`, `line`, `line-strong`, `success`, `success-soft`, `surface`, `surface-raised`, `surface-sunken`, `text`, `text-faint`, `text-muted`, `warning-soft`.

## Accessibility

- Pattern: none.
- Name: none; its content is read.
- Description: none.
- Enter in a text block: ends it and starts a new paragraph below; in an empty list item, leaves the list.
- Arrow up and down in a text block: move to the text block above or below; in the `/` menu, move through its choices.
- Escape: leaves the field, keeping the edit; in the `/` menu, closes it.
- Enter, arrows in a table cell: Enter moves down a row and adds one at the end; up and down move between rows.
- The toolbar and a phone's selection bar are toolbars, each named; every icon button is named by its action.
- The activity mark, the change bar and the review queue are status regions, so a screen reader hears them when they appear.
- An agent's change is never shown by the start-edge mark alone: the change bar says who changed how many blocks.
- Each block's edit button is named by the block, and each history step by what it did, in the reader's language.

| Contrast | Pair | Minimum | Light | Dark |
| --- | --- | --- | --- | --- |
| document text | `text` on `canvas` | 4.5:1 | 15.19:1 | 15.31:1 |
| change bar text | `text` on `accent-soft/40` over `canvas` | 4.5:1 | 14.81:1 | 13.81:1 |
| activity text | `text` on `info-soft/40` over `canvas` | 4.5:1 | 14.81:1 | 13.81:1 |
| change mark | `accent` on `canvas` | 3:1 | 5.25:1 | 6.83:1 |
| suggestion note | `text` on `accent-soft/25` over `canvas` | 4.5:1 | 14.95:1 | 14.41:1 |
| save state | `text-muted` on `canvas` | 4.5:1 | 5.50:1 | 7.33:1 |
| find match | `text` on `warning-soft/50` over `canvas` | 4.5:1 | 14.89:1 | 13.50:1 |

Ratios are computed from @lingxia/view/theme and rounded down.

## Guidance

- Do: Edit through a live `channel`: the source keeps history, saving, the review and the leave guard's state in one place.
- Do: Let agents work through the tools the app registers with `createJinshuTools`; the editor shows their changes and requests, and has no AI entry of its own.
- Do: Set `onAskAgent` to offer "Ask the agent" in each block's menu, and open the app's own conversation with those blocks.
- Do: Give the source `media.pick` to offer images, video, audio and files; without it, the editor offers none.
- Do: Set `saveMode: 'auto'` on the source to save 1.5 s after the person stops editing; an agent's edit alone never saves.
- Don't: Apply an agent's edit in the page; it goes through the source so the change bar, undo and history see it.
- Don't: Turn off the leave guard on a page that holds unsaved work.

## Not yet

| Missing | Needs | Meanwhile |
| --- | --- | --- |
| Bold, italic and links on a selection of words in a text block. | reading and setting the selection of a text field, and buttons that leave its caret alone | A text block edits as shorthand (`**bold**`, `==mark==`, `[text](#id)`); the format bar wraps the last word. |
| Backspace at a block's start merging it into the one above, and Tab indenting a list. | Backspace and Tab in an editor's key map | An empty block is deleted from its menu; Enter on an empty list item leaves the list. |

## Limits

- History, saving, the change bar, the review queue and the activity mark need a live `channel`; a page-held `doc` has none of them.
- `openAt` brings the block to the nearest edge of the scroll, not to its top.
- A table's header row scrolls with the page; it does not stay pinned.

Related: JinshuView, JinshuDiff, JinshuOutline.

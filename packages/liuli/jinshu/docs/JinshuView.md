<!-- Generated from the doc file by scripts/generate.mjs; do not edit. -->

# JinshuView

A Jinshu document, read: text, media, tables and the app's own blocks, with resume and reading observations. Status: preview.

```ts
import { JinshuProvider, JinshuView } from '@liulidesign/jinshu/JinshuView';
```

JinshuView draws a Jinshu document for reading: headings, paragraphs, lists, callouts, folds, code, formulas, tables, images, video, audio, files and the app's own `x-` blocks. It reads a document the page holds (`doc`), or follows a live one that Logic serves (`channel` and `docKey`), so an edit made elsewhere, by a person or an agent, appears as it lands, with a bar naming an agent's change and a mark where one is writing. It can offer to continue where the reader stopped, open at one block, hold media to a policy, and report what was read and played through `onObserve`.

## Examples

### A lesson

```tsx
import { callout, codeBlock, createDocument, details, fileBlock, heading, image, list, listItem, mathBlock, paragraph, quote, run, table } from '@liulidesign/jinshu/schema';
import { JinshuView } from '@liulidesign/jinshu/JinshuView';

const lesson = createDocument({
  title: 'Limits',
  blocks: [
    heading(2, 'What a limit says', 'h1'),
    paragraph([run('A limit describes where a function '), run('heads', { bold: true }), run(', not where it is.')], 'p1'),
    image('public/photos/photo-1.png', { alt: 'The sea running out to the horizon at sunset', width: 480, height: 360, caption: 'The horizon: the sea heads toward it and never arrives.', id: 'g1' }),
    mathBlock('\\lim_{x \\to 1} \\frac{x^2 - 1}{x - 1} = 2', 'm1'),
    list('task', [listItem('Read the definition', { id: 'i1', checked: true }), listItem('Try the exercises', { id: 'i2', checked: false })], { id: 'l1' }),
    callout('tip', [paragraph('Sketch the graph first.', 'p2')], { title: 'Tip', id: 'c1' }),
    table(['Notation', 'Read as', 'Means'], [['lim x→a f(x)', 'the limit of f at a', 'where f heads near a'], ['x→a⁺', 'from the right', 'only x greater than a']], { id: 't1' }),
    quote([paragraph('The limit does not care what happens at the point itself.', 'q1p')], 'q1'),
    details('Why the hollow point?', [paragraph('Dividing by x − 1 is undefined at 1, so the graph has a hole there.', 'd1p')], { id: 'd1' }),
    codeBlock('ts', 'const f = (x: number) => x * x;\nf(2);', { id: 'k1', filename: 'square.ts' }),
    fileBlock('https://example.com/limits.pdf', 'limits.pdf', { pageCount: 12, sizeBytes: 482_000, id: 'f1' }),
  ],
});

/** A lesson: text, an image with its caption, a formula, a task list, a tip, a table, a quote, a fold, code and an attachment. */
export function Lesson() {
  return <JinshuView doc={lesson} data-testid="lesson" />;
}
```

### Continue where the reader stopped

```tsx
import { callout, codeBlock, createDocument, details, fileBlock, heading, image, list, listItem, mathBlock, paragraph, quote, run, table } from '@liulidesign/jinshu/schema';
import { JinshuView } from '@liulidesign/jinshu/JinshuView';

const lesson = createDocument({
  title: 'Limits',
  blocks: [
    heading(2, 'What a limit says', 'h1'),
    paragraph([run('A limit describes where a function '), run('heads', { bold: true }), run(', not where it is.')], 'p1'),
    image('public/photos/photo-1.png', { alt: 'The sea running out to the horizon at sunset', width: 480, height: 360, caption: 'The horizon: the sea heads toward it and never arrives.', id: 'g1' }),
    mathBlock('\\lim_{x \\to 1} \\frac{x^2 - 1}{x - 1} = 2', 'm1'),
    list('task', [listItem('Read the definition', { id: 'i1', checked: true }), listItem('Try the exercises', { id: 'i2', checked: false })], { id: 'l1' }),
    callout('tip', [paragraph('Sketch the graph first.', 'p2')], { title: 'Tip', id: 'c1' }),
    table(['Notation', 'Read as', 'Means'], [['lim x→a f(x)', 'the limit of f at a', 'where f heads near a'], ['x→a⁺', 'from the right', 'only x greater than a']], { id: 't1' }),
    quote([paragraph('The limit does not care what happens at the point itself.', 'q1p')], 'q1'),
    details('Why the hollow point?', [paragraph('Dividing by x − 1 is undefined at 1, so the graph has a hole there.', 'd1p')], { id: 'd1' }),
    codeBlock('ts', 'const f = (x: number) => x * x;\nf(2);', { id: 'k1', filename: 'square.ts' }),
    fileBlock('https://example.com/limits.pdf', 'limits.pdf', { pageCount: 12, sizeBytes: 482_000, id: 'f1' }),
  ],
});

/** A returning reader is offered their last position above the document. */
export function Resume() {
  return <JinshuView doc={lesson} resume={{ position: { blockId: 'h1', offset: 0 }, blocks: {}, media: {} }} data-testid="resume" />;
}
```

### Video and audio held to a policy

```tsx
import { audio, createDocument, paragraph, video } from '@liulidesign/jinshu/schema';
import { JinshuView } from '@liulidesign/jinshu/JinshuView';

const course = createDocument({
  title: 'Week 3: continuity',
  blocks: [
    paragraph('Watch the lecture before the exercise set; it unlocks once you have seen it through.', 'cp'),
    video(clip, {
      poster,
      durationMs: 10_000,
      caption: 'Lecture 3, recorded on 12 March.',
      chapters: [{ title: 'Definition', atMs: 0 }, { title: 'Three conditions', atMs: 3_000 }, { title: 'Worked examples', atMs: 6_000 }],
      id: 'v1',
    }),
    audio(clip, {
      title: 'Recap',
      durationMs: 10_000,
      transcript: [paragraph('A function is continuous at a point when its limit there equals its value.', 'tr1')],
      id: 'a1',
    }),
  ],
});

/** A course page whose video can only be scrubbed back over what was watched, at up to 1.5×. */
export function Course() {
  return <JinshuView doc={course} mediaPolicy={{ seek: 'played-only', maxRate: 1.5 }} data-testid="course" />;
}
```

### Files, inline or attached

```tsx
import { Said } from '@liulidesign/core/internal/example';
import { createDocument, fileBlock, paragraph } from '@liulidesign/jinshu/schema';
import { JinshuView } from '@liulidesign/jinshu/JinshuView';

const handbook = createDocument({
  title: 'Course handbook',
  blocks: [
    paragraph('Everything you need for the term, in two files.', 'hp'),
    fileBlock('https://example.com/handbook.pdf', 'handbook.pdf', { pageCount: 36, sizeBytes: 2_400_000, display: 'inline', id: 'hf1' }),
    fileBlock('https://example.com/timetable.xlsx', 'timetable.xlsx', { sizeBytes: 64_000, description: 'Spring term', id: 'hf2' }),
  ],
});

/** Attachments: the app decides which files may show inline and handles the download. */
export function Attachments() {
  return (
    <Said>
      {say => (
        <JinshuView
          doc={handbook}
          filePolicy={{ default: 'attachment', inline: 'allow' }}
          onFileAction={({ src }) => say(`Downloading ${src.split('/').pop()}`)}
          data-testid="attachments"
        />
      )}
    </Said>
  );
}
```

### No document

```tsx
import { JinshuView } from '@liulidesign/jinshu/JinshuView';

/** No document: the reader says so in place of the content. */
export function Unavailable() {
  return <JinshuView doc={null} data-testid="unavailable" />;
}
```

### Loading

```tsx
import { JinshuView } from '@liulidesign/jinshu/JinshuView';

/** Waiting for the document: placeholders where the title and the first blocks will be. */
export function Loading() {
  return <JinshuView doc={undefined} data-testid="loading" />;
}
```

## Props

| Prop | Type | Default | Description |
| --- | --- | --- | --- |
| `doc` | `JinshuDocument \| null` |  | The document, held by the page: `null` says it is unavailable, `undefined` that it is still loading. Leave it out to read a live `channel`. |
| `channel` | `JinshuChannelAction` |  | The page's `syncDoc` action, which Logic answers with `source.serve`: the reader follows the live document. |
| `docKey` | `string` |  | Which document `channel` serves. |
| `nav` | `JinshuNavHandle` |  | From `useJinshuNav()`, shared with a `JinshuOutline`: pressing a heading scrolls the reader to it. Give `nav.onVisibleChange` to the `<scroll>` holding the reader to follow the heading being read and report dwell and position. |
| `resume` | `JinshuProgress` |  | Saved progress: offers to continue at its position and resumes media where it stopped. |
| `openAt` | `string` |  | A top-level block to open at, once: where an agent's change landed (a tool result's `page`). |
| `onObserve` | `((events: readonly Observation[]) => void)` |  | What was read and played, in batches: played media ranges and events from custom blocks; dwell per block and the reading position once `nav` hears the scroll. Facts only; the app decides what counts as done. |
| `filePolicy` | `FilePolicy` |  | Whether files may show inline or only as attachments. |
| `mediaPolicy` | `MediaPolicy` |  | How media may be scrubbed (`any`, `played-only`, `none`) and the fastest rate offered. |
| `resolveSrc` | `((src: string) => string)` |  | Turns a block's `src` into what the page loads: a signed URL, a custom scheme. |
| `customBlocks` | `Readonly<Record<string, CustomBlockRenderer>>` |  | Renderers for the app's own block types, keyed by `x-scope-name`. |
| `customInlines` | `Readonly<Record<string, InlineAtomRenderer>>` |  | Renderers for the app's own inline atoms, keyed by `x-scope-name`. |
| `hasUpdateHighlights` | `boolean` |  | Tints the blocks a live update changed, briefly. |
| `hasAgentFeedback` | `boolean` | `true` | On a live `channel`, shows what agents do: a mark where one is writing, and a bar naming its latest change, with View and Dismiss, over the blocks it marks. |
| `onLinkPress` | `((info: { readonly blockId: string; readonly href: string; }) => void)` |  | A link to outside the document was pressed; in-page links scroll on their own. |
| `onFileAction` | `((info: { readonly blockId: string; readonly src: string; readonly action: "download"; }) => void)` |  | A file's Download was pressed; without it, the button does nothing. |
| `onCopyCode` | `((code: string, blockId: string) => void)` |  | Puts the code on the clipboard; a code block offers Copy only when set. |
| `isSelectable` | `boolean` | `true` | Text can be selected and copied, up to the page's budget of selectable nodes. |
| `hasTitle` | `boolean` | `true` | Shows the document's title above its blocks. |
| `isFullWidth` | `boolean` | `false` | Fills its container instead of a 65-character reading measure. |
| `fallback` | `Child` |  | Drawn in place of the document when it is unavailable. Defaults to one line in the page's language. |
| `className` | `string` |  |  |
| `data-testid` | `string` |  |  |

## Anatomy

| Part | Element | Required | Description |
| --- | --- | --- | --- |
| root | `<view>` | yes | The column of blocks, a 65-character reading measure unless `isFullWidth`. |
| resume | `<view>` | no | With `resume`: the bar above the document naming where the reader stopped, with Continue and Start over. |
| agentBar | `<view>` | no | On a live `channel`: an agent's latest change, by name and count, with View, which steps through the changed blocks, and Dismiss; held at the top while reading. Changed blocks carry a bar at their start edge. |
| activity | `<view>` | no | On a live `channel`: a mark above the block an agent is writing in, or under the agent bar for the whole document. |
| docTitle | `<text>` | no | The document's title, a level-one heading; hidden by `hasTitle={false}`. |
| heading | `<text>` | no | A heading block, levels one to four. |
| paragraph | `<text>` | no | Text in runs: bold, italic, code, highlights, links, inline math and the app's atoms. |
| quote | `<view>` | no | A quotation, set in from a rule at its start edge. |
| callout | `<view>` | no | A tinted note in one of four tones, with an optional title in the tone. |
| list | `<view>` | no | A bulleted, numbered or task list; a task's box is checkable when the app handles it. |
| code | `<view>` | no | Code under a header of file and language, with line numbers, highlighted lines, and Copy when `onCopyCode` is set; long lines scroll sideways. |
| math | `<view>` | no | A display formula, named by its LaTeX for assistive tech. |
| table | `<view>` | no | A framed grid; columns are at least 9em, and a table wider than its frame scrolls sideways and fades at the edge it runs under. |
| divider | `<view>` | no | A hairline between sections. |
| image | `<view>` | no | A picture with its caption; pressing it opens a lightbox. A blocked or failed source says so, with Retry. |
| video | `<view>` | no | A player with Liuli's video controls, chapters and caption, held to `mediaPolicy`. |
| audio | `<view>` | no | An audio card: play, rate, time and a scrubber, then chapters and a folded transcript. |
| file | `<view>` | no | A file card: type badge, name, pages, size, and Download. |
| columns | `<view>` | no | Blocks side by side; stacked on a phone or at a large text size. |
| extension | `<view>` | no | An `x-` block the app gave no renderer for: its fallback text in a dashed box. |

## What an app may override

- `className`: classes merged after the recipe's, so they win.
- The recipe, `jinshuViewRecipe` from `@liulidesign/jinshu/JinshuView/recipe`: the classes of each part, to style an element of your own to match. The component does not read a changed copy.
- Scale tokens it reads, set in the app's stylesheet under `@theme`: `--text-caption`, `--text-footnote`, `--text-body`, `--text-callout`, `--text-title`, `--text-display`, `--radius-control`, `--radius-container`, `--radius-checkbox`, `--radius-segment`.
- Colour roles it paints with, set by the theme pack: `accent`, `accent-soft`, `accent-solid`, `canvas`, `danger`, `danger-soft`, `info`, `info-soft`, `line`, `line-strong`, `on-accent`, `on-accent-solid`, `success`, `success-soft`, `surface`, `surface-sunken`, `text`, `text-faint`, `text-muted`, `warning`, `warning-soft`.

## Accessibility

- Pattern: none.
- Name: none; its content is read.
- Description: none.
- The title and each heading are headings at their level, so a screen reader can move between sections.
- Lists are lists of items; a task's box is a checkable item named by its text.
- Images are named by `alt`, then the caption; a formula is named by its LaTeX; media controls are named by the block's title or caption.
- The agent bar and the writing mark are status regions, so a change or an agent starting to write is announced without moving focus.
- Selection is budgeted per page: past the budget, further text renders unselectable rather than slowing the page.

| Contrast | Pair | Minimum | Light | Dark |
| --- | --- | --- | --- | --- |
| body text | `text` on `canvas` | 4.5:1 | 15.19:1 | 15.31:1 |
| quote and caption | `text-muted` on `canvas` | 4.5:1 | 5.50:1 | 7.33:1 |
| link | `accent` on `canvas` | 4.5:1 | 5.25:1 | 6.83:1 |
| text in a tip | `text` on `success-soft` | 4.5:1 | 14.33:1 | 11.76:1 |
| tip title | `success` on `success-soft` | 4.5:1 | 4.68:1 | 6.45:1 |
| warning title | `warning` on `warning-soft` | 4.5:1 | 4.84:1 | 6.70:1 |
| danger title | `danger` on `danger-soft` | 4.5:1 | 4.53:1 | 4.77:1 |
| inline code | `text` on `text/8` over `canvas` | 4.5:1 | 13.01:1 | 12.79:1 |
| code line numbers | `text-muted` on `surface-sunken` | 3:1 | 5.20:1 | 7.45:1 |
| table header | `text` on `surface-sunken` | 4.5:1 | 14.36:1 | 15.55:1 |
| resume bar | `text` on `accent-soft/50` over `canvas` | 4.5:1 | 14.72:1 | 13.40:1 |
| agent bar | `text` on `accent-soft/40` over `canvas` | 4.5:1 | 14.81:1 | 13.81:1 |
| writing mark | `text` on `info-soft/40` over `canvas` | 4.5:1 | 14.81:1 | 13.81:1 |

Ratios are computed from @lingxia/view/theme and rounded down.

## Guidance

- Do: Hold the document in Logic and pass `channel` and `docKey` when anything else can change it; pass `doc` for a fixed one.
- Do: Pass `resume` from the progress you stored, so a returning reader is offered their place.
- Do: Use `mediaPolicy` to keep a required video from being skipped; say so near it in the page if it matters.
- Do: Keep what `onObserve` reports and decide completion in Logic; the reader reports facts, not grades.
- Do: Pass `nav.onVisibleChange` from `useJinshuNav()` to the `<scroll>` holding the reader, with the same `nav` on the reader, for dwell, position and pausing media that scrolls away.
- Don't: Rebuild the document in the page to change one block; send ops through the source.
- Don't: Put interactive app content in a block as text; register it as an `x-` block with `customBlocks`.

## Not yet

| Missing | Needs | Meanwhile |
| --- | --- | --- |
| A table's header row staying in view while the page scrolls down. | a sticky box that binds to the nearest scroll on its own axis | The header row scrolls with the table; it stays aligned with its columns as the table scrolls sideways. |

## Limits

- A reader shows an agent's changes but takes none back and reviews nothing: Undo, Stop and the review queue are the editor's. A change made while no page showed the document is told to the first reader to open it, once.
- `openAt` brings the block to the nearest edge of the scroll, not to its top.
- Dwell is measured per top-level block from scrolling alone: time with the page hidden or the reader away shows as one long `idleMs` stretch, and time since the last scroll is not reported when the page closes.
- Every block is mounted; the page's scroll belongs to the app, so the reader does not mount blocks by viewport.
- It has no AI entry of its own: an agent reaches documents only through the tools the app registers with `createJinshuTools`.

Related: JinshuEditor, JinshuOutline, JinshuDiff, VideoControls, Lightbox.

# @liulidesign/jinshu

Jinshu (锦书) is a block document that people and AI agents edit together: a JSON format with stable ids,
a Logic-side source that owns the draft, saving, history and review, and GPU page components that read,
edit and compare it. It follows `@liulidesign/core` in every convention: one import path per component,
a recipe beside each, colours from the app's theme roles, words from a catalogue the app can override.

Status: preview. The schema and source are unit-tested in Node; each component has a reference page in
`docs/`, and `jinshu-showcase` in the repository's examples is a document app built on it.

## What it needs

- A GPU View lxapp with `@lingxia/view` and `@liulidesign/core`.
- The document lives in Logic. Pages only render it and send edits back over a channel, so several pages
  can show one document and stay in step.

## Use

In the page's Logic, create a source and hand its channel to the page:

```ts
import { createJinshuSource } from '@liulidesign/jinshu/source';

const source = createJinshuSource({
  key: 'lesson-42',
  load: () => api.load('lesson-42'),                          // { doc, version }
  save: (doc, baseVersion) => api.save('lesson-42', doc, baseVersion),
  storage: {                                                  // unsaved drafts survive a crash
    get: key => lx.storage.get(key),
    set: (key, value) => lx.storage.set(key, value),
    remove: key => lx.storage.delete(key),
  },
});

definePage(page => ({
  actions: { syncDoc: (_params, channel) => source.serve(channel) },
}));
```

In the page, give the channel to a reader or an editor:

```tsx
import { JinshuEditor } from '@liulidesign/jinshu/JinshuEditor';

export function render({ actions }) {
  return (
    <page>
      <JinshuEditor channel={actions.syncDoc} docKey="lesson-42" />
    </page>
  );
}
```

`JinshuView` reads the same channel, or takes a `doc` directly when nothing edits it.

<!-- components:start -->
4 components, each at its own import path:

| Import | Exports | What it is |
| --- | --- | --- |
| `@liulidesign/jinshu/JinshuView` | JinshuProvider, JinshuView | A Jinshu document, read: text, media, tables and the app's own blocks, with resume and reading observations |
| `@liulidesign/jinshu/JinshuEditor` | JinshuProvider, JinshuEditor | A Jinshu document, edited block by block, with history, saving, and a review of what an agent changed or asks |
| `@liulidesign/jinshu/JinshuDiff` | JinshuDiff | What changed between two versions of a Jinshu document, block by block, with words struck and inserted |
| `@liulidesign/jinshu/JinshuOutline` | useJinshuNav, extractOutline, JinshuOutline | A Jinshu document's headings as a contents list that moves the reader and shows each section's progress |
<!-- components:end -->

| Import | What it is |
| --- | --- |
| `@liulidesign/jinshu/schema` | The format: types, builders (`paragraph`, `heading`, `table`…), `checkDocument`, `applyOps`, markup, diff, `defineBlock` |
| `@liulidesign/jinshu/source` | Logic side: `createJinshuSource`, `createJinshuTools`, `bindJinshuPage` |

The reference for each component is in `docs/<Name>.md` and the format in `docs/format.md`, both
generated, with `catalog.json` holding the components for agents. `docs/design.md` explains the rules
behind them.

## The format

A document is a list of typed blocks: paragraph, heading, quote, callout, details, list, code, math,
table, columns, divider, image, video, audio, file, and custom `x-<scope>-<name>` blocks. Text is a
list of flat runs whose marks are keys (`{ text, bold: true, link }`), never a nested tree.

- Every block, list item, table row, column and inline atom has an id, so an edit, a link, a comment or
  a reading position points at content that moves without breaking.
- Agents see the document as Jinshu Markup, an XML subset that reads like the document itself, ids
  included, and converts to and from the JSON without loss.
- Unknown keys are kept, and a document from a newer format version opens read-only instead of being
  rewritten. There is no migration step.

`docs/format.md` lists every field, its type, default and allowed values.

## Markdown or JSON

`format` picks the file a document is stored as. `json` (the default) holds every block and mark;
`markdown` holds what ordinary Markdown can say, for files people also open elsewhere.

```ts
createJinshuSource({
  key: 'notes',
  format: 'markdown',
  initialText: '# Notes\n\nFirst line.\n',
  load: () => api.load('notes'),                              // { text, version } or { doc, version }
  save: (doc, baseVersion, file) => api.save('notes', file.text, baseVersion),
});
```

- `save` receives the file text (`file.text`) beside the document; `source.file()` returns the same
  text without saving. A Markdown file keeps the bytes of every block that did not change.
- In a Markdown document, edits outside the profile are refused with `FORMAT`, and the editor offers
  only what fits. The profile is the Markdown section of `docs/format.md`.
- Markdown has no ids: they are derived on each read, kept for blocks that did not change. An id of
  a block whose text changed outside Jinshu is new, so a link, comment or reading position on it is lost.

## Edits, history and review

Every change is an operation batch (`insert`, `replace`, `edit`, `update`, `move`, `delete`, table row
and column ops, text and mark ops), applied atomically and recorded with its origin: a person, an agent,
or the app. A batch that fails validation changes nothing and returns its problems.

- **Undo** goes by batch and by origin, so a person can undo an agent's change without losing their own.
- **Suggestions**: an agent edit in `suggest` mode waits for review in place, accepted or rejected
  one at a time or all at once. An agent edit to the block a person is editing becomes a suggestion
  on its own, as do rewrites and edits that delete more than 30% of the document.
- **Saving**: `saveMode` is `manual` or `auto`; `agentSave` decides whether an agent may save
  (`allow`), must ask (`confirm`), or may not (`deny`). A version conflict is resolved as mine, theirs
  or a copy.
- **Streaming**: `source.stream()` writes blocks as an agent produces them; aborting keeps what arrived
  as a suggestion.

## Agents

`createJinshuTools(sources)` gives five tools with fixed names and parameters, the same in every app:
`jinshu_list`, `jinshu_read`, `jinshu_edit`, `jinshu_undo`, `jinshu_save`. Register their
`definitions` with your model and route calls to `tools.call(name, args, origin)`. `jinshu_read`
returns markup with ids, optionally just the outline, a range, a search, or what changed since a
revision; `jinshu_edit` needs the revision it read (`base`) and returns problems instead of throwing.

## Images, video, audio and files

The app decides where media comes from: a course library, the photo picker, a recorder, a cloud drive.
Without a `media` adapter the editor offers no media entries.

```ts
const source = createJinshuSource({
  key: 'lesson-42',
  media: {
    async pick({ kind }) {
      const res = await lx.pick.media({ types: [kind], limit: 1 });
      return res.status === 'ok' ? { file: res.files[0] } : null;     // or { src, width, height } from your library
    },
    async upload(file, onProgress) {                                   // optional; without it the local path is kept
      return api.upload(file.path, onProgress);                        // { src, width, height, … }
    },
  },
});
```

The block shows the local file at once and the upload's progress, then switches to the uploaded `src`.

## Custom blocks

```ts
import { createRegistry, defineBlock } from '@liulidesign/jinshu/schema';

const quiz = defineBlock<{ question: string; options: string[]; answerIndex: number }>({
  type: 'x-school-quiz',
  version: 1,
  attrsSchema: { type: 'object', required: ['question', 'options', 'answerIndex'], properties: { /* … */ } },
  content: 'none',
  aiDescription: 'A single-choice question with one correct answer.',
  fallbackText: attrs => attrs.question,
});

createJinshuSource({ key, registry: createRegistry({ blocks: [quiz] }) });
```

Render it with `customBlocks={{ 'x-school-quiz': QuizView }}` on `JinshuView` and `JinshuEditor`. The
agent tools describe it to the model from `aiDescription` and its schema. A reader without the renderer
shows `fallbackText`.

## Conventions, beyond core's

- The document is data: the app formats nothing into it, and nothing in it depends on the platform.
- Every edit has an origin; agents never write around the source.
- The reader keeps a person's place: a block that did not change keeps its playback position, open
  state and scroll offset when an update arrives.

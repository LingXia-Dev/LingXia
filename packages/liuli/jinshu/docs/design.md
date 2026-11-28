# Jinshu design

The rules behind the format, the source and the components that the code alone does not explain. The
field-by-field format is in `format.md`; each component's props are in `docs/<Name>.md`.

## What Jinshu owns, and what the app owns

| | Jinshu | The app |
| --- | --- | --- |
| Resources | A block holds `src` (relative path, `https:`, `lx://`, or the app's own scheme) and display metadata; `resolveSrc` maps it at render time | Asset tables, transcoding, signed URLs |
| Choosing and uploading media | Asks the app's `media.pick` and `media.upload`, shows the local file and the progress | Where media comes from and where it is stored |
| Reading and watching | Reports facts: time a block was on screen, the ranges of media actually played, the reading position. `reduceObservations` adds them up | Whether that counts as read or watched, where progress is kept |
| Saving | Draft, revision, conflict detection, crash recovery | The server and its versions |

The document holds no executable content and no styling: no colours, fonts, sizes, alignment or
spacing. The theme and Jinshu's typographic rhythm decide how it looks, which is why a document an agent
wrote still looks right. Emphasis uses `highlight` with a semantic tone.

`@liulidesign/core/Markdown` stays the small read-only component for chat bubbles and short notes;
anything long, with media, live updates or editing uses Jinshu.

## The format

### What may contain what

| Container | Children |
| --- | --- |
| Document | Any blocks, at least one; an empty document is one empty paragraph |
| `paragraph`, `heading`, captions, callout `title`, details `summary`, table cells | Inlines; no line break in a heading |
| `quote` | At least one block; no `heading` or `columns` |
| `callout`, `details` | At least one block; no `columns` or `details` |
| List item | `runs`, and optionally child blocks: `paragraph`, `list`, `code`, `image`; at most 6 list levels |
| `columns` | 2 or 3 columns of at least one block each; no `columns` inside |
| Audio `transcript` | Paragraphs only |
| `code`, `math` | Plain text, no marks |

Headings are levels 1–4 and the document `title` is metadata, rendered on its own. A skipped heading level
is a warning, not an error.

### Marks

| Mark | Continues when typing at its edge | Excludes | Removed in |
| --- | --- | --- | --- |
| `bold` | yes | | headings (already bold) |
| `italic`, `underline`, `strike`, `highlight` | yes | | |
| `script` (`sub` or `super`) | yes | | headings |
| `code` | no | `highlight`, `script` | |
| `link` | no | | |

Line breaks, inline math and inline atoms carry no marks. Nesting in markup is always
`a > mark > b > i > u > s > sub/sup > code`.

### Values and limits

- Ids match `[a-z0-9_-]{1,32}` and are unique across blocks, list items, columns, table rows and inline
  atoms. Whoever creates content makes its id, at random rather than in sequence.
- Links: `https:`, `http:`, `mailto:`, `tel:` and in-page `#id`. Media `src`: no `javascript:`, `data:`,
  `file:` or `vbscript:`, and no relative path that climbs out of the root; `allowedSrcSchemes` narrows it
  further, and the URL `resolveSrc` returns is checked the same way.
- Image `width` and `height` are the source pixels, used only to reserve space before loading; `size`
  decides how large it shows. `widths` are relative and have one entry per column.
- A document has at most 5,000 blocks and 2 MB of canonical JSON, and a block at most 50,000 characters.

### Canonical form

Two documents mean the same thing exactly when their canonical forms are equal, and storage, comparison
and live updates all use it. `normalizeDocument` produces it: adjacent runs with the same marks merge,
empty runs go, marks are written in a fixed order, a field equal to its default is left out, mark
combinations the table above forbids are dropped, `\r\n` in code and math becomes `\n`, and list numbers
come from `start` instead of being stored per item. It never rewrites `version` and never drops a key it
does not know.

### Text offsets

A position inside a block is a UTF-16 code unit offset, as in JS strings. A line break, an inline math and
an inline atom each count as one. A valid position never splits a surrogate pair. Selections, text
operations and suggestions all use this unit.

### How ids survive edits

| Edit | Ids |
| --- | --- |
| Split (Enter) | The first half keeps the id, the second gets a new one; Enter at the end of a heading makes a paragraph |
| Merge (Backspace at the start) | The earlier block keeps its id |
| Paste | New ids; cut and paste within one document keeps them |
| Change type (paragraph, heading, quote, list, callout, code) | Keeps the id; refused with `INVALID` when it would lose content, saying what |
| Delete | The id is never reused |

Reading progress, in-page links and agent edits all point at ids, so this table is part of the format.

### Markdown files

A Markdown file is another encoding of the same model, read on load and written on save. It holds a
strict profile: CommonMark with GFM tables, task lists, strikethrough and autolinks, `$math$` and `$$`
blocks, and the alert syntax `> [!NOTE]` for callouts. Jinshu adds no syntax of its own, so any
Markdown tool reads the file the same way. The document title is a leading plain `# Title` line, front
matter is kept verbatim in `meta.frontMatter`, and raw HTML stays as literal text with a warning. The
generated reference lists what the profile holds and what each block loses.

The file has no ids. Reading derives each id from the block's content, and, given the previous
document, keeps the ids of unchanged blocks and of same-type blocks in the same place between them. The
cost: a block that is both edited and moved outside Jinshu gets a new id, so anything pointing at it is
lost. Writing what Jinshu itself wrote and reading it back gives the same document and the same bytes.

## The source

### Draft and saved version

The draft lives in Logic and changes with every edit, by a person or an agent; the saved version lives
on the server and changes only through `save()`. Saving sends the draft's revision with the
`baseVersion` it was loaded from. Editing may continue while a save is in flight: `dirty` stays true
until a save covers the latest revision. A version mismatch is a conflict, resolved as mine (overwrite),
theirs (drop the draft) or a copy. Loading, restoring a draft and taking theirs replace the draft
wholesale: the history starts again there, and a batch based before it is stale.

With `storage`, the draft, its base version, pending suggestions and, for Markdown, the base file's bytes
are written locally after each change and offered back after a crash. A stored draft that no longer validates is discarded.

With `saveMode: 'auto'`, the source saves 1.5 s after the person's last edit or accepted suggestion. It
never saves while a conflict is open, waits for a running upload to finish, and does not save after an
agent's edit alone (`agentSave` decides that). A failed save shows as failed and is retried on the next
edit.

### One file format per document

A source has a `format`, `json` (the default) or `markdown`. `load` may return the file's text, which
is parsed in that format, and `save` receives the draft serialized in it beside the document. A
Markdown document holds only what Markdown can: an op that adds anything else is refused with
`FORMAT`, and the editor does not offer it. Parsing a Markdown file again keeps the ids of blocks that
did not change, so reading positions and agent edits still point at them.

### One pipeline for every edit

Every change, from the editor, an agent, or the app, is an `OpBatch` with a `batchId`, an origin and the
revision it was based on. The source applies a batch in this order, and any failure rolls back the whole
batch:

1. Ops on the blocks a person is editing (`focusIds`, the union over open editors; an editor's focus
   goes when its channel closes) are turned into suggestions when they come from an agent.
2. With a template, only its slots may change.
3. `beforeApply`, once per op; it may accept, reject with a reason, or rewrite the op.
4. `checkDocument` on the result, including `allowedSrcSchemes`.
5. `validateBatch`, for the app's own rules over the whole result.

An agent batch that rewrites the document or deletes more than 30% of it becomes a suggestion unless
`guardLargeAgentEdits` is false.

### People and agents at once

- **Per-block rebase.** Ops address blocks by id. If a target block has not changed since the batch's
  `base`, the op applies; if it has, the op fails with `STALE_BLOCK` and the block's current markup, so the
  agent re-reads and retries; a deleted target fails with `NOT_FOUND`.
- **The block a person is editing is never rewritten under them.** Agent edits to it become suggestions,
  and the editor keeps local input first if an update races its focus report.
- **Undo by origin.** Cmd+Z undoes a person's own edits only; each agent batch has its own undo in the
  history. Undoing a batch whose blocks were changed later by someone else is refused, unless forced.
  Rapid edits by a person to one block within 500 ms are one undo step. History keeps each batch with
  the ids it minted written in, so undo, redo and replaying later batches give the same ids.
- **Streaming.** `source.stream()` parses markup block by block as it arrives; a block is final at its
  closing tag. One stream per document at a time. `abort()` keeps the finished blocks as a suggestion.

### Suggestions

A suggestion is either whole-block ops (`kind: 'block'`) or a text range with its replacement
(`kind: 'range'`). Agents never write offsets: an `edit` in `suggest` mode is run on a copy and the smallest
changed range becomes the range suggestion. An agent's new suggestion replaces its own pending one on the
same range; an overlapping one from another origin is refused. When a person changes a block, range
suggestions on it are dropped, and block suggestions on it become `stale` and can only be rejected.
Accepting creates a human batch with `acceptedFrom`, so it can be undone and still shows who proposed it.
Each accept or reject is recorded with a revision, and `jinshu_read({ since })` returns the ones after it.

### Templates

A template is a document with named slots: `inline` (fill text), `blocks` (fill a container's children,
with `maxBlocks`) or `attrs` (set fields, such as a video's `src`). With a template, agents can change slot
blocks only; ops elsewhere are refused with the reason.

### Agents

Jinshu's agent surface is an API, not an integration: `createJinshuTools({ documents })` returns the
lxapp agent tools (name, summary, description, input schema, effect per call, `run`) that the app
registers with `lx.agent.provide` at launch. Whatever runs the agent reaches them through the host; the
tools do not know which engine or transport is on the other side, and take the caller's identity from
the host, never from arguments. Agents never edit through files: an open document's
truth is the Logic draft, files would need hand-written ids and runs, paths belong to the lxapp's
sandbox, and many documents are not files.

The tools follow documents, not pages. A document's draft lives in Logic for as long as the app runs,
opened by key on demand; a page is one viewer of it, and a page and an agent that open the same key share
one draft. The app supplies the catalogue:

```ts
const documents = {
  list: query => api.searchLessons(query),   // [{ key, title }]
  open: key => sourceFor(key),               // the shared source a page would also use
  pageOf: key => `pages/lesson?id=${key}`,   // optional: where a person opens it
};

export default defineApp({
  onLaunch() {
    // Registered once at launch: the tools do not depend on any page being open.
    lx.agent.provide(createJinshuTools({ documents }));
  },
});
```

`jinshu_list` searches that catalogue and says for each document whether it is on screen, has unsaved
changes or has items waiting for the person. Either order works:

- **Page first.** The agent sees where the person is, its changes appear live, and the person approves
  them in place.
- **Agent first.** The agent finds, reads and edits a document nobody has open. Its changes wait in the
  draft, which survives a restart, and a save request waits for the person. Tool results carry the
  document's page so the agent can offer to open it; opening it shows what changed and what is waiting.

`jinshu_read` wraps the document in an explicit untrusted-data boundary, since a document may contain
text written to steer a model. What an agent can do is limited to the five tools, `beforeApply`,
`validateBatch` and `agentSave`. A read longer than 32,000 characters returns the outline and asks for a
`range` or `search`.

The page never offers an AI menu: the person talks to the agent elsewhere. The page's part is to show
what the agent does and to collect the person's decisions.

- **Where the person is.** `jinshu_read` includes the blocks under the person's caret or selection, so
  "make this paragraph shorter" needs no pointing. A block's menu can hand blocks to the agent as context
  through the app's `onAskAgent`.
- **What the agent is doing.** While an agent reads a range or streams into a place, that place shows
  the agent's name and that it is working, with a stop control for a stream.
- **What changed.** Blocks an agent changed directly are highlighted, and a bar pinned under the
  document bar names the agent and the number of changes, with "view", which steps through the changed
  blocks, and "undo this change", which undoes that batch only. A document that was not on screen shows
  the agent's changes when it next opens.
- **In a reader.** The same bar and block marks, held at the top of the page's scroll, with View and
  Dismiss but no Undo, and a writing mark without Stop: a reader sees what the agent does and decides
  nothing. "Reading" marks are not shown. `hasAgentFeedback={false}` turns it all off.
- **Undo belongs to the document.** The person takes back an agent's change in the page, where they see
  it; the agent's own UI offers no undo button. In conversation the person asks the agent, which calls
  `jinshu_undo` on its own batches. Either way there is one history.
- **What needs the person.** Pending suggestions, an agent's save request, edits turned into suggestions
  by the large-edit guard, and save conflicts are one queue in one bar, reviewed in place one by one or
  all at once. Nothing an agent asks for opens a modal.
- **What the person decided.** Every accept, reject, approval, refusal, Stop, and undo or redo of an
  agent's direct change is recorded with a revision; `jinshu_read({ since })` returns them, with the
  `batchId` of a taken-back change and what is still waiting, so the agent does not build on a change
  the person took back or redo it.

### Media

Picking and uploading are Logic abilities, so the editor asks the source over its channel and the source
calls the app's `media.pick`. A result with `src` becomes a block at once. A result with a local `file`
becomes a block pointing at the local file while `media.upload` runs, with progress sent to the page; the
uploaded `src` then replaces it without entering the undo history, and replaces the local file wherever
history recorded it, so undo, redo and replay never bring back a dead local path. A block no longer
showing the local file, such as a replacement undone during the upload, is left as it is.

## Pages

### Transport

A read-only page passes the document as data: `<JinshuView doc={doc} />`. Once a draft, an agent or an
editor is involved, the page passes the source's channel action instead (`channel` and `docKey`, or
`JinshuProvider` for several components), and the source serves snapshots, block changes, suggestions,
streams, history, upload progress and where saving stands (`saveState`) over it, so an editor on the
channel needs no flags from the page; `bindJinshuPage` also puts `dirty` and `saveStatus` in the page's
data for the page's own chrome. "Save and leave" puts open fields into the document and leaves only once
that save lands; a conflict or failure stays on the page. A reopened channel sends the revision it has; the source answers with the changes
since then or a snapshot. `doc` and `channel` are exclusive. A page opens its channel as an `editor` or a
`reader`. Both are sent an agent's change as it lands. A change made while no editor was open waits for
the next editor, which can undo it; one made while no page was open is also told once to the first
reader, unless an editor showed it first or it was undone meanwhile.

### Live updates

A new document is matched to the old one by block id, never rebuilt. A block that did not change keeps
its mounted view, so a playing video keeps playing, an open `details` stays open and the scroll position
holds. Changed blocks update in place.

### Long documents and engine limits

The page's scroll belongs to the app, so the reader mounts every block. Wired to that scroll through
the nav (`nav.onVisibleChange`), it pauses media whose block scrolls off screen; the runtime pauses all
playback while the page is hidden.

The View caps how many selectable text nodes a page may have, so selection is budgeted across the mounted
blocks, and secondary labels are not selectable. Code is drawn one line per text node, which keeps each
node under the 256-run limit. Diffs are computed in Logic with `diffDocuments` and sent to the page, not
computed in the View; Chinese text is compared by word.

### Observation

`JinshuView` reports facts in one callback, `onObserve(events)`; the app decides what they mean. Dwell
and position come from the scroll holding the reader, which reports to it through `nav.onVisibleChange`.

- `dwell`: how long a top-level block was on screen, the largest share of it shown at once (`seenRatio`)
  and the longest stretch in that time without scrolling (`idleMs`). A block reports when it leaves the
  screen, or at the next scroll once it has gathered a second. The View sees only scrolling: time with
  the page hidden or the person away is a long `idleMs` stretch, which the app caps as it sees fit.
- `played`: a continuous playback range, sent when it ends (pause, seek, end, leave), with the rate and
  `ended` on the last one.
- `position`: where the reader is, as the top-level block at the top of the screen and how far past its
  start, so it still points at the same place after the document changes.
- `block`: an event a custom block emitted, such as a quiz answer.

Each event carries a session id, new each time the reader mounts, and a sequence number.
`reduceObservations(progress, events)` drops duplicates, merges played ranges and keeps the latest position; `playedRatio` and `seenRatioOf` read the
result. Media progress records its `src`, so replacing a video does not inherit the old one's progress.
`resume` is read once when the view mounts.

The View has no page-hide or unload signal: a played range ends when playback pauses, which hiding the
page does, but dwell since the last scroll is not reported when the page closes.

`mediaPolicy` limits playback per page, not per document: `seek` (`any`, `played-only`, `none`) and
`maxRate`; `played-only` starts from the ranges in `resume` and grows as the person watches, fullscreen
included. Playback always pauses when the page is hidden.

`filePolicy` decides how file blocks show: `default` for a block that does not say, and `inline: 'never'`
to show every file as an attachment card whatever the block says. A file that cannot be shown inline,
such as an archive, is a card either way. It is configured on the source and
sent to the page, so the agent tools and the page agree.

### Navigation

`useJinshuNav()` returns a data handle shared by `JinshuOutline` and `JinshuView`: the outline lists the
headings, marks the current one and takes an optional `status` per heading (`done`, `current`, `locked`)
that the app decides; `nav.scrollTo(id)` scrolls to any block. With `nav.onVisibleChange` on the scroll
holding the reader, the current heading follows scrolling. An in-page `#id` link scrolls with or without
a nav; a list item, table row or column scrolls to itself, an inline atom to the block holding it.

## Editor interaction

The editor and the reader share one block renderer, so a document looks the same while it is edited.
Editing chrome adapts to the pointer, not the platform:

- **Fine pointer**: block handles appear on hover (`+` inserts below; the handle opens turn-into, move,
  duplicate, properties, ask the agent and delete), and a compact format bar floats above selected
  text; link and equation also sit in the toolbar, for a bare caret.
- **Coarse pointer**: no floating bar over the text, which would collide with the system's own copy and
  paste menu. A format bar sits on top of the keyboard with a Done button at its end, and a tap selects
  a block that is not text, with its own bar and a "more" menu.
- `/` at the start of a paragraph opens the block menu, filtered by Chinese, English or pinyin initials.
- A text block edits as shorthand (`**bold**`, `*italic*`, `==mark==`, `[text](url)`, `$x$`); the
  format bar wraps the last word. Input rules at the start of a paragraph turn it into a block: `# ` to
  `#### `, `- `, `1. ` or `1) `, `- [ ] ` and `- [x] `, `> `, `> [!NOTE]` (and `TIP`, `IMPORTANT`,
  `WARNING`, `CAUTION`) for a callout, `$$`, ```` ``` ````, `---`, and a `| a | b |` row then Enter for
  a table. JSON documents also take `!> ` for a callout.
- Enter starts a new paragraph below. In a list it adds an item; on an empty item it ends the list
  there, the item becoming a paragraph, in one undo step. Arrow up and down move to the text block
  above or below.
- The toolbar counts the document's length beside the save state: Chinese, Japanese and Korean by
  character, other text by word.
- A paste from a web page or a word processor keeps headings, lists, quotes, code, tables, links and
  marks; pasted Markdown or text with paragraph breaks does the same. Blocks split the current one at
  the caret and go in between, in one undo step; one paragraph goes in at the caret with its marks;
  plain text stays text. Pasted HTML is untrusted: scripts, styles, media and unknown elements are
  dropped or unwrapped to their text, and links pass the document's link rules. Where the host gives
  only plain text, that is what is read.
- In a Markdown document the menus, the format bar and the property editors offer only what Markdown
  can hold.
- Source mode shows the whole document as one Markdown text. A JSON document offers it only when
  Markdown can hold everything in it, else the toggle is disabled and says why. Leaving parses the text
  against the document it started from and sends only the changed blocks as one human batch, so other
  blocks and their ids stay; warnings show first, and a change by someone else meanwhile refuses the
  batch and keeps the text.
- IME composition never splits an undo step, and the person's typing always wins over an incoming
  update to the same block.
- Media blocks edit their caption, `alt` and size in place; tables add and remove rows and columns; code
  picks its language.

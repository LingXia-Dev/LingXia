import type { ComponentDoc } from '@liulidesign/core/internal/doc';

export const doc: ComponentDoc = {
  name: 'JinshuEditor',
  category: 'documents',
  status: 'preview',
  since: '2026-11-24',
  summary: 'A Jinshu document, edited block by block, with history, saving, and a review of what an agent changed or asks',
  description:
    'JinshuEditor edits a Jinshu document where it is read: each block keeps the reader\'s geometry, so moving between reading and ' +
    'editing does not jump. Text blocks edit in place; tables, code, folds, callouts and media have their own editors, and `/` inserts ' +
    'any block. On a live `channel` the source keeps the history, the save state and the review: an agent\'s change is marked at each ' +
    'block it touched, with a bar to view or undo that batch; its suggestions and save requests wait in one queue; and an agent at ' +
    'work shows a mark the person can stop.',
  keywords: ['editor', 'document editor', 'rich text', 'block editor', 'notion', 'wysiwyg', 'review', 'suggestions', 'track changes', 'jinshu'],
  propsType: 'JinshuEditorProps',
  recipe: { map: 'jinshuEditorRecipe', parts: 'jinshuEditorParts' },
  anatomy: [
    { part: 'root', required: true, description: 'The editor: its header, then the document.' },
    { part: 'header', required: true, description: 'Stays in view while scrolling: the toolbar, then whatever needs the person (activity, the change bar, the review queue).' },
    { part: 'toolbar', required: false, description: 'The document\'s toolbar: insert, the app\'s `toolbarActions` and find; on a live `channel` also undo, redo, history and the save state. Absent when read-only.' },
    { part: 'saved', required: false, description: 'The quiet save state at the toolbar\'s end, from `saveStatus`; Save or Retry takes its place while there is something to save.' },
    { part: 'findBar', required: false, description: 'Find and replace across the document, with the match count.' },
    { part: 'history', required: false, description: 'The edit history: who changed what, in the reader\'s language, with undo and redo of each step.' },
    { part: 'activity', required: false, description: 'An agent reading or writing here, named, with Stop; its next edit is refused once stopped.' },
    { part: 'agentBar', required: false, description: 'The change bar: who changed how many blocks while the person was here or away, with View (steps through the changes) and Undo for that batch only.' },
    { part: 'queue', required: false, description: 'One item at a time of what waits for the person: a conflict, an agent\'s save request, its suggestions in document order.' },
    { part: 'canvas', required: true, description: 'The document, at the reader\'s measure.' },
    { part: 'title', required: false, description: 'The document\'s title; pressing it edits it.' },
    { part: 'block', required: true, description: 'One block, with a start-edge mark when an agent changed it.' },
    { part: 'gutter', required: false, description: 'On a desktop, beside the hovered block: insert below, and the block menu (move, duplicate, turn into, properties, ask the agent, delete).' },
    { part: 'textEditor', required: false, description: 'The field a text block becomes while edited, in the block\'s own type.' },
    { part: 'objectBlock', required: false, description: 'A table, media, divider or `x-` block. On a phone a tap selects it, outlined in accent.' },
    { part: 'selectionBar', required: false, description: 'Under a selected block on a phone: move, replace, properties, preview, delete, more, and deselect.' },
    { part: 'suggestions', required: false, description: 'An agent\'s suggestions for a block, each with its note, accept and reject.' },
    { part: 'slashMenu', required: false, description: 'The `/` menu of block types, filtered as the person types.' },
    { part: 'formatBar', required: false, description: 'Bold, italic, code, highlight, link and math for the text being edited; above the keyboard on a phone.' },
    { part: 'trailing', required: false, description: 'The empty line after the last block; pressing it starts a new paragraph.' },
  ],
  accessibility: {
    pattern: null,
    name: null,
    description: [],
    states: [],
    keyboard: [
      { keys: 'Enter in a text block', effect: 'ends it and starts a new paragraph below' },
      { keys: 'Arrow up and down in a text block', effect: 'move to the text block above or below; in the `/` menu, move through its choices' },
      { keys: 'Escape', effect: 'leaves the field, keeping the edit; in the `/` menu, closes it' },
      { keys: 'Enter, arrows in a table cell', effect: 'Enter moves down a row and adds one at the end; up and down move between rows' },
    ],
    contrast: [
      { what: 'document text', pair: ['text', 'canvas'], minimum: '4.5:1' },
      { what: 'change bar text', pair: ['text', 'accent-soft/40'], over: 'canvas', minimum: '4.5:1' },
      { what: 'activity text', pair: ['text', 'info-soft/40'], over: 'canvas', minimum: '4.5:1' },
      { what: 'change mark', pair: ['accent', 'canvas'], minimum: '3:1' },
      { what: 'suggestion note', pair: ['text', 'accent-soft/25'], over: 'canvas', minimum: '4.5:1' },
      { what: 'save state', pair: ['text-muted', 'canvas'], minimum: '4.5:1' },
      { what: 'find match', pair: ['text', 'warning-soft/50'], over: 'canvas', minimum: '4.5:1' },
    ],
    notes: [
      'The toolbar and a phone\'s selection bar are toolbars, each named; every icon button is named by its action.',
      'The activity mark, the change bar and the review queue are status regions, so a screen reader hears them when they appear.',
      'An agent\'s change is never shown by the start-edge mark alone: the change bar says who changed how many blocks.',
      'Each block\'s edit button is named by the block, and each history step by what it did, in the reader\'s language.',
    ],
  },
  guidance: {
    do: [
      'Edit through a live `channel`: the source keeps history, saving, the review and the leave guard\'s state in one place.',
      'Let agents work through the tools the app registers with `createJinshuTools`; the editor shows their changes and requests, and has no AI entry of its own.',
      'Set `onAskAgent` to offer "Ask the agent" in each block\'s menu, and open the app\'s own conversation with those blocks.',
      'Give the source `media.pick` to offer images, video, audio and files; without it, the editor offers none.',
    ],
    dont: [
      'Apply an agent\'s edit in the page; it goes through the source so the change bar, undo and history see it.',
      'Turn off the leave guard on a page that holds unsaved work.',
    ],
  },
  examples: [
    { name: 'Draft', title: 'A draft the page holds' },
    { name: 'AskAgent', title: 'Handing blocks to the app\'s agent' },
    { name: 'ReadOnly', title: 'Read-only' },
  ],
  gaps: [
    {
      missing: 'Bold, italic and links on a selection of words in a text block.',
      needs: 'reading and setting the selection of a text field, and buttons that leave its caret alone',
      today: 'A text block edits as shorthand (`**bold**`, `==mark==`, `[text](#id)`); the format bar wraps the last word.',
    },
  ],
  limits: [
    'History, saving, the change bar, the review queue and the activity mark need a live `channel`; a page-held `doc` has none of them.',
    '`openAt` brings the block to the nearest edge of the scroll, not to its top.',
    'A table\'s header row scrolls with the page; it does not stay pinned.',
  ],
  related: ['JinshuView', 'JinshuDiff', 'JinshuOutline'],
  gallery: {
    specimen: 'Specimen',
    tagline: 'A document, edited',
    order: 2,
    page: {
      lead: 'Use JinshuEditor to let people, and the app\'s agent, edit a structured document in place.',
      stage: 'bare',
      hero: 'Draft',
      variants: [{ name: 'AskAgent', title: 'Handing blocks to the app\'s agent' }, { name: 'ReadOnly', title: 'Read-only' }],
    },
  },
};

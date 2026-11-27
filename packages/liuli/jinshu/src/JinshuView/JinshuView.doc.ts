import type { ComponentDoc } from '@liulidesign/core/internal/doc';

export const doc: ComponentDoc = {
  name: 'JinshuView',
  category: 'documents',
  status: 'preview',
  since: '2026-11-24',
  summary: 'A Jinshu document, read: text, media, tables and the app\'s own blocks, with resume and reading observations',
  description:
    'JinshuView draws a Jinshu document for reading: headings, paragraphs, lists, callouts, folds, code, formulas, tables, images, ' +
    'video, audio, files and the app\'s own `x-` blocks. It reads a document the page holds (`doc`), or follows a live one that Logic ' +
    'serves (`channel` and `docKey`), so an edit made elsewhere, by a person or an agent, appears as it lands. It can offer to continue ' +
    'where the reader stopped, open at one block, hold media to a policy, and report what was read and played through `onObserve`.',
  keywords: ['document', 'reader', 'rich text', 'article', 'lesson', 'course', 'blocks', 'markdown', 'viewer', 'jinshu'],
  propsType: 'JinshuViewProps',
  recipe: { map: 'jinshuViewRecipe', parts: 'jinshuViewParts' },
  anatomy: [
    { part: 'root', required: true, description: 'The column of blocks, a 65-character reading measure unless `isFullWidth`.' },
    { part: 'resume', required: false, description: 'With `resume`: the bar above the document naming where the reader stopped, with Continue and Start over.' },
    { part: 'docTitle', required: false, description: 'The document\'s title, a level-one heading; hidden by `hasTitle={false}`.' },
    { part: 'heading', required: false, description: 'A heading block, levels one to four.' },
    { part: 'paragraph', required: false, description: 'Text in runs: bold, italic, code, highlights, links, inline math and the app\'s atoms.' },
    { part: 'quote', required: false, description: 'A quotation, set in from a rule at its start edge.' },
    { part: 'callout', required: false, description: 'A tinted note in one of four tones, with an optional title in the tone.' },
    { part: 'list', required: false, description: 'A bulleted, numbered or task list; a task\'s box is checkable when the app handles it.' },
    { part: 'code', required: false, description: 'Code under a header of file and language, with line numbers, highlighted lines, and Copy when `onCopyCode` is set; long lines scroll sideways.' },
    { part: 'math', required: false, description: 'A display formula, named by its LaTeX for assistive tech.' },
    { part: 'table', required: false, description: 'A framed grid; columns are at least 9em, and a table wider than its frame scrolls sideways and fades at the edge it runs under.' },
    { part: 'divider', required: false, description: 'A hairline between sections.' },
    { part: 'image', required: false, description: 'A picture with its caption; pressing it opens a lightbox. A blocked or failed source says so, with Retry.' },
    { part: 'video', required: false, description: 'A player with Liuli\'s video controls, chapters and caption, held to `mediaPolicy`.' },
    { part: 'audio', required: false, description: 'An audio card: play, rate, time and a scrubber, then chapters and a folded transcript.' },
    { part: 'file', required: false, description: 'A file card: type badge, name, pages, size, and Download.' },
    { part: 'columns', required: false, description: 'Blocks side by side; stacked on a phone or at a large text size.' },
    { part: 'extension', required: false, description: 'An `x-` block the app gave no renderer for: its fallback text in a dashed box.' },
  ],
  accessibility: {
    pattern: null,
    name: null,
    description: [],
    states: [],
    keyboard: [],
    contrast: [
      { what: 'body text', pair: ['text', 'canvas'], minimum: '4.5:1' },
      { what: 'quote and caption', pair: ['text-muted', 'canvas'], minimum: '4.5:1' },
      { what: 'link', pair: ['accent', 'canvas'], minimum: '4.5:1' },
      { what: 'text in a tip', pair: ['text', 'success-soft'], minimum: '4.5:1' },
      { what: 'tip title', pair: ['success', 'success-soft'], minimum: '4.5:1' },
      { what: 'warning title', pair: ['warning', 'warning-soft'], minimum: '4.5:1' },
      { what: 'danger title', pair: ['danger', 'danger-soft'], minimum: '4.5:1' },
      { what: 'inline code', pair: ['text', 'text/8'], over: 'canvas', minimum: '4.5:1' },
      { what: 'code line numbers', pair: ['text-muted', 'surface-sunken'], minimum: '3:1' },
      { what: 'table header', pair: ['text', 'surface-sunken'], minimum: '4.5:1' },
      { what: 'resume bar', pair: ['text', 'accent-soft/50'], over: 'canvas', minimum: '4.5:1' },
    ],
    notes: [
      'The title and each heading are headings at their level, so a screen reader can move between sections.',
      'Lists are lists of items; a task\'s box is a checkable item named by its text.',
      'Images are named by `alt`, then the caption; a formula is named by its LaTeX; media controls are named by the block\'s title or caption.',
      'Selection is budgeted per page: past the budget, further text renders unselectable rather than slowing the page.',
    ],
  },
  guidance: {
    do: [
      'Hold the document in Logic and pass `channel` and `docKey` when anything else can change it; pass `doc` for a fixed one.',
      'Pass `resume` from the progress you stored, so a returning reader is offered their place.',
      'Use `mediaPolicy` to keep a required video from being skipped; say so near it in the page if it matters.',
      'Keep what `onObserve` reports and decide completion in Logic; the reader reports facts, not grades.',
      'Pass `nav.onVisibleChange` from `useJinshuNav()` to the `<scroll>` holding the reader, with the same `nav` on the reader, for dwell, position and pausing media that scrolls away.',
    ],
    dont: [
      'Rebuild the document in the page to change one block; send ops through the source.',
      'Put interactive app content in a block as text; register it as an `x-` block with `customBlocks`.',
    ],
  },
  examples: [
    { name: 'Lesson', title: 'A lesson' },
    { name: 'Resume', title: 'Continue where the reader stopped' },
    { name: 'Course', title: 'Video and audio held to a policy' },
    { name: 'Attachments', title: 'Files, inline or attached' },
    { name: 'Unavailable', title: 'No document' },
    { name: 'Loading', title: 'Loading' },
  ],
  gaps: [
    {
      missing: 'A table\'s header row staying in view while the page scrolls down.',
      needs: 'a sticky box that binds to the nearest scroll on its own axis',
      today: 'The header row scrolls with the table; it stays aligned with its columns as the table scrolls sideways.',
    },
  ],
  limits: [
    'The reader shows no change bar or review queue; an agent\'s changes appear in place, briefly tinted with `hasUpdateHighlights`. Changes made while no editor was open wait for the next `JinshuEditor`.',
    '`openAt` brings the block to the nearest edge of the scroll, not to its top.',
    'Dwell is measured per top-level block from scrolling alone: time with the page hidden or the reader away shows as one long `idleMs` stretch, and time since the last scroll is not reported when the page closes.',
    'Every block is mounted; the page\'s scroll belongs to the app, so the reader does not mount blocks by viewport.',
    'It has no AI entry of its own: an agent reaches documents only through the tools the app registers with `createJinshuTools`.',
  ],
  related: ['JinshuEditor', 'JinshuOutline', 'JinshuDiff', 'VideoControls', 'Lightbox'],
  gallery: {
    specimen: 'Specimen',
    tagline: 'A document, read',
    order: 1,
    page: {
      lead: 'Use JinshuView to show a structured document for reading: a lesson, an article, a handbook.',
      stage: 'bare',
      hero: 'Lesson',
      variants: [
        { name: 'Course', title: 'Video and audio held to a policy' },
        { name: 'Attachments', title: 'Files, inline or attached' },
      ],
      states: [
        { name: 'Resume', title: 'Continue where the reader stopped' },
        { name: 'Loading', title: 'Loading' },
        { name: 'Unavailable', title: 'Unavailable' },
      ],
      statesLayout: 'stack',
    },
  },
};

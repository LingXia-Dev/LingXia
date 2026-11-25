import type { ComponentDoc } from '../doc';

export const doc: ComponentDoc = {
  name: 'Markdown',
  category: 'content',
  status: 'preview',
  since: '2026-11-15',
  summary: 'Markdown text drawn as Liuli type: a message, a note, a help page',
  description:
    'Markdown shows text someone wrote in Markdown: a chat reply, a note, release notes, help. It draws headings, paragraphs, bold, italic, struck and code runs, links, ' +
    'bulleted, numbered and task lists, quotes, code blocks, rules and tables. It only shows text; it does not edit it. Links do nothing on their own: the app opens an address in `onLinkClick`.',
  keywords: ['markdown', 'rich text', 'prose', 'formatted text', 'viewer', 'message', 'article', 'help text'],
  propsType: 'MarkdownProps',
  recipe: { map: 'markdownRecipe', parts: 'markdownParts' },
  anatomy: [
    { part: 'root', required: true, description: 'The blocks in a column; the text can be selected unless `isSelectable` is off.' },
    { part: 'heading', required: false, description: 'A heading, read as one; `#` is at `topHeadingLevel` and each further `#` one level down.' },
    { part: 'paragraph', required: false, description: 'A paragraph of runs. A line end in the source is a space; two spaces before it break the line.' },
    { part: 'run', required: false, description: 'Bold, italic, struck or code text inside a paragraph.' },
    { part: 'link', required: false, description: 'A link, pressable when `onLinkClick` is set.' },
    { part: 'list', required: false, description: 'A bulleted or numbered list; lists nest.' },
    { part: 'marker', required: false, description: 'An item\'s bullet or number, hidden from assistive tech, which counts the items itself.' },
    { part: 'task', required: false, description: 'A task item\'s mark, empty or ticked. It shows the state; it cannot be pressed.' },
    { part: 'quote', required: false, description: 'A quotation, set off by a rule at its start edge.' },
    { part: 'code', required: false, description: 'A code block in the fixed-width face on a tint, coloured like CodeBlock by the fence\'s language; long lines scroll sideways instead of wrapping.' },
    { part: 'rule', required: false, description: 'A hairline between sections.' },
  ],
  accessibility: {
    pattern: null,
    name: null,
    description: [],
    states: [],
    keyboard: [{ keys: 'Tab', effect: 'Moves through the links, when `onLinkClick` is set' }],
    target: {},
    contrast: [
      { what: 'text', pair: ['text', 'surface'], minimum: '4.5:1' },
      { what: 'markers, struck text and small headings', pair: ['text-muted', 'surface'], minimum: '4.5:1' },
      { what: 'links', pair: ['accent', 'surface'], minimum: '4.5:1' },
      { what: 'inline code on its tint', pair: ['text', 'text/8'], over: 'surface', minimum: '4.5:1' },
      { what: 'code block on its tint', pair: ['text', 'text/6'], over: 'surface', minimum: '4.5:1' },
      { what: 'ticked task mark', pair: ['on-accent-solid', 'accent-solid'], minimum: '3:1' },
      { what: 'empty task mark', pair: ['line-strong', 'surface'], minimum: '3:1' },
    ],
    notes: [
      'Headings are read as headings at their level, lists as lists whose items report their position, links as links named by their text.',
      'A task\'s mark is read as `taskLabels.done` or `taskLabels.open` before its text; without `taskLabels` the mark is not read, so pass them wherever tasks can appear.',
      'A table is a Table: each cell is read with its column\'s heading.',
      'Links are underlined as well as coloured, so they do not depend on colour.',
      'Without `onLinkClick` a link is plain text and not a control: nothing looks pressable that is not.',
    ],
  },
  guidance: {
    do: [
      'Set `topHeadingLevel` to one below the heading the text sits under.',
      'Open links in `onLinkClick`, after checking the address is one the app means to open.',
      'Use `sm` inside a card or a comment thread.',
    ],
    dont: [
      'Build a page\'s own layout from Markdown; use the components.',
      'Pass text from strangers and open whatever it links to.',
      'Use it for one styled line; a `<text>` with a `<span>` does that.',
    ],
  },
  examples: [
    { name: 'Note', title: 'A note with tasks and a link' },
    { name: 'Reference', title: 'Code, a table and a quote' },
    { name: 'Comment', title: 'Small' },
  ],
  gaps: [
    { missing: 'Images.', needs: 'nothing from the View', today: 'An image is drawn as its alternative text.' },
    { missing: 'Editing: bold and lists while typing.', needs: 'an editor that holds styled text', today: 'Viewing only; TextArea edits the plain source.' },
    { missing: 'Raw HTML, footnotes and definition lists.', needs: 'nothing from the View', today: 'Shown as the text they were written as.' },
    { missing: 'A paragraph of more than 256 styled runs.', needs: 'a higher run limit on `<text>`', today: 'Not handled; split the paragraph.' },
  ],
  playground: { props: { source: '**Bold**, _italic_ and `code`, in a short note.' } },
  related: ['Table', 'Item', 'TextArea'],
  gallery: {
    specimen: 'Specimen',
    tagline: 'Written text, shown',
    order: 8,
    page: {
      stage: 'bare',
      lead: 'Use Markdown to show text someone wrote: a message, a note, a help page.',
      hero: 'Note',
      variants: [
        { name: 'Reference', title: 'Inline code, a code block, a table and a quote' },
        { name: 'Comment', title: 'Small, for a comment or a card' },
      ],
    },
  },
};

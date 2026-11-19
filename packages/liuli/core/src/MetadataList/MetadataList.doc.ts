import type { ComponentDoc } from '../doc';

export const doc: ComponentDoc = {
  name: 'MetadataList',
  category: 'content',
  status: 'preview',
  since: '2026-11-15',
  summary: 'What is known about one thing, as labels and their values',
  description:
    'MetadataList shows the facts about a single record: who owns it, when it was made, what state it is in. In rows the labels share a column and the values line up beside them; ' +
    'stacked, each label sits over its value, a few to a line. A value can be drawn as a Badge or an Avatar and is still read as its text. For many records side by side, use a Table.',
  keywords: ['metadata', 'key value', 'details', 'description list', 'properties', 'facts', 'definition list'],
  propsType: 'MetadataListProps',
  otherPropsTypes: ['MetadataItem'],
  recipe: { map: 'metadataListRecipe', parts: 'metadataListParts' },
  anatomy: [
    { part: 'root', required: true, description: 'The pairs, a list named by `label`.' },
    { part: 'item', required: true, description: 'One pair, read as "Label: value".' },
    { part: 'label', required: true, description: 'What the value is, in muted footnote type.' },
    { part: 'value', required: true, description: 'The value; it wraps rather than truncating.' },
    { part: 'content', required: false, description: 'Drawn in place of the value\'s text.' },
  ],
  accessibility: {
    pattern: null,
    name: 'label',
    description: [],
    states: [],
    keyboard: [],
    target: {},
    contrast: [
      { what: 'value', pair: ['text', 'surface'], minimum: '4.5:1' },
      { what: 'label', pair: ['text-muted', 'surface'], minimum: '4.5:1' },
    ],
    notes: [
      'The pairs are a list, so a screen reader says how many there are; each is read as one phrase, "Status: Overdue".',
      'A value drawn by `content` is read as its `value` text, so a badge\'s meaning must be in that text.',
    ],
  },
  guidance: {
    do: [
      'Order the pairs by what people look for first, not by the record\'s fields.',
      'Use `rows` for a list that may have long values; `stacked` for a handful of short ones.',
      'Format dates and amounts as the rest of the app does.',
      'At large text, a label word wider than its third widens the label column; keep labels to short words.',
    ],
    dont: [
      'Use it for fields people edit; that is a form.',
      'Leave a pair with an empty value; leave the pair out, or say "None".',
    ],
  },
  examples: [
    { name: 'Invoice', title: 'Labels beside their values' },
    { name: 'Stacked', title: 'Labels over their values' },
  ],
  gaps: [
    { missing: 'A label column as wide as its longest label.', needs: 'text measurement before layout', today: 'A third of the row.' },
    { missing: 'A value that can be copied with a press.', needs: 'nothing from the View', today: 'The app puts an IconButton in `content`.' },
  ],
  related: ['Table', 'Item', 'Badge'],
  gallery: {
    specimen: 'Specimen',
    tagline: 'Labels and values',
    order: 9,
    page: {
      lead: 'Use a metadata list for the facts about one record.',
      hero: 'Invoice',
      variants: [{ name: 'Stacked', title: 'Each label over its value, two to a line' }],
    },
  },
};

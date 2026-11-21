import type { ComponentDoc } from '../doc';

export const doc: ComponentDoc = {
  name: 'Divider',
  category: 'content',
  status: 'preview',
  since: '2026-11-15',
  summary: 'A hairline between two things that belong apart',
  description:
    'Divider draws the system\'s subtle hairline between blocks, across a card or upright between items in a row. It can carry a word in its middle, "or". ' +
    'Space parts most things better than a line does; use a divider where two blocks would otherwise read as one.',
  keywords: ['divider', 'separator', 'rule', 'line', 'hairline', 'hr'],
  propsType: 'DividerProps',
  recipe: { map: 'dividerRecipe', parts: 'dividerParts' },
  anatomy: [
    { part: 'line', required: true, description: 'The hairline, across or upright.' },
    { part: 'label', required: false, description: 'A word in the middle of a horizontal line.' },
  ],
  accessibility: {
    pattern: null,
    name: null,
    description: [],
    states: [],
    keyboard: [],
    target: {},
    contrast: [{ what: 'label', pair: ['text-muted', 'surface'], minimum: '4.5:1' }],
    notes: [
      'The line is decoration and is not read: headings and groups carry the structure. Its contrast is below 3:1 on purpose.',
      'A label on the line is read as text where it stands.',
    ],
  },
  guidance: {
    do: [
      'Try more space first; add the line where the blocks still read as one.',
      'Use `inset="start"` in a list whose rows begin with an icon, so the line starts where the text does.',
    ],
    dont: [
      'Rule every row of a list by hand; ItemList and Table divide their own rows.',
      'Use it as a border around something; that is a Card.',
    ],
  },
  examples: [
    { name: 'Between', title: 'Between two blocks' },
    { name: 'Labelled', title: 'With a word on the line' },
    { name: 'Vertical', title: 'Upright, in a row' },
  ],
  gaps: [],
  playground: { props: { orientation: 'horizontal' } },
  related: ['Section', 'Card', 'Item'],
  gallery: {
    specimen: 'Specimen',
    tagline: 'A line between',
    order: 11,
    page: {
      lead: 'Use a divider where two blocks would otherwise read as one.',
      hero: 'Between',
      variants: [
        { name: 'Labelled', title: 'A word on the line' },
        { name: 'Vertical', title: 'Upright, between items in a row' },
      ],
    },
  },
};

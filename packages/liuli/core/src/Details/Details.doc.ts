import type { ComponentDoc } from '../doc';

export const doc: ComponentDoc = {
  name: 'Details',
  category: 'content',
  status: 'preview',
  since: '2026-11-12',
  summary: 'A summary row that shows and hides the content under it',
  description:
    'Details hides secondary content behind a row the person opens when they need it: an answer, advanced settings, the fine print. ' +
    'DetailsGroup stacks rows in one bordered list, and can keep a single row open as an accordion.',
  keywords: ['details', 'disclosure', 'accordion', 'expand', 'collapse', 'faq', 'show more', 'summary'],
  propsType: 'DetailsProps',
  recipe: { map: 'detailsRecipe', parts: 'detailsParts' },
  anatomy: [
    { part: 'group', required: false, description: 'DetailsGroup: rows in one bordered list, divided by hairlines.' },
    { part: 'summary', required: true, description: 'The toggle: a button named by `label` that announces whether it is expanded.' },
    { part: 'chevron', required: true, description: 'Turns over as the row opens.' },
    { part: 'body', required: false, description: 'The content, which exists only while open.' },
  ],
  accessibility: {
    pattern: 'button',
    name: 'label',
    description: [],
    states: [{ state: 'expanded', from: 'isOpen, isDefaultOpen, or the group\'s open row' }],
    keyboard: [{ keys: 'Enter, Space', effect: 'Opens or closes the row.' }],
    target: { lg: 48 },
    contrast: [{ what: 'summary text', pair: ['text', 'surface'], minimum: '4.5:1' }],
    notes: [
      'Closed content does not exist, so a search or a screen reader reaches it only once it is open.',
      'In an exclusive group, closing the open row leaves none open.',
    ],
  },
  guidance: {
    do: [
      'Write the summary as the question or the name of what is inside.',
      'Use an exclusive group when the rows are alternatives, such as answers in an FAQ.',
    ],
    dont: [
      'Hide what most people need; show it.',
      'Put a form someone must fill in behind a closed row.',
    ],
  },
  examples: [
    { name: 'Single', title: 'One row' },
    { name: 'Accordion', title: 'An accordion' },
    { name: 'Independent', title: 'Rows that open on their own' },
  ],
  gaps: [
    {
      missing: 'Liuli on the Details primitive.',
      needs: 'a `<Summary>` that can drop or move its start marker, and whose inner row stretches to the button\'s width',
      today: 'Liuli builds the same button with aria-expanded, so the chevron can sit at the end.',
    },
    {
      missing: 'A body that grows open instead of appearing.',
      needs: 'height transitions, or a layout motion that animates size',
      today: 'The body fades and drops in; the rows below jump.',
    },
  ],
  related: ['Item', 'Card'],
  gallery: {
    specimen: 'Specimen',
    tagline: 'More, behind a row',
    order: 3,
    page: {
      stage: 'bare',
      lead: 'Use details for content some people need and most do not: an answer, advanced settings, the fine print.',
      hero: 'Single',
      variants: [
        { name: 'Accordion', title: 'An accordion: one row open at a time' },
        { name: 'Independent', title: 'A group whose rows open on their own' },
      ],
      states: [
        { name: 'StateClosed', title: 'Closed' },
        { name: 'StateOpen', title: 'Open' },
      ],
    },
  },
};

import type { ComponentDoc } from '../doc';

export const doc: ComponentDoc = {
  name: 'PageControl',
  category: 'navigation',
  status: 'preview',
  since: '2026-11-15',
  summary: 'Dots for which page of a few is in view',
  description:
    'PageControl draws one dot per page and stretches the current one. Pressing a dot goes to its page. ' +
    'Bound to the same cell as a paging scroll, the dots and the pages follow each other; Carousel does that for you.',
  keywords: ['page control', 'dots', 'pager', 'indicator', 'carousel', 'pagination'],
  propsType: 'PageControlProps',
  recipe: { map: 'pageControlRecipe', parts: 'pageControlParts' },
  anatomy: [
    { part: 'root', required: true, description: 'The row of dots, a tab list named by `label`.' },
    { part: 'dot', required: true, description: 'One page: a button taller and wider than its mark.' },
    { part: 'mark', required: true, description: 'The 6 px dot; the current page\'s is a pill in the tone.' },
  ],
  accessibility: {
    pattern: null,
    name: 'label',
    description: [],
    states: [{ state: 'selected', from: 'page, on the dot of the page in view' }],
    keyboard: [
      { keys: 'Tab', effect: 'Moves to a dot' },
      { keys: 'Enter or Space', effect: 'Goes to its page' },
    ],
    target: {},
    contrast: [
      { what: 'current dot', pair: ['accent', 'surface'], minimum: '3:1' },
      { what: 'current dot over media', pair: ['on-media', 'media'], minimum: '3:1' },
    ],
    notes: [
      'Each dot is a tab named by `pageLabel` and reports whether it is the selected one.',
      'The current dot is told apart by its width, not only its colour; the other dots are faint on purpose.',
      'A dot is 14 by 26 px to the finger; a press that misses goes to the nearest dot within reach.',
    ],
  },
  guidance: {
    do: [
      'Keep it to a handful of pages; past ten, say "3 of 24" in words.',
      'Write `pageLabel` so it says where the dot leads: "Photo 2 of 5".',
    ],
    dont: [
      'Use dots to move through pages of results; that is Pagination.',
      'Use it as the only way to page; the content itself swipes.',
    ],
  },
  examples: [
    { name: 'Welcome', title: 'Under what it pages' },
    { name: 'OnMedia', title: 'Over a picture' },
  ],
  gaps: [],
  limits: [
    'Many pages are not folded into a window of dots; the row grows with `count`.',
  ],
  related: ['Carousel', 'Pagination', 'Tabs'],
  gallery: {
    specimen: 'Specimen',
    tagline: 'Which page of a few',
    order: 7,
    page: {
      lead: 'Use a page control to show which of a few pages is in view.',
      hero: 'Welcome',
      variants: [{ name: 'OnMedia', title: 'Over a picture, in the on-media tone' }],
    },
  },
};

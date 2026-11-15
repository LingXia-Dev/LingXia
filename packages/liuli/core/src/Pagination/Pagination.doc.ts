import type { ComponentDoc } from '../doc';

export const doc: ComponentDoc = {
  name: 'Pagination',
  category: 'navigation',
  status: 'preview',
  since: '2026-11-15',
  summary: 'Moves through a long set a page at a time',
  description:
    'Pagination sits under a table or a list that is shown a page at a time. With a mouse it is a row of page numbers between two arrows: the first, the last, ' +
    'the current page and its neighbours. With a finger it is the two arrows around the position, "3 / 12", since a row of small numbers is hard to press. ' +
    'The app holds the page and loads its rows.',
  keywords: ['pagination', 'pager', 'pages', 'page numbers', 'next page', 'previous page', 'paging'],
  propsType: 'PaginationProps',
  recipe: { map: 'paginationRecipe', parts: 'paginationParts' },
  anatomy: [
    { part: 'root', required: true, description: 'The control, a group named by `label`, between its two arrows.' },
    { part: 'pages', required: false, description: 'The page numbers, with a mouse or `variant="pages"`.' },
    { part: 'page', required: false, description: 'One page: a 40 px button, pressed when it is the current page.' },
    { part: 'gap', required: false, description: 'An ellipsis where pages are left out; not read.' },
    { part: 'position', required: false, description: 'The position between the arrows, with a finger or `variant="compact"`; announced as it changes.' },
  ],
  accessibility: {
    pattern: null,
    name: 'label',
    description: [],
    states: [{ state: 'pressed', from: 'page, on the current page\'s button' }, { state: 'disabled', from: 'the first or last page, on its arrow' }],
    keyboard: [
      { keys: 'Tab', effect: 'Through the arrows and the page numbers' },
      { keys: 'Enter or Space', effect: 'Goes to the page' },
    ],
    target: { md: 40 },
    contrast: [
      { what: 'page number', pair: ['text', 'surface'], minimum: '4.5:1' },
      { what: 'current page', pair: ['on-accent-soft', 'accent-soft'], minimum: '4.5:1' },
      { what: 'ellipsis', pair: ['text-muted', 'surface'], minimum: '4.5:1' },
    ],
    notes: [
      'Each page button is named by `pageLabel`, "Page 6", and the current one reports pressed.',
      'The arrows are named by `previousLabel` and `nextLabel` and are disabled at the ends.',
      'The compact position is a status: a screen reader hears it when the page changes. Give `positionLabel` words, "Page 3 of 12", where "3 / 12" would be read badly.',
      'The current page is marked by its fill and a heavier weight as well as by its state.',
    ],
  },
  guidance: {
    do: [
      'Say beside it which rows are shown: "Showing 41–60 of 240".',
      'Leave `variant` on `auto`. With `pages`, numbers that do not fit their row drop their neighbours, then give way to the position.',
      'Keep the page in the app\'s state, so coming back to the list keeps the place.',
    ],
    dont: [
      'Page a set that fits on one screen.',
      'Use it for steps in a task; that is a stepper.',
      'Use it in a feed people scroll; load more as the end comes into view.',
    ],
  },
  examples: [
    { name: 'Results', title: 'Under a list of results' },
    { name: 'Pages', title: 'Page numbers' },
    { name: 'Compact', title: 'Arrows and the position' },
    { name: 'Narrow', title: 'Page numbers in a narrow row' },
  ],
  gaps: [
    { missing: 'Choosing how many rows a page holds, and jumping to a page by typing it.', needs: 'nothing from the View', today: 'An app puts a Select or a NumberInput beside it.' },
  ],
  related: ['Table', 'Tabs', 'IconButton'],
  gallery: {
    specimen: 'Specimen',
    tagline: 'A page at a time',
    order: 4,
    page: {
      lead: 'Use pagination under a table or list that is shown a page at a time.',
      hero: 'Results',
      variants: [
        { name: 'Pages', title: 'Page numbers: first, last, the current page and its neighbours' },
        { name: 'Compact', title: 'The arrows and the position' },
        { name: 'Narrow', title: 'Page numbers at 280 and 232 points: fewer neighbours, then the position' },
      ],
    },
  },
};

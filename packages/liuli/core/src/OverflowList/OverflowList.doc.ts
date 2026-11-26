import type { ComponentDoc } from '../doc';

export const doc: ComponentDoc = {
  name: 'OverflowList',
  category: 'content',
  status: 'preview',
  since: '2026-11-26',
  summary: 'A line of items that keeps to one row, folding what does not fit into one element',
  description:
    'OverflowList lays out items on one line and measures them: when they do not all fit, the ones that do stay and the rest are handed to `renderOverflow`, which draws one element for them, a "+3" token or a More menu. ' +
    'It folds from the end, or from the start to keep the end of a path. Widths are remembered by key, so a line that widens brings items back without a second look.',
  keywords: ['overflow list', 'overflow', 'more menu', 'collapse', 'plus n', 'truncate list', 'priority plus', 'toolbar overflow', 'tags'],
  propsType: 'OverflowListProps',
  recipe: { map: 'overflowListRecipe', parts: 'overflowListParts' },
  anatomy: [
    { part: 'root', required: true, description: 'The line: one row that never wraps. With `label` it is a group of that name.' },
    { part: 'item', required: true, description: 'Each item as `renderItem` draws it, at its own width.' },
    { part: 'overflow', required: false, description: 'What `renderOverflow` draws for the items that do not fit; left out when all fit.' },
  ],
  accessibility: {
    pattern: null,
    name: 'label',
    description: [],
    states: [],
    keyboard: [{ keys: 'Tab', effect: 'Reaches the items on the line, then the overflow element' }],
    contrast: [],
    notes: [
      'Items that do not fit are not on the line at all, so a screen reader reaches them only through the overflow element: make it a menu or say how many it holds.',
      'The first frame lays out every item, clipped, to measure them; the overflow element stands in unseen and unread until the fit is known.',
    ],
  },
  guidance: {
    do: [
      'Give `keyOf` a stable key, so measured widths survive a change of the list.',
      'Make the overflow a menu of the hidden actions, in the same order, or a count of the hidden tags.',
      'Fold from the start for a path, so where you are stays on the line.',
    ],
    dont: [
      'Hide the only way to an action in a count; a "+3" token is for things to read, a menu for things to do.',
      'Put items that change width on their own (a live counter) on the line; their width is measured once.',
    ],
  },
  examples: [
    { name: 'Tags', title: 'Tags, then a count' },
    { name: 'Actions', title: 'Actions, then a More menu' },
    { name: 'FromStart', title: 'Folding from the start of a path' },
  ],
  gaps: [
    {
      missing: 'Re-measuring an item whose width changes while it is folded away.',
      needs: 'nothing from the View',
      today: 'Its width is the one last measured; a new key measures it again.',
    },
  ],
  playground: { props: {}, fixture: true },
  related: ['ButtonGroup', 'Token', 'DropdownMenu', 'Breadcrumb'],
  gallery: {
    specimen: 'Specimen',
    tagline: 'One line, the rest',
    order: 21,
    page: {
      lead: 'Use an overflow list when a line of tags or actions must keep to one row at any width.',
      hero: 'Actions',
      variants: [
        { name: 'Tags', title: 'Tags, then a count' },
        { name: 'FromStart', title: 'Folding from the start of a path' },
      ],
      sizes: [],
      states: [],
      inContext: [],
    },
  },
};

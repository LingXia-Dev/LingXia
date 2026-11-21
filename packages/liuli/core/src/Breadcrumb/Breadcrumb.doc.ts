import type { ComponentDoc } from '../doc';

export const doc: ComponentDoc = {
  name: 'Breadcrumb',
  category: 'navigation',
  status: 'preview',
  since: '2026-11-15',
  summary: 'Where a page sits: the levels above it, each a way back up',
  description:
    'Breadcrumb shows the path to the current page in a hierarchy: a folder in a drive, a record in a section. Each level above is a button that goes back to it; ' +
    'the last item is the current page and is not pressable. A deep trail folds its middle levels into a menu, keeping the first level and the last two.',
  keywords: ['breadcrumb', 'breadcrumbs', 'path', 'trail', 'hierarchy', 'location', 'back'],
  propsType: 'BreadcrumbProps',
  otherPropsTypes: ['BreadcrumbItem'],
  recipe: { map: 'breadcrumbRecipe', parts: 'breadcrumbParts' },
  anatomy: [
    { part: 'root', required: true, description: 'The trail, a list named by `label`; it wraps by whole levels.' },
    { part: 'item', required: true, description: 'One level with the separator before it.' },
    { part: 'link', required: false, description: 'A level above the current page: a button in muted text.' },
    { part: 'label', required: true, description: 'A level\'s name, cut short past twelve characters\' width or so; the current page is heavier and in the text colour.' },
    { part: 'separator', required: false, description: 'A chevron between levels; it mirrors for right-to-left text and is not read.' },
  ],
  accessibility: {
    pattern: null,
    name: 'label',
    description: [],
    states: [],
    keyboard: [
      { keys: 'Tab', effect: 'Through the levels above the current page' },
      { keys: 'Enter or Space', effect: 'Opens the level' },
    ],
    target: { sm: 32 },
    contrast: [
      { what: 'levels above', pair: ['text-muted', 'canvas'], minimum: '4.5:1' },
      { what: 'current page', pair: ['text', 'canvas'], minimum: '4.5:1' },
    ],
    notes: [
      'The trail is a list, so a screen reader says how many levels there are and where each stands.',
      'The current page is the last item and plain text: it is told apart by not being a button, by its weight and by its colour.',
      'Folded levels are in a menu named by `moreLabel`; they stay reachable, in order.',
      'A level is 32 px tall, below the touch minimum: on a phone the page\'s own back button is the main way up, and the trail is for seeing where one is.',
    ],
  },
  guidance: {
    do: [
      'Start at the top of the hierarchy and end on the current page.',
      'Pass `moreLabel` wherever a trail can be deep, so it folds instead of wrapping to three lines.',
      'Use it on wide screens and for deep hierarchies; a two-level app needs only a back button.',
    ],
    dont: [
      'Use it for the steps of a task, or for history: it shows where a page is, not how the person got there.',
      'Make the current page a link.',
    ],
  },
  examples: [
    { name: 'Path', title: 'A path of three levels' },
    { name: 'Deep', title: 'A deep trail, folded' },
  ],
  gaps: [
    { missing: 'Folding by the room there is, not by a count.', needs: 'nothing from the View', today: 'Folds past `maxItems`; otherwise the trail wraps.' },
    { missing: 'The current page announced as current.', needs: 'a current-page state for assistive tech', today: 'The current page is the last item of the list and not a button.' },
  ],
  playground: { props: { label: 'Breadcrumb' }, fixture: true },
  related: ['Tabs', 'NavList', 'DropdownMenu'],
  gallery: {
    specimen: 'Specimen',
    tagline: 'The way back up',
    order: 3,
    page: {
      lead: 'Use a breadcrumb to show where a page sits in a hierarchy and to go back up it.',
      hero: 'Path',
      variants: [{ name: 'Deep', title: 'Six levels: the middle three fold into a menu' }],
    },
  },
};

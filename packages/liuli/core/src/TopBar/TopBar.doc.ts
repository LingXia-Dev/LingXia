import type { ComponentDoc } from '../doc';

export const doc: ComponentDoc = {
  name: 'TopBar',
  category: 'navigation',
  status: 'preview',
  since: '2026-11-18',
  summary: 'A page\'s own header row: a way back, its title, a control at the middle, its actions at the end',
  description:
    'TopBar is the header row a page draws itself, for a page that shows no host navigation bar (`navigationStyle: "custom"`). ' +
    'It holds a control at the start (a way back, a menu), the page\'s title as a heading, a control at the middle (a SearchField, a segmented control) and the page\'s actions at the end. ' +
    'The start and the end share the room left over, so the middle stays at the bar\'s middle while everything fits; past that the bar wraps. ' +
    'The page already keeps its content clear of system bars and host chrome, so the bar adds only its own padding.',
  keywords: ['top bar', 'header', 'navigation bar', 'toolbar', 'app bar', 'page title', 'actions'],
  propsType: 'TopBarProps',
  recipe: { map: 'topBarRecipe', parts: 'topBarParts' },
  anatomy: [
    { part: 'root', required: true, description: 'The row, at least 44 pt tall, with a hairline under it.' },
    { part: 'start', required: true, description: '`startContent`, then the heading; it shares the room left over with `end`.' },
    { part: 'heading', required: false, description: 'A string `heading`: the page\'s title, a heading, cut short when it does not fit.' },
    { part: 'center', required: false, description: '`centerContent`, at the bar\'s middle while everything fits.' },
    { part: 'end', required: true, description: '`endContent`, held to the end.' },
  ],
  accessibility: {
    pattern: null,
    name: null,
    description: [],
    states: [],
    keyboard: [],
    contrast: [
      { what: 'title', pair: ['text', 'surface'], minimum: '4.5:1' },
    ],
    notes: [
      'A string `heading` is the page\'s heading at level 1, so a screen reader can jump to it.',
      'Start and end follow the page\'s direction: in right-to-left the way back sits on the right.',
      'Each action is its own button, reached in order after the heading.',
    ],
  },
  guidance: {
    do: [
      'Draw either the host\'s navigation bar or a TopBar on a page, never both: set `navigationStyle: "custom"` on a page that draws its own.',
      'Name every icon-only action with its `label`.',
    ],
    dont: [
      'Put app-close or window controls in it; those belong to the host.',
      'Stack a second bar under the host\'s navigation bar for the page\'s actions; give the page a TopBar instead.',
    ],
  },
  examples: [
    { name: 'Standard', title: 'A way back, the title and actions' },
    { name: 'WithSearch', title: 'A search field at the middle' },
  ],
  gaps: [],
  related: ['IconButton', 'SearchField', 'ListDetail'],
  gallery: {
    specimen: 'Specimen',
    tagline: 'A page\'s own header',
    order: 9,
    page: {
      lead: 'Use TopBar on a page that draws its own header: a way back, its title, a search field or a filter at the middle, its actions at the end.',
      stage: 'bare',
      hero: 'Standard',
      variants: [
        { name: 'WithSearch', title: 'A search field at the middle' },
      ],
    },
  },
};

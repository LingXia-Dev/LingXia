import type { ComponentDoc } from '../doc';

export const doc: ComponentDoc = {
  name: 'ListDetail',
  category: 'content',
  status: 'preview',
  since: '2026-11-18',
  summary: 'A list and the detail of the row chosen in it: side by side on a wide screen, one over the other on a phone',
  description:
    'ListDetail lays out a list and the record chosen in it from one tree, driven by one value: the chosen row\'s id. ' +
    'On a wide screen the list sits at the start, `listWidth` wide, and the detail beside it, capped so lines stay readable; at a fold that crosses the page the list takes the start leaf and the detail the other. ' +
    'On a phone the list fills the screen; choosing a row opens its detail over it, under a way back, and Back returns to the list where it was scrolled. ' +
    'Each pane scrolls on its own, and the detail starts at its top for each new choice. A deep link that sets the value opens the record directly.',
  keywords: ['list detail', 'master detail', 'two pane', 'split view', 'sidebar layout', 'workspace', 'adaptive', 'foldable'],
  propsType: 'ListDetailProps',
  otherPropsTypes: ['ListDetailListProps', 'ListDetailDetailProps'],
  recipe: { map: 'listDetailRecipe', parts: 'listDetailParts' },
  anatomy: [
    { part: 'root', required: true, description: 'Fills its page: the two panes side by side, or the one a phone shows.' },
    { part: 'list', required: true, description: 'The list pane, a group named by `listLabel`, with a hairline at its end. On a phone it stays mounted while the detail covers it, so its scroll survives.' },
    { part: 'detail', required: true, description: 'The detail pane, a group named by `title`; on a phone it slides in from the end edge and out again, unless motion is reduced.' },
    { part: 'back', required: false, description: 'Phone: the back button, named by `listLabel`, and the record\'s title as a heading, outside the detail\'s scroll.' },
    { part: 'title', required: false, description: 'Phone: the record\'s title, a heading beside the back button.' },
    { part: 'scroll', required: true, description: 'In each pane, so the two scroll apart.' },
    { part: 'listInner', required: true, description: 'The list\'s padding.' },
    { part: 'detailInner', required: true, description: 'The detail\'s padding and cap, held to the start of its pane.' },
    { part: 'placeholder', required: false, description: 'Wide screen, nothing chosen: what choosing a row will show.' },
    { part: 'actions', required: false, description: 'With `actions`: a row under the detail\'s scroll, behind a hairline, that stays in view.' },
  ],
  accessibility: {
    pattern: null,
    name: 'listLabel',
    description: [],
    states: [],
    keyboard: [],
    contrast: [
      { what: 'record title on a phone', pair: ['text', 'canvas'], minimum: '4.5:1' },
    ],
    notes: [
      'Each pane is a group named by its label, so a screen reader can move between them; the detail\'s name follows the chosen record.',
      'The chosen row says so itself: pass `isSelected` to its Item.',
      'On a phone Back leaves the detail for the list, as the back button does; it is taken only with `bind`, or `value` with `onChange`.',
    ],
  },
  guidance: {
    do: [
      'Hold the choice in Logic with `value` and `onChange` when a deep link can open a record; use `bind` when the page alone knows it.',
      'Give `<ListDetail.Detail>` a `placeholder`, so a wide screen says what choosing a row will show.',
      'Put the actions on the chosen record in `actions`, so a long record never hides them.',
      'Put it straight in a `<page>`: it sizes the panes to the page, and at a fold to its leaves.',
      'Use `contentWidth="prose"` for reading and `wide` for tables or examples.',
    ],
    dont: [
      'Build a phone page and a wide page apart just to switch between one pane and two.',
      'Put it inside a `<scroll>`, or a scroll inside its panes; each pane scrolls itself.',
    ],
  },
  examples: [
    { name: 'Orders', title: 'Orders, with an action on the chosen one' },
    { name: 'Workspace', title: 'A mail workspace held by the caller' },
  ],
  gaps: [
    { missing: 'Dragging the hairline to resize the list', needs: 'Pointer coordinates on an element (`onPointer`)', today: 'A fixed `listWidth`' },
  ],
  limits: [
    'At a fold, a start leaf narrower than 200 pt still gets a 200 pt list, which then reaches past the crease.',
  ],
  related: ['Split', 'Item', 'NavList'],
  gallery: {
    specimen: 'Specimen',
    tagline: 'List and detail',
    order: 17,
    page: {
      lead: 'Use ListDetail when choosing a row in a list opens its record: side by side on a wide or unfolded screen, one over the other on a phone, with Back.',
      stage: 'bare',
      hero: 'Orders',
    },
  },
};

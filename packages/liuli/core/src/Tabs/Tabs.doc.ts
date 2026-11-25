import type { ComponentDoc } from '../doc';

export const doc: ComponentDoc = {
  name: 'Tabs',
  category: 'navigation',
  status: 'preview',
  since: '2026-11-12',
  summary: 'A row of tabs over one panel, as an underline or a segmented control',
  description:
    'Tabs switch one area between a few peers: sections of a page, or modes of one view. ' +
    'The tabs share the row equally, and a single indicator slides to the chosen one. No label is cut: a tab is never ' +
    'narrower than its label, and a row that cannot hold them scrolls.',
  keywords: ['tabs', 'tab bar', 'segmented control', 'segments', 'switcher', 'sections', 'filter', 'range'],
  propsType: 'TabsProps',
  recipe: { map: 'tabsRecipe', parts: 'tabsParts' },
  anatomy: [
    { part: 'frame', required: true, description: 'What the row fits in, around the scroll that holds it; for the segmented control, the sunken track.' },
    { part: 'list', required: true, description: 'The tab list: one Tab stop, named by `label`; it carries the underline rule.' },
    { part: 'track', required: true, description: 'Holds the indicator; mirrors in right-to-left so it slides toward the end.' },
    { part: 'indicator', required: true, description: 'The underline or thumb, one tab wide, sliding to the chosen tab.' },
    { part: 'tab', required: true, description: 'One tab per item; its text is its name.' },
    { part: 'panel', required: false, description: 'The chosen tab\'s content from `children`, named after that tab.' },
  ],
  accessibility: {
    pattern: 'tabs',
    name: 'label',
    description: [],
    states: [
      { state: 'selected', from: 'value or bind' },
      { state: 'disabled', from: 'items[].isDisabled' },
    ],
    keyboard: [
      { keys: 'Tab', effect: 'Into the row, on the chosen tab; again, on to the panel.' },
      { keys: 'Left, Right', effect: 'The previous or next tab, chosen on arrival; wraps at the ends.' },
      { keys: 'Home, End', effect: 'The first or last tab.' },
    ],
    target: { md: 40 },
    contrast: [
      { what: 'chosen tab text', pair: ['text', 'surface'], minimum: '4.5:1' },
      { what: 'other tabs\' text', pair: ['text-muted', 'surface'], minimum: '4.5:1' },
      { what: 'underline', pair: ['accent', 'surface'], minimum: '3:1' },
      { what: 'chosen segment text', pair: ['text', 'surface-raised'], minimum: '4.5:1' },
      { what: 'other segments\' text', pair: ['text-muted', 'surface-sunken'], minimum: '4.5:1' },
    ],
    notes: [
      'The chosen state is in the text weight and colour and in the indicator, never in colour alone.',
      'The segmented variant is drawn from SegmentedControl\'s pieces at its medium size: 40 px tall, level with a medium Button.',
      'The segmented thumb\'s edge is decorative and has no contrast minimum: the chosen segment is carried by its label and the raised thumb.',
    ],
  },
  guidance: {
    do: [
      'Keep to two to five short labels that fit the row.',
      'At large text, expect the row to scroll rather than cut a label; put the tab people need first at the start.',
      'Use the underline for sections of a page and the segmented control for modes of one view.',
    ],
    dont: [
      'Use tabs for steps someone must finish in order.',
      'Put actions in a tab row; a tab only switches what is shown.',
    ],
  },
  examples: [
    { name: 'Sections', title: 'Sections of a page' },
    { name: 'Segmented', title: 'Modes of one view' },
    { name: 'Icons', title: 'Icons, a count, and icon-only tabs' },
    { name: 'Filter', title: 'A segmented filter over a list' },
    { name: 'Fitted', title: 'Tabs as wide as their labels, in a row that scrolls' },
  ],
  gaps: [
    {
      missing: 'An indicator that changes width as it slides between fitted tabs.',
      needs: 'a transition on a box\'s width',
      today: 'It slides to the chosen tab and takes that tab\'s width at once.',
    },
    {
      missing: 'Fitted tabs on a host built before `useBox`.',
      needs: 'a newer host',
      today: 'The tabs are drawn and work; the indicator is not shown, and the chosen tab is told by its darker label.',
    },
  ],
  playground: { props: { label: 'Project', value: 'overview' }, fixture: true },
  related: ['Button', 'DropdownMenu'],
  gallery: {
    specimen: 'Specimen',
    tagline: 'A few peer views',
    order: 1,
    page: {
      lead: 'Use tabs to switch one area between a few peers. Each tab is a place, not an action.',
      hero: 'Sections',
      variants: [
        { name: 'Segmented', title: 'Segmented: modes of one view' },
        { name: 'Icons', title: 'Icons and a count; icon-only segmented tabs' },
        { name: 'Fitted', title: 'Six tabs as wide as their labels; the row scrolls and the chosen tab is brought into view' },
      ],
      states: [
        { name: 'StateUnderline', title: 'Underline' },
        { name: 'StateSegmented', title: 'Segmented' },
        { name: 'StateDisabled', title: 'A disabled tab' },
        { name: 'StateSegmentedDisabled', title: 'A disabled segment' },
      ],
      inContext: [{ name: 'Filter', title: 'Filtering a list' }],
    },
  },
};

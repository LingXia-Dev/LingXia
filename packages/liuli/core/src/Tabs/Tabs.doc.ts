import type { ComponentDoc } from '../doc';

export const doc: ComponentDoc = {
  name: 'Tabs',
  category: 'navigation',
  status: 'preview',
  summary: 'A row of tabs over one panel, as an underline or a segmented control',
  description:
    'Tabs switch one area between a few peers: sections of a page, or modes of one view. ' +
    'The tabs share the row equally, and a single indicator slides to the chosen one.',
  keywords: ['tabs', 'tab bar', 'segmented control', 'segments', 'switcher', 'sections', 'filter', 'range'],
  propsType: 'TabsProps',
  recipe: { map: 'tabsRecipe', parts: 'tabsParts' },
  anatomy: [
    { part: 'frame', required: true, description: 'The underline rule, or the sunken track of the segmented control.' },
    { part: 'list', required: true, description: 'The tab list: one Tab stop, named by `label`.' },
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
      { state: 'selected', from: 'value' },
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
      { what: 'segmented thumb edge', pair: ['surface-raised', 'surface-sunken'], minimum: '3:1' },
    ],
    notes: [
      'The chosen state is in the text weight and colour and in the indicator, never in colour alone.',
      'The segmented control is 36 px tall, with its 32 px segments; it suits pointer and compact layouts.',
    ],
  },
  guidance: {
    do: [
      'Keep to two to five short labels that fit the row without truncating.',
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
    { name: 'Filter', title: 'A segmented filter over a list' },
  ],
  gaps: [
    {
      missing: 'Tabs as wide as their labels, scrolling when they overflow, with the indicator following.',
      needs: 'a way to read a laid-out box\'s position and width from View, or a shared-element layout motion between siblings',
      today: 'Tabs share the row equally, so the indicator moves by a tab\'s width.',
    },
  ],
  related: ['Button', 'DropdownMenu'],
  gallery: {
    specimen: 'Specimen',
    tagline: 'One area, a few peers',
    order: 1,
    page: {
      lead: 'Use tabs to switch one area between a few peers. Each tab is a place, not an action.',
      hero: 'Sections',
      variants: [{ name: 'Segmented', title: 'Segmented: modes of one view' }],
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

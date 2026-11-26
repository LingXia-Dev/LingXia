import type { ComponentDoc } from '../doc';

export const doc: ComponentDoc = {
  name: 'InputGroup',
  category: 'inputs',
  status: 'preview',
  since: '2026-11-26',
  summary: 'A field joined edge to edge with fixed text, a select or a button, as one control',
  description:
    'InputGroup joins a TextInput with what completes it: fixed text such as a scheme or a unit (InputGroupText), a Select such as a currency, or the Button that acts on it. ' +
    'Neighbours share an edge and only the group\'s two ends are round; the text field takes the room the others leave. Each control keeps its own name, and the group is named once.',
  keywords: ['input group', 'input addon', 'prefix', 'suffix', 'unit', 'currency', 'joined field', 'attached input', 'field with button'],
  propsType: 'InputGroupProps',
  recipe: { map: 'inputGroupRecipe', parts: 'inputGroupParts' },
  anatomy: [
    { part: 'root', required: true, description: 'The row, a group assistive tech names by `label`. Each control rounds only the group\'s two ends.' },
    { part: 'text', required: false, description: 'InputGroupText: fixed text on a sunken box with the field\'s edge, so it reads as part of the value.' },
    { part: 'textLabel', required: false, description: 'The fixed text, muted and never cut short.' },
  ],
  accessibility: {
    pattern: null,
    name: 'label',
    description: [],
    states: [],
    keyboard: [{ keys: 'Tab', effect: 'Moves through the controls in the group in reading order' }],
    contrast: [
      { what: 'the fixed text', pair: ['text-muted', 'surface-sunken'], minimum: '4.5:1' },
      { what: 'the joined edges', pair: ['line-strong', 'surface'], minimum: '3:1' },
    ],
    notes: [
      'Role group, named by `label`; each control keeps its own name, so its visible label is left out and read instead.',
      'A focused field keeps its accent on the edge it shares: the second pixel of its focus edge is drawn inside it.',
    ],
  },
  guidance: {
    do: [
      'Join only what makes one value: a scheme and a host, a currency and an amount, a field and its one action.',
      'Give every control a `label`: it is what a screen reader says for it inside the group.',
      'Use InputGroupText for text the user never types; use TextInput\'s own `prefix` or `suffix` when the text belongs inside the field.',
    ],
    dont: [
      'Join two text fields; give each its own label above it.',
      'Put a primary button in the group; the joined button is secondary, so the group stays one quiet shape.',
    ],
  },
  examples: [
    { name: 'Website', title: 'Fixed text before the field' },
    { name: 'Price', title: 'A currency and an amount' },
    { name: 'Invite', title: 'A field and its action' },
    { name: 'Sizes', title: 'A unit after the value, at two sizes' },
  ],
  gaps: [
    {
      missing: 'A status message under the whole group.',
      needs: 'nothing from the View',
      today: 'A control in the group shows its error edge; say what is wrong in text near the group.',
    },
  ],
  playground: { props: { label: 'Price' }, fixture: true },
  related: ['TextInput', 'Select', 'ButtonGroup'],
  gallery: {
    specimen: 'Specimen',
    tagline: 'Joined fields',
    order: 16,
    page: {
      lead: 'Use an input group when a field needs fixed text, a choice or an action joined to it to make one value.',
      hero: 'Price',
      variants: [
        { name: 'Website', title: 'Fixed text before the field' },
        { name: 'Invite', title: 'A field and its action' },
      ],
      sizes: [{ name: 'Sizes', title: 'The group sets the size of its controls' }],
      states: [],
      inContext: [],
    },
  },
};

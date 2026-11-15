import type { ComponentDoc } from '../doc';

export const doc: ComponentDoc = {
  name: 'MultiSelect',
  category: 'selection',
  status: 'preview',
  since: '2026-11-15',
  summary: 'Several choices from a list, shown as tokens',
  description:
    'MultiSelect shows what is chosen as Tokens inside a field box and opens the whole list from a button at its end, each option with a check. ' +
    'A token\'s own button removes it without opening the list. It carries Field\'s label, help text and status. ' +
    'For a handful of options that fit on the page, a CheckboxList keeps them all in view.',
  keywords: ['multi select', 'multiselect', 'tokenizer', 'tags', 'chips', 'multiple', 'picker'],
  propsType: 'MultiSelectProps',
  recipe: { map: 'multiSelectRecipe', parts: 'multiSelectParts' },
  anatomy: [
    { part: 'box', required: true, description: 'The field box: a group named by the label, holding the tokens.' },
    { part: 'placeholder', required: false, description: 'Shown while nothing is chosen.' },
    { part: 'add', required: true, description: 'A DropdownMenu at the end of the box, named by `addLabel`, with a checkbox item per option.' },
  ],
  accessibility: {
    pattern: null,
    name: 'label',
    description: ['description', 'status'],
    states: [{ state: 'disabled', from: 'isDisabled, or isDisabled on an option' }],
    keyboard: [
      { keys: 'Tab', effect: 'To each token\'s remove button, then to the add button' },
      { keys: 'Enter or Space on the add button', effect: 'Opens the list' },
      { keys: 'Enter or Space on an option', effect: 'Checks or unchecks it; the list stays open' },
      { keys: 'Escape', effect: 'Closes the list' },
    ],
    target: { sm: 24 },
    contrast: [
      { what: 'boundary', pair: ['line-strong', 'surface'], minimum: '3:1' },
      { what: 'placeholder', pair: ['text-muted', 'surface'], minimum: '4.5:1' },
    ],
    notes: [
      'The box is a group named by the field\'s label and described by its help and status.',
      'Each option is a menu item that reports checked; each token\'s remove button says what it removes.',
      'The chosen values keep the options\' order, so the tokens do not jump as more are added.',
    ],
  },
  guidance: {
    do: [
      'Use it when the list is too long to leave open, or the field must stay one line tall when little is chosen.',
      'Name the add button for what is added: "Add a label".',
    ],
    dont: [
      'Use it for two to six options that fit; a CheckboxList shows them all.',
      'Use it for one choice; that is a Select or a Combobox.',
    ],
  },
  examples: [
    { name: 'Labels', title: 'Labels for a note' },
    { name: 'Empty', title: 'Nothing chosen, with an error' },
  ],
  gaps: [],
  limits: [
    'The list is not searched by typing, and new values cannot be typed in: it chooses from the options given.',
    'The error state colours the boundary but is not reported as invalid on the group.',
  ],
  related: ['Token', 'CheckboxList', 'Combobox', 'Select'],
  gallery: {
    specimen: 'Specimen',
    tagline: 'Several of many',
    order: 9,
    page: {
      lead: 'Use a multi-select to choose several things from a long list.',
      hero: 'Labels',
      variants: [
        { name: 'Empty', title: 'Empty and required, with an error' },
        { name: 'StateDisabled', title: 'Disabled, with the reason' },
      ],
    },
  },
};

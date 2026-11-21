import type { ComponentDoc } from '../doc';

export const doc: ComponentDoc = {
  name: 'ListInput',
  category: 'inputs',
  status: 'preview',
  since: '2026-11-15',
  summary: 'A field of rows the user adds and removes',
  description:
    'ListInput is one field whose value is a list: phone numbers, links, line items. Each row holds the app\'s own controls with a remove button at its end, and a button under the rows adds one. ' +
    'The app owns the rows and draws each one; ListInput gives them a shared label, help text and status, reads them as a list, and keeps the count between `min` and `max`.',
  keywords: ['list', 'repeat', 'rows', 'add', 'remove', 'array', 'multiple', 'field'],
  propsType: 'ListInputProps',
  recipe: { map: 'listInputRecipe', parts: 'listInputParts' },
  anatomy: [
    { part: 'list', required: false, description: 'The rows: a list named by the label; absent while there are none.' },
    { part: 'row', required: false, description: 'One row: the app\'s controls, then its remove button.' },
    { part: 'controls', required: false, description: 'What `renderRow` returns; it takes the row\'s width.' },
    { part: 'add', required: true, description: 'The button that adds a row, under the rows.' },
  ],
  accessibility: {
    pattern: null,
    name: 'label',
    description: ['description', 'status'],
    states: [{ state: 'disabled', from: 'isDisabled; the add button at `max`, the remove buttons at `min`' }],
    keyboard: [
      { keys: 'Tab', effect: 'Through each row\'s controls and its remove button, then to the add button' },
      { keys: 'Enter or Space', effect: 'Removes the row, or adds one' },
    ],
    target: { md: 40 },
    contrast: [
      { what: 'remove icon', pair: ['text', 'surface'], minimum: '3:1' },
    ],
    notes: [
      'The rows are a list named by the label and described by the help text and status.',
      'Each control in a row needs its own name with the row\'s number, hidden: "Phone 2". The field\'s label names the list, not the controls.',
      'Each remove button says which row it removes.',
    ],
  },
  guidance: {
    do: [
      'Start with one empty row when at least one is needed, and set `min` to 1.',
      'Key rows by an id the app makes when a row is added, so a row keeps its text when another is removed.',
      'At large text, let a row of controls wrap (`flex-wrap`, least widths, no fixed widths) so each keeps its words whole.',
    ],
    dont: [
      'Use it for choosing among known options; that is MultiSelect or CheckboxList.',
      'Put more than two or three controls in a row on a phone; open the row in a sheet when it needs more.',
    ],
  },
  examples: [
    { name: 'Phones', title: 'Rows of two controls, between one and four' },
    { name: 'Links', title: 'Empty and required' },
  ],
  gaps: [],
  limits: [
    'Rows are not reordered by dragging.',
    'It does not move focus to a new row; the app\'s controls can ask for it.',
  ],
  playground: { props: { label: 'Phone numbers', addLabel: 'Add a phone number', min: 1, max: 4 }, fixture: true },
  related: ['Field', 'MultiSelect', 'FormLayout'],
  gallery: {
    specimen: 'Specimen',
    tagline: 'Rows to add',
    order: 15,
    page: {
      lead: 'Use a list input when one field holds several values that are typed, not picked.',
      hero: 'Phones',
      variants: [{ name: 'Links', title: 'Empty and required: the error says what to add' }],
      statesLayout: 'stack',
      states: [{ name: 'StateDisabled', title: 'Disabled, with the reason' }],
    },
  },
};

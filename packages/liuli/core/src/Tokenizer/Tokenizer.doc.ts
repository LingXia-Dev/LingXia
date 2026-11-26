import type { ComponentDoc } from '../doc';

export const doc: ComponentDoc = {
  name: 'Tokenizer',
  category: 'inputs',
  status: 'preview',
  since: '2026-11-26',
  summary: 'Typed entries that become tokens',
  description:
    'Tokenizer is a field whose value is a list of words the user types: addresses, tags, keywords. Enter, a comma, a semicolon or a line break turns the text into a token, a pasted list splits into one token each, and leaving the field keeps what was typed. ' +
    'An entry already there is not added twice. With `validate`, a refused entry stays in the editor with the reason as the field\'s error. It carries Field\'s label, help text and status.',
  keywords: ['tokenizer', 'tags', 'chips', 'token input', 'recipients', 'emails', 'keywords', 'free text', 'multiple'],
  propsType: 'TokenizerProps',
  recipe: { map: 'tokenizerRecipe', parts: 'tokenizerParts' },
  anatomy: [
    { part: 'box', required: true, description: 'The field box: a group named by the label, holding the tokens and the editor.' },
    { part: 'editor', required: true, description: 'The text after the last token, named by the label; it wraps to a line of its own when the tokens fill the line.' },
  ],
  accessibility: {
    pattern: null,
    name: 'label',
    description: ['description', 'status'],
    states: [{ state: 'disabled', from: 'isDisabled' }, { state: 'invalid', from: 'status, or an entry validate refused' }],
    keyboard: [
      { keys: 'Enter', effect: 'Turns the typed text into a token' },
      { keys: ', or ;', effect: 'Ends the entry before it, which becomes a token' },
      { keys: 'Tab', effect: 'To each token\'s remove button, then the editor; leaving the editor keeps what was typed' },
    ],
    target: { sm: 24 },
    contrast: [
      { what: 'boundary', pair: ['line-strong', 'surface'], minimum: '3:1' },
      { what: 'placeholder', pair: ['text-muted', 'surface'], minimum: '4.5:1' },
    ],
    notes: [
      'The editor is named by the field\'s label and described by its help and status; a refused entry is announced as the field\'s error.',
      'Each token\'s remove button says what it removes.',
    ],
  },
  guidance: {
    do: [
      'Use it when the entries are the user\'s own words: addresses, tags, search terms.',
      'Check addresses or codes with `validate`, so a typo is fixed before it becomes a token.',
    ],
    dont: [
      'Use it to choose from a known list; a MultiSelect offers the options.',
      'Use it for one value; that is a TextInput.',
    ],
  },
  examples: [
    { name: 'Recipients', title: 'Email addresses, checked as they are entered' },
    { name: 'Tags', title: 'Free tags, small' },
    { name: 'Empty', title: 'Nothing entered, with an error' },
  ],
  gaps: [],
  limits: [
    'Backspace in an empty editor does not remove the last token; its own button does.',
    'It suggests nothing while typing; a Combobox suggests from a list.',
  ],
  playground: { props: { label: 'Tags', placeholder: 'Add a tag' }, fixture: true },
  related: ['MultiSelect', 'Token', 'TextInput', 'Combobox'],
  gallery: {
    specimen: 'Specimen',
    tagline: 'Words into tokens',
    order: 17,
    page: {
      lead: 'Use a tokenizer to enter several words of the user\'s own.',
      hero: 'Recipients',
      variants: [
        { name: 'Tags', title: 'Free tags, in the small size' },
        { name: 'Empty', title: 'Empty and required, with an error' },
        { name: 'StateDisabled', title: 'Disabled, with the reason' },
      ],
    },
  },
};

import type { ComponentDoc } from '../doc';

export const doc: ComponentDoc = {
  name: 'TextArea',
  category: 'inputs',
  status: 'preview',
  summary: 'Multi-line text field that grows with its text, with label, help text and status',
  description:
    'TextArea is a labelled editor for notes, comments, replies and descriptions. It shows `rows` lines, grows with the text ' +
    'up to `maxRows`, then scrolls. Bind it to a `useLocal` cell, or pass `value` and `onChange` when the app owns the text.',
  keywords: ['textarea', 'text area', 'multiline', 'multi-line', 'note', 'comment', 'reply', 'message', 'description', 'form'],
  propsType: 'TextAreaProps',
  recipe: { map: 'textAreaRecipe', parts: 'textAreaParts' },
  anatomy: [
    { part: 'box', required: true, description: 'The bordered box; it takes the focus colour while the editor has focus and the status colour otherwise.' },
    { part: 'editor', required: true, description: 'The editor; it fills the box, shows `rows` lines and grows to `maxRows`.' },
  ],
  accessibility: {
    pattern: 'textbox',
    name: 'label',
    description: ['description, or disabledMessage while disabled', 'the status message'],
    states: [
      { state: 'disabled', from: 'isDisabled' },
      { state: 'invalid', from: "status.type === 'error'" },
      { state: 'required', from: 'isRequired' },
    ],
    keyboard: [
      { keys: 'Return', effect: 'a new line; with enterKeyHint send, go, done or search it runs onSubmit' },
      { keys: 'Shift+Return', effect: 'a new line when Return submits' },
    ],
    contrast: [
      { what: 'text', pair: ['text', 'surface'], minimum: '4.5:1' },
      { what: 'placeholder', pair: ['text-muted', 'surface'], minimum: '4.5:1' },
      { what: 'box boundary', pair: ['line-strong', 'surface'], minimum: '3:1' },
      { what: 'focus boundary', pair: ['focus', 'surface'], minimum: '3:1' },
    ],
    notes: [
      'The editor is named by the label even when the label is hidden; the placeholder is not its name.',
      'Growing does not move focus or the caret; past `maxRows` the editor scrolls its own text.',
    ],
  },
  guidance: {
    do: [
      'Use it when the answer may run past one line: a note, a comment, a reason.',
      'Set `rows` to the length you expect, so the box says how much to write.',
      'Use enterKeyHint="send" for a chat or reply box, so Return sends and Shift+Return adds a line.',
    ],
    dont: [
      'Use it for a name, an email or a code; use TextInput.',
      'Let it grow without a `maxRows` in a sheet or a short screen; the keyboard covers the rest.',
    ],
  },
  examples: [
    { name: 'Bound', title: 'Bound to local state, growing to six lines' },
    { name: 'Reply', title: 'A reply box that sends on Return' },
    { name: 'Invalid', title: 'Required, with an error' },
  ],
  gaps: [
    {
      missing: 'Read-only text that can still be selected and copied.',
      needs: 'readOnly on editors',
      today: 'Show the value as selectable text instead.',
    },
    {
      missing: 'Validation on leaving the field.',
      needs: 'focus and blur events on editors',
      today: 'Validate as the value changes, or on submit.',
    },
  ],
  related: ['TextInput', 'Field'],
  gallery: {
    specimen: 'Specimen',
    tagline: 'Several lines',
    order: 7,
    page: {
      lead: 'Use a text area when the answer may run to several lines: a note, a comment, a reply.',
      hero: 'Bound',
      variants: [{ name: 'Reply', title: 'A reply box that sends on Return' }],
      sizes: [{ name: 'Sizes', title: 'Small, medium and large' }],
      states: [
        { name: 'StateEmpty', title: 'Empty' },
        { name: 'StateFilled', title: 'Filled' },
        { name: 'StateInvalid', title: 'Invalid' },
        { name: 'StateDisabled', title: 'Disabled' },
      ],
      inContext: [{ name: 'Invalid', title: 'Required, with an error' }],
    },
  },
};

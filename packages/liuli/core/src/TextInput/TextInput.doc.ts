import type { ComponentDoc } from '../doc';

export const doc: ComponentDoc = {
  name: 'TextInput',
  category: 'form',
  status: 'lab',
  summary: 'Single-line text field with label, help text, status and an optional clear button',
  description:
    'TextInput is a labelled one-line editor for names, emails, codes, amounts and searches. ' +
    'Bind it to a `useLocal` cell for View-local drafts, or pass `value` and `onChange` when the app owns the text.',
  keywords: ['text input', 'text field', 'input', 'textbox', 'email', 'password', 'search', 'form'],
  propsType: 'TextInputProps',
  recipe: { map: 'textInputRecipe', parts: 'textInputParts' },
  anatomy: [
    { part: 'box', required: true, description: 'The bordered box; it takes the focus colour while the editor has focus and the status colour otherwise.' },
    { part: 'icon', required: false, description: '`startIcon`, hidden from assistive tech.' },
    { part: 'input', required: true, description: 'The editor; it fills the box height so taps above and below the text land in it.' },
    { part: 'clear', required: false, description: 'Empties the text; shown while there is text and `hasClear` is on.' },
  ],
  accessibility: {
    pattern: 'textbox',
    name: 'label',
    states: [{ state: 'disabled', from: 'isDisabled' }],
    keyboard: [{ keys: 'the keyboard action key, Enter', effect: 'runs onSubmit' }],
    target: { sm: 32, md: 40, lg: 48 },
    contrast: [
      { what: 'text', pair: ['text', 'surface'], minimum: '4.5:1', proven: true },
      { what: 'placeholder', pair: ['text-muted', 'surface'], minimum: '4.5:1', proven: true },
      { what: 'box boundary', pair: ['line-strong', 'surface'], minimum: '3:1', proven: true },
      { what: 'focus boundary', pair: ['focus', 'surface'], minimum: '3:1', proven: true },
    ],
    notes: ['The clear button is a separate control named by `clearLabel`.'],
  },
  guidance: {
    do: [
      'Bind to useLocal for drafts; send the value to Logic on submit, not on every keystroke.',
      'Set type, inputMode, autoComplete and enterKeyHint so the right keyboard and autofill appear.',
      'Explain the fix in the status message.',
    ],
    dont: [
      'Leave out label because a placeholder is shown; use isLabelHidden when the label must not be drawn.',
      'Use type="number" for codes, phone numbers or card numbers; use inputMode="numeric".',
    ],
  },
  examples: [
    { name: 'Bound', title: 'Bound to local state, with validation' },
    { name: 'Search', title: 'Search with a clear button' },
    { name: 'Disabled', title: 'Disabled, with the reason' },
  ],
  gaps: [
    {
      missing: 'Validation on leaving the field; dirty tracking.',
      needs: 'focus and blur events on editors',
      today: 'Validate as the value changes, or on submit.',
    },
    {
      missing: 'Help text and status tied to the editor; invalid and required reported to assistive tech.',
      needs: 'aria-describedby, aria-invalid and aria-required on editors',
      today: 'Read after the editor in tree order; required is an asterisk only.',
    },
    {
      missing: 'Read-only text that can still be selected and copied.',
      needs: 'readOnly on editors',
      today: 'Show the value as selectable text instead.',
    },
    {
      missing: 'Moving focus to the next field from code when the app owns the value.',
      needs: 'a focus handle for controlled editors',
      today: 'Bound fields chain with enterKeyHint="next" or cell.focus().',
    },
  ],
  related: ['Field'],
};

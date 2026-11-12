import type { ComponentDoc } from '../doc';

export const doc: ComponentDoc = {
  name: 'Field',
  category: 'form',
  status: 'lab',
  summary: 'Label, help text and validation status around one form control',
  description:
    'Field lays out the parts every form control shares: a label, the control, help text and a status message. ' +
    'Liuli form controls use it inside; wrap a raw primitive in it only when Liuli has no component for that control yet.',
  keywords: ['field', 'form', 'label', 'help text', 'error', 'validation', 'required'],
  propsType: 'FieldProps',
  recipe: { map: 'fieldRecipe', parts: 'fieldParts' },
  anatomy: [
    { part: 'label', required: true, description: 'The visible label; hidden from assistive tech because the control carries the same text as its name.' },
    { part: 'required', required: false, description: 'An asterisk after the label when `isRequired`.' },
    { part: 'description', required: false, description: 'Help text, or `disabledMessage` while disabled.' },
    { part: 'status', required: true, description: 'A polite live region, always mounted; it shows an icon and the message when `status` has one.' },
  ],
  accessibility: {
    pattern: null,
    name: 'label',
    states: [{ state: 'disabled', from: 'isDisabled, passed on to the control by the component that uses Field' }],
    keyboard: [],
    contrast: [
      { what: 'label', pair: ['text', 'canvas'], minimum: '4.5:1', proven: true },
      { what: 'help text', pair: ['text-muted', 'canvas'], minimum: '4.5:1', proven: true },
      { what: 'status icon', pair: ['danger', 'canvas'], minimum: '3:1', proven: false },
    ],
    notes: [
      'Help text and status are read after the control, in tree order; they are not yet tied to it.',
      'The status region announces a message politely when it appears or changes.',
    ],
  },
  guidance: {
    do: [
      'Give the wrapped control aria-label equal to the Field label.',
      'Say what to fix in a status message, not only that something is wrong.',
    ],
    dont: [
      'Use the placeholder as the label; it disappears as soon as someone types.',
      'Wrap a Liuli form control in a Field; it already has one.',
    ],
  },
  examples: [{ name: 'WrappedSelect', title: 'Wrapping a primitive select' }],
  gaps: [
    {
      missing: 'Help text and status tied to the control; invalid and required reported.',
      needs: 'aria-describedby, aria-invalid and aria-required on controls',
      today: 'The control is named by the label; help and status are read after it; required is visual only.',
    },
    {
      missing: 'Validation when the user leaves a field.',
      needs: 'focus and blur events on editors',
      today: 'Validate on change or on submit.',
    },
  ],
  related: ['TextInput', 'CheckboxInput'],
};

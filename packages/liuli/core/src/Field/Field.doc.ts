import type { ComponentDoc } from '../doc';

export const doc: ComponentDoc = {
  name: 'Field',
  category: 'inputs',
  status: 'preview',
  since: '2026-11-12',
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
    description: ['description, or disabledMessage while disabled', 'the status message'],
    states: [
      { state: 'disabled', from: 'isDisabled' },
      { state: 'invalid', from: "status.type === 'error'" },
      { state: 'required', from: 'isRequired' },
    ],
    keyboard: [],
    contrast: [
      { what: 'label', pair: ['text', 'canvas'], minimum: '4.5:1' },
      { what: 'help text', pair: ['text-muted', 'canvas'], minimum: '4.5:1' },
      { what: 'status icon', pair: ['danger', 'canvas'], minimum: '3:1' },
    ],
    notes: [
      'Field hands its control the name, the description ids and the states; pass each one on to the control element.',
      'The status region also announces a message politely when it appears or changes.',
    ],
  },
  guidance: {
    do: [
      'Pass every prop Field hands the control on to the control element.',
      'Say what to fix in a status message, not only that something is wrong.',
    ],
    dont: [
      'Use the placeholder as the label; it disappears as soon as someone types.',
      'Wrap a Liuli form control in a Field; it already has one.',
    ],
  },
  examples: [{ name: 'WrappedSelect', title: 'Wrapping a primitive select' }, { name: 'Info', title: 'A note behind an info button' }],
  gaps: [
    {
      missing: 'Validation when the user leaves a field.',
      needs: 'focus and blur events on editors',
      today: 'Validate on change or on submit.',
    },
  ],
  related: ['TextInput', 'CheckboxInput'],
  gallery: {
    specimen: 'Specimen',
    tagline: 'Label, help, status',
    order: 6,
    page: {
      lead: 'Use a field to give a control Liuli\'s label, help text and status when Liuli has no component for it yet.',
      hero: 'WrappedSelect',
      variants: [{ name: 'Info', title: 'A note behind an info button beside the label' }],
      states: [
        { name: 'StateHelp', title: 'With help' },
        { name: 'StateWarning', title: 'Warning' },
        { name: 'StateError', title: 'Error' },
        { name: 'StateDisabled', title: 'Disabled' },
      ],
    },
  },
};

import type { ComponentDoc } from '../doc';

export const doc: ComponentDoc = {
  name: 'RadioGroup',
  category: 'selection',
  status: 'preview',
  since: '2026-11-12',
  summary: 'One choice from a short list, every option in view',
  description:
    'RadioGroup picks one option from two to six, all visible at once, as a plain list or as cards. ' +
    'It is a Field: the label names the group, and help text and status describe it.',
  keywords: ['radio', 'radio group', 'radio button', 'option', 'choice', 'plan', 'pick one', 'single select'],
  propsType: 'RadioGroupProps',
  recipe: { map: 'radioGroupRecipe', parts: 'radioGroupParts' },
  anatomy: [
    { part: 'field', required: true, description: 'Field: the group label, help text and status.' },
    { part: 'group', required: true, description: 'The radio group: one Tab stop, named by the label.' },
    { part: 'option', required: true, description: 'One radio per option; the whole row or card is the press target.' },
    { part: 'circle', required: true, description: 'Drawn from the choice: filled with the accent and a dot when chosen.' },
    { part: 'label', required: true, description: 'The option\'s text and its name.' },
    { part: 'description', required: false, description: 'A second line, read as the option\'s description.' },
  ],
  accessibility: {
    pattern: 'radioGroup',
    name: 'label',
    description: ['description, or disabledMessage while disabled', 'the status message'],
    states: [
      { state: 'checked', from: 'value or bind, on the chosen option' },
      { state: 'disabled', from: 'isDisabled, or the option\'s isDisabled' },
      { state: 'invalid', from: "status.type === 'error'" },
      { state: 'required', from: 'isRequired' },
    ],
    keyboard: [
      { keys: 'Tab', effect: 'Into the group, on the chosen option or else the first.' },
      { keys: 'Up / Left, Down / Right', effect: 'The previous or next option, chosen on arrival; wraps at the ends.' },
    ],
    target: { sm: 32, md: 40, lg: 48 },
    contrast: [
      { what: 'circle boundary, not chosen', pair: ['line-strong', 'surface'], minimum: '3:1' },
      { what: 'circle fill, chosen', pair: ['accent-solid', 'surface'], minimum: '3:1' },
      { what: 'chosen card edge', pair: ['accent', 'surface'], minimum: '3:1' },
    ],
    notes: [
      'A card shows the choice twice, in its edge and in the circle, so it never rests on colour alone.',
      'With nothing chosen, Tab lands on the first option without choosing it.',
    ],
  },
  guidance: {
    do: [
      'At large text, expect the circle to keep its size, centred on the label\'s first line as that line grows; the label wraps, the mark is never squeezed.',
      'Choose a sensible default when one exists; leave the group empty only when the person must decide.',
      'Use cards when each option needs a second line: plans, delivery, shipping speed.',
    ],
    dont: [
      'Use radios for more than about six options; use a Select.',
      'Use radios for a choice that takes effect at once in a toolbar; use Tabs in the segmented variant.',
    ],
  },
  examples: [
    { name: 'Delivery', title: 'Options with a second line' },
    { name: 'Plans', title: 'Cards' },
    { name: 'Plain', title: 'Short options with help text' },
    { name: 'Required', title: 'Required, with an error' },
    { name: 'Sizes', title: 'Sizes' },
    { name: 'Inline', title: 'Side by side' },
  ],
  gaps: [],
  related: ['CheckboxInput', 'Field', 'Tabs'],
  gallery: {
    specimen: 'Specimen',
    tagline: 'One of a few',
    order: 2,
    page: {
      lead: 'Use radios to pick one option from a short list when seeing every option helps the choice.',
      hero: 'Delivery',
      variants: [
        { name: 'Plans', title: 'Cards: the whole card chooses' },
        { name: 'Plain', title: 'A plain list with help text' },
        { name: 'Inline', title: 'Horizontal, for two or three short options' },
      ],
      sizes: [{ name: 'Sizes', title: 'Small, medium and large' }],
      states: [
        { name: 'StateDefault', title: 'Default' },
        { name: 'StateInvalid', title: 'Invalid' },
        { name: 'StateDisabledOption', title: 'A disabled option' },
        { name: 'StateDisabled', title: 'Disabled' },
      ],
      inContext: [{ name: 'Required', title: 'A required question, after a failed send' }],
    },
  },
};

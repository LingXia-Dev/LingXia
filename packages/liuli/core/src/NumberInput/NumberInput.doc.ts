import type { ComponentDoc } from '../doc';

export const doc: ComponentDoc = {
  name: 'NumberInput',
  category: 'inputs',
  status: 'preview',
  since: '2026-11-15',
  summary: 'A number typed or nudged by steps, kept between a minimum and a maximum',
  description:
    'NumberInput holds one number: a count of guests, a weight, a price. People can type it, or nudge it with the two steppers at the end of the box. ' +
    'It carries a number, or `null` while the field is empty, so the app never parses text. A unit or a currency sign can sit inside the box, ' +
    'and it takes Field\'s label, help text and status.',
  keywords: ['number', 'number input', 'stepper', 'quantity', 'counter', 'spinbutton', 'amount', 'numeric'],
  propsType: 'NumberInputProps',
  recipe: { map: 'numberInputRecipe', parts: 'numberInputParts' },
  anatomy: [
    { part: 'box', required: true, description: 'TextInput\'s box, holding the number, any fixed text and the steppers.' },
    { part: 'input', required: true, description: 'The editor, in tabular figures; the element assistive tech names by `label`.' },
    { part: 'adornment', required: false, description: '`prefix` and `suffix`: fixed text beside the number, read with the description.' },
    { part: 'steppers', required: false, description: 'Minus and plus at the end of the box, behind a hairline.' },
    { part: 'stepper', required: false, description: 'One stepper: a square as tall as the box, named by `decrementLabel` or `incrementLabel`, disabled at its end of the range.' },
  ],
  accessibility: {
    pattern: 'textbox',
    name: 'label',
    description: ['prefix', 'suffix', 'optionalLabel', 'description', 'status message'],
    states: [{ state: 'invalid', from: 'status.type === "error"' }, { state: 'required', from: 'isRequired' }, { state: 'disabled', from: 'isDisabled' }],
    keyboard: [
      { keys: 'Tab', effect: 'Moves from the number to the minus stepper, then the plus stepper' },
      { keys: 'Enter or Space', effect: 'On a stepper: one step down or up' },
    ],
    target: { sm: 32, md: 40, lg: 48 },
    contrast: [
      { what: 'number', pair: ['text', 'surface'], minimum: '4.5:1' },
      { what: 'unit, placeholder and stepper marks', pair: ['text-muted', 'surface'], minimum: '4.5:1' },
      { what: 'field edge', pair: ['line-strong', 'surface'], minimum: '3:1' },
      { what: 'error edge', pair: ['danger', 'surface'], minimum: '3:1' },
    ],
    notes: [
      'A number editor named by `label`; the unit, then the help text and status, are read as its description.',
      'Each stepper is a button with its own name from the app, so a screen reader hears what a press changes: "More guests".',
      'A stepper at its end of the range is disabled, and the field keeps typed numbers within `min` and `max` when it loses focus.',
      'The hairlines beside the steppers are decoration; each stepper is told apart by its mark.',
    ],
  },
  guidance: {
    do: [
      'Use steppers for small counts people adjust by one or two; leave them out with `hasSteppers={false}` for numbers people type.',
      'Set `min`, `max` and `step` to what the number can be; `step` also sets how many decimals a stepper keeps.',
      'Put the unit in `suffix` or the currency in `prefix`, not in the label.',
      'Name the steppers for what changes: "Fewer guests", "More guests".',
    ],
    dont: [
      'Use it for numbers that are not amounts: a phone number, a postcode, a card number. Those are text.',
      'Use it for a value picked by feel along a range; that is a Slider.',
      'Reject a number while it is being typed; show a status once it is wrong.',
    ],
  },
  examples: [
    { name: 'Guests', title: 'A count with steppers' },
    { name: 'Weight', title: 'A unit, and steps of a tenth' },
    { name: 'Price', title: 'A typed number, without steppers' },
    { name: 'Sizes', title: 'Sizes' },
  ],
  gaps: [
    {
      missing: 'The number shown formatted (grouped digits, a fixed number of decimals) while the field is not being edited.',
      needs: 'focus and blur events on an editor',
      today: 'The field shows the digits as typed; a unit or currency sits beside them.',
    },
    {
      missing: 'Up and Down arrow keys stepping the number from the editor, and the spin-button role.',
      needs: 'key events from an editor and a spinbutton role',
      today: 'The steppers are two buttons after the editor in the Tab order.',
    },
    {
      missing: 'A stepper that repeats while held.',
      needs: 'a press-and-hold event',
      today: 'One step per press.',
    },
  ],
  related: ['TextInput', 'Slider'],
  gallery: {
    specimen: 'Specimen',
    tagline: 'A number, by steps',
    order: 8,
    page: {
      lead: 'Use a number input for an amount people type or nudge: a count, a weight, a price.',
      hero: 'Guests',
      variants: [
        { name: 'Weight', title: 'A unit after the number, and steps of a tenth' },
        { name: 'Price', title: 'A typed number: a currency sign, no steppers, marked optional' },
      ],
      sizes: [{ name: 'Sizes', title: 'Small, medium and large' }],
      states: [
        { name: 'StateEmpty', title: 'Empty' },
        { name: 'StateAtMinimum', title: 'At the minimum' },
        { name: 'StateInvalid', title: 'Invalid' },
        { name: 'StateDisabled', title: 'Disabled' },
      ],
      statesLayout: 'stack',
    },
  },
};

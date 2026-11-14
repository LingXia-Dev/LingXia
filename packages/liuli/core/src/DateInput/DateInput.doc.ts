import type { ComponentDoc } from '../doc';

export const doc: ComponentDoc = {
  name: 'DateInput',
  category: 'inputs',
  status: 'preview',
  since: '2026-11-13',
  summary: 'A labelled date field that opens a month picker',
  description:
    'DateInput takes one day: a due date, a birthday, the day a trip starts. A tap opens the View\'s month picker, which ' +
    'marks today, greys days outside `min` and `max`, and starts weeks on the user\'s first weekday. It has Field\'s label, ' +
    'help text and status. The value is `YYYY-MM-DD`, or `\'\'` while empty.',
  keywords: ['date input', 'date picker', 'calendar', 'day', 'due date', 'birthday', 'form', 'field'],
  propsType: 'DateInputProps',
  recipe: { map: 'dateInputRecipe', parts: 'dateInputParts' },
  anatomy: [
    { part: 'box', required: true, description: 'TextInput\'s bordered box; it takes the focus colour while the field is focused and the status colour otherwise.' },
    { part: 'input', required: true, description: 'The date editor; the day shows in the user\'s language, and a tap opens the picker.' },
    { part: 'icon', required: true, description: 'A calendar mark at the end, hidden from assistive tech.' },
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
      { keys: 'Enter, Space', effect: 'Opens the picker.' },
      { keys: 'Escape', effect: 'Closes the picker without a change.' },
      { keys: 'Return', effect: 'In the picker, picks; then onSubmit runs.' },
      { keys: 'Arrows, Page Up, Page Down', effect: 'In the picker, a day, a week, a month.' },
    ],
    target: { sm: 32, md: 40, lg: 48 },
    contrast: [
      { what: 'value', pair: ['text', 'surface'], minimum: '4.5:1' },
      { what: 'placeholder', pair: ['text-muted', 'surface'], minimum: '4.5:1' },
      { what: 'box boundary', pair: ['line-strong', 'surface'], minimum: '3:1' },
      { what: 'end mark', pair: ['text-muted', 'surface'], minimum: '3:1' },
    ],
    notes: [
      'The picker is the View\'s own, drawn in the theme\'s roles: a popover under the field with a pointer, a sheet with Cancel and Done on touch.',
      'Month and day names follow the display language; the week starts on the user\'s first weekday.',
      'Sizes sm (32) and md (40) are below the 44 pt touch minimum; on touch layouts prefer lg.',
    ],
  },
  guidance: {
    do: [
      'Bound the range with `min` and `max` when only some days make sense.',
      'Tie a second date to the first, as a return to its departure, through `min`.',
    ],
    dont: [
      'Parse the value as local midnight in Logic without a time zone; it is a calendar day.',
      'Use it for a time of day; that is TimeInput.',
    ],
  },
  examples: [
    { name: 'DueDate', title: 'A due date' },
    { name: 'Trip', title: 'A return tied to its departure' },
    { name: 'Sizes', title: 'Sizes' },
  ],
  gaps: [],
  related: ['TimeInput', 'TextInput', 'Field'],
  gallery: {
    specimen: 'Specimen',
    tagline: 'One day',
    order: 3,
    page: {
      lead: 'Use a date field to take one day, picked from a month.',
      hero: 'DueDate',
      sizes: [{ name: 'Sizes', title: 'Small, medium and large' }],
      statesLayout: 'stack',
      states: [
        { name: 'StateEmpty', title: 'Empty' },
        { name: 'StateFilled', title: 'Filled' },
        { name: 'StateInvalid', title: 'Invalid' },
        { name: 'StateDisabled', title: 'Disabled' },
      ],
      inContext: [{ name: 'Trip', title: 'A trip: leave, then return' }],
    },
  },
};

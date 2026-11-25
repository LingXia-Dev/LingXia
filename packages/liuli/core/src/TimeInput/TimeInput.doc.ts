import type { ComponentDoc } from '../doc';

export const doc: ComponentDoc = {
  name: 'TimeInput',
  category: 'inputs',
  status: 'preview',
  since: '2026-11-13',
  summary: 'A labelled time field that opens an hour and minute picker',
  description:
    'TimeInput takes a time of day: a reminder, an alarm, the start of quiet hours. A tap opens the View\'s time picker, ' +
    'in the user\'s 12- or 24-hour clock; `step` sets the minutes offered. It has Field\'s label, help text and status. ' +
    'The value is `HH:MM` (or `HH:MM:SS`), or `\'\'` while empty.',
  keywords: ['time input', 'time picker', 'clock', 'hour', 'minute', 'reminder', 'alarm', 'form', 'field'],
  propsType: 'TimeInputProps',
  recipe: { map: 'timeInputRecipe', parts: 'timeInputParts' },
  anatomy: [
    { part: 'box', required: true, description: 'TextInput\'s bordered box; it takes the focus colour while the field is focused and the status colour otherwise.' },
    { part: 'input', required: true, description: 'The time editor; the time shows in the user\'s clock, and a tap opens the picker.' },
    { part: 'icon', required: true, description: 'A clock mark at the start, before the value in either direction, that grows with the text; hidden from assistive tech.' },
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
      { keys: 'Digits', effect: 'In the picker, fill HH:MM.' },
    ],
    target: { sm: 32, md: 40, lg: 48 },
    contrast: [
      { what: 'value', pair: ['text', 'surface'], minimum: '4.5:1' },
      { what: 'placeholder', pair: ['text-muted', 'surface'], minimum: '4.5:1' },
      { what: 'box boundary', pair: ['line-strong', 'surface'], minimum: '3:1' },
      { what: 'start mark', pair: ['text-muted', 'surface'], minimum: '3:1' },
    ],
    notes: [
      'The picker is the View\'s own, drawn in the theme\'s roles: a popover under the field with a pointer, a sheet with Cancel and Done on touch.',
      'The picker follows the user\'s 12- or 24-hour setting; the value is always 24-hour.',
      'Sizes sm (32) and md (40) are shorter than the 44 pt touch minimum; a finger still reaches them in a 44 pt band around the box, which the host adds without changing what is drawn.',
    ],
  },
  guidance: {
    do: [
      'Set `step` to the precision people need: 300 or 900 seconds for most schedules.',
      'Say in the description what the time repeats on, if it does.',
    ],
    dont: [
      'Use it for a duration; a time of day is not an amount of time.',
      'Use it for a date and time together without a DateInput beside it.',
    ],
  },
  examples: [
    { name: 'Reminder', title: 'A daily reminder' },
    { name: 'Clearable', title: 'Clearable: the clear button empties the time' },
    { name: 'QuietHours', title: 'Every 15 minutes, within the evening' },
    { name: 'Sizes', title: 'Sizes' },
  ],
  gaps: [],
  playground: { props: { value: '', label: 'Reminder' } },
  related: ['DateInput', 'TextInput', 'Field'],
  gallery: {
    specimen: 'Specimen',
    tagline: 'A time of day',
    order: 4,
    page: {
      lead: 'Use a time field to take a time of day, in the user\'s own clock.',
      hero: 'Reminder',
      variants: [{ name: 'Clearable', title: 'Clearable: the clear button empties the time' }],
      sizes: [{ name: 'Sizes', title: 'Small, medium and large' }],
      statesLayout: 'stack',
      states: [
        { name: 'StateEmpty', title: 'Empty' },
        { name: 'StateFilled', title: 'Filled' },
        { name: 'StateInvalid', title: 'Invalid' },
        { name: 'StateDisabled', title: 'Disabled' },
      ],
      inContext: [{ name: 'QuietHours', title: 'Quiet hours' }],
    },
  },
};

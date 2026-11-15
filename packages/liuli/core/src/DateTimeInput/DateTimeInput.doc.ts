import type { ComponentDoc } from '../doc';

export const doc: ComponentDoc = {
  name: 'DateTimeInput',
  category: 'inputs',
  status: 'preview',
  since: '2026-11-15',
  summary: 'A day and a time of day as one value',
  description:
    'DateTimeInput puts a DateInput and a TimeInput side by side under one label, help text and status, and gives the app one value, `YYYY-MM-DDTHH:mm`, once both parts are set. ' +
    'Each part opens the platform\'s own picker. The value is in the user\'s own time; the app converts it.',
  keywords: ['date time', 'datetime', 'date and time', 'schedule', 'reminder', 'appointment'],
  propsType: 'DateTimeInputProps',
  recipe: { map: 'dateTimeInputRecipe', parts: 'dateTimeInputParts' },
  anatomy: [
    { part: 'group', required: true, description: 'The two parts in a row that wraps: a group named by the label.' },
    { part: 'date', required: true, description: 'A DateInput named by `dateLabel`; it takes the width left.' },
    { part: 'time', required: true, description: 'A TimeInput named by `timeLabel`.' },
  ],
  accessibility: {
    pattern: null,
    name: 'label',
    description: ['description', 'status'],
    states: ['disabled'],
    keyboard: [
      { keys: 'Tab', effect: 'To the date, then to the time' },
      { keys: 'Enter or Space', effect: 'Opens the part\'s picker' },
    ],
    target: {},
    contrast: [],
    notes: [
      'A group named by the field\'s label and described by its help and status; each part has its own name.',
      'An error marks both parts invalid and is said once, under the pair.',
    ],
  },
  guidance: {
    do: [
      'Name the parts in full: "Start date", "Start time".',
      'Use it when the day and the time are one answer: a reminder, a meeting, a deadline.',
    ],
    dont: [
      'Use it when only the day matters; that is a DateInput.',
      'Show a time zone inside it; say which zone in the help text.',
    ],
  },
  examples: [
    { name: 'Reminder', title: 'A reminder' },
    { name: 'Required', title: 'Required, with an error' },
    { name: 'Narrow', title: 'In a narrow place' },
  ],
  gaps: ['The earliest and latest are days: a time cannot be limited on one particular day.'],
  related: ['DateInput', 'TimeInput', 'DateRangeInput', 'Timestamp'],
  gallery: {
    specimen: 'Specimen',
    tagline: 'A day and a time',
    order: 12,
    page: {
      lead: 'Use a date-time input when a day and a time are one answer.',
      hero: 'Reminder',
      variants: [
        { name: 'Required', title: 'Empty and required, with an error' },
        { name: 'StateDisabled', title: 'Disabled, with the reason' },
        { name: 'Narrow', title: 'In a narrow place the time goes under the date' },
      ],
    },
  },
};

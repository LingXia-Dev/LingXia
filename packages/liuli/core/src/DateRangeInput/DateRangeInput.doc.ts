import type { ComponentDoc } from '../doc';

export const doc: ComponentDoc = {
  name: 'DateRangeInput',
  category: 'inputs',
  status: 'preview',
  since: '2026-11-15',
  summary: 'A span of days in a form: a field that opens a calendar',
  description:
    'DateRangeInput asks for a first and a last day: a stay, time off, a report\'s period. The field shows the span as the app formats it; pressing it opens a Calendar, ' +
    'in a panel under the field for a mouse and in a sheet from the bottom edge for a finger. The first press sets the start, the second the end, and the calendar closes. ' +
    'It takes Field\'s label, help text and status. For one day, use DateInput.',
  keywords: ['date range', 'date range picker', 'period', 'stay', 'from to', 'range input', 'calendar field'],
  propsType: 'DateRangeInputProps',
  recipe: { map: 'dateRangeInputRecipe', parts: 'dateRangeInputParts' },
  anatomy: [
    { part: 'value', required: true, description: 'The span in the field, "18 Nov – 24 Nov", or the placeholder; in tabular figures.' },
    { part: 'popover', required: false, description: 'With a mouse or trackpad: a panel under the field holding the calendar.' },
  ],
  accessibility: {
    pattern: 'button',
    name: 'label',
    description: ['the chosen span', 'description', 'status message'],
    states: [{ state: 'expanded', from: 'the open sheet' }, { state: 'disabled', from: 'isDisabled' }, { state: 'modal', from: 'the sheet on touch' }],
    keyboard: [
      { keys: 'Enter or Space', effect: 'On the field: opens the calendar' },
      { keys: 'Tab', effect: 'Through the month buttons and the days' },
      { keys: 'Escape', effect: 'Closes the calendar and keeps what was chosen' },
    ],
    target: { sm: 32, md: 40, lg: 48 },
    contrast: [
      { what: 'span in the field', pair: ['text', 'surface'], minimum: '4.5:1' },
      { what: 'placeholder', pair: ['text-muted', 'surface'], minimum: '4.5:1' },
      { what: 'field edge', pair: ['line-strong', 'surface'], minimum: '3:1' },
      { what: 'day in the panel', pair: ['text', 'surface-overlay'], minimum: '4.5:1' },
      { what: 'error edge', pair: ['danger', 'surface'], minimum: '3:1' },
    ],
    notes: [
      'The field is a button named by `label`; the span, then the help text and status, are read as its description.',
      'The calendar is Calendar: each day is named in full and the span\'s days report selected.',
      'On touch the sheet is modal and titled by `label`; a tap outside, a drag down or Back closes it. With a mouse a click outside or Escape closes the panel.',
      'Closing with only a start chosen keeps that start; the field shows it with an open end.',
    ],
  },
  guidance: {
    do: [
      'Format days as the rest of the app does, short enough that a span fits the field: "18 Nov", not "Wednesday 18 November 2026".',
      'Rule out days that cannot be chosen with `min`, `max` and `isDateDisabled`, and say why in the help text.',
      'Check the finished span in the app (a minimum stay, a taken night) and say what is wrong in `status`.',
    ],
    dont: [
      'Use it for one day; that is DateInput.',
      'Use it for spans of months or years; two Selects or a DateInput each end reach further.',
    ],
  },
  examples: [
    { name: 'Stay', title: 'A span of days' },
    { name: 'Empty', title: 'Nothing chosen yet' },
  ],
  gaps: [
    { missing: 'Typing the dates into the field.', needs: 'nothing from the View', today: 'The days are pressed in the calendar.' },
    { missing: 'Ready-made spans beside the calendar: this week, last month.', needs: 'nothing from the View', today: 'An app puts its own buttons beside the field.' },
    { missing: 'Arrow keys through the days.', needs: 'a grid whose cells take one Tab stop and move by arrows in two directions', today: 'Every day is its own Tab stop, as in Calendar.' },
  ],
  playground: { props: { label: 'Stay', today: '2026-11-15', closeLabel: 'Close' }, fixture: true },
  related: ['Calendar', 'DateInput', 'Combobox'],
  gallery: {
    specimen: 'Specimen',
    tagline: 'From a day to a day',
    order: 10,
    page: {
      lead: 'Use a date range input when a form needs a first and a last day.',
      hero: 'Stay',
      variants: [{ name: 'Empty', title: 'Nothing chosen yet, with a placeholder' }],
      statesLayout: 'stack',
      states: [
        { name: 'StateInvalid', title: 'Invalid' },
        { name: 'StateDisabled', title: 'Disabled' },
      ],
    },
  },
};

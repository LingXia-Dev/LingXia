import type { ComponentDoc } from '../doc';

export const doc: ComponentDoc = {
  name: 'Calendar',
  category: 'inputs',
  status: 'preview',
  since: '2026-11-15',
  summary: 'A month of days in the page, to choose one day or a span',
  description:
    'Calendar draws a month in the page and lets people press a day, or two days for a span: a due date beside its task, the nights of a stay. ' +
    'Days are `YYYY-MM-DD` text. The chosen day is a filled circle, a span is a soft band between its ends, and today is ringed. ' +
    'Its words follow the page\'s language and its weeks start on the user\'s first weekday. For a date typed into a form, DateInput opens the platform\'s own picker.',
  keywords: ['calendar', 'date picker', 'date range', 'month', 'day picker', 'range picker', 'schedule'],
  propsType: 'CalendarProps',
  otherPropsTypes: ['CalendarRange'],
  recipe: { map: 'calendarRecipe', parts: 'calendarParts' },
  anatomy: [
    { part: 'root', required: true, description: 'The calendar, a group named by `label`.' },
    { part: 'header', required: true, description: 'The month\'s title and the buttons to the previous and next month; while choosing a month, the year\'s.' },
    { part: 'titleButton', required: true, description: 'The title as a button: it opens a year of months to jump to, and reports expanded while open.' },
    { part: 'title', required: true, description: 'The month and year, announced when the month changes; the year while choosing a month.' },
    { part: 'titleChevron', required: true, description: 'The chevron after the title, down for the days and up for the months.' },
    { part: 'spacer', required: true, description: 'Holds the month buttons at the header\'s end.' },
    { part: 'grid', required: true, description: 'The weeks, a grid named by the month.' },
    { part: 'weekday', required: true, description: 'The short weekday names over the columns; hidden from assistive tech, since each day is named in full.' },
    { part: 'slot', required: true, description: 'A day\'s seventh of the row; it carries the band of a span.' },
    { part: 'day', required: true, description: 'A day: a 40 px button, named in full, selected when chosen or inside the span.' },
    { part: 'monthGrid', required: false, description: 'While choosing a month: the year\'s twelve months, a grid named by the year.' },
    { part: 'monthRow', required: false, description: 'Three months of the year.' },
    { part: 'month', required: false, description: 'A month: a 44 px button named by month and year, selected for the month shown, ringed for this month, disabled when the range leaves it out.' },
    { part: 'monthLabel', required: false, description: 'The month\'s name.' },
  ],
  accessibility: {
    pattern: null,
    name: 'label',
    description: [],
    states: [{ state: 'selected', from: 'value or bind, a day or a span' }, { state: 'disabled', from: 'min, max, isDateDisabled' }],
    keyboard: [
      { keys: 'Tab', effect: 'To the title, the month buttons, then into the days as one stop' },
      { keys: 'Enter or Space on the title', effect: 'Opens the year\'s months; a month shows its days again' },
      { keys: 'Arrow keys', effect: 'A day along the week, a week up and down; days that cannot be chosen are passed over' },
      { keys: 'Home and End', effect: 'To the ends of the week' },
      { keys: 'PageUp and PageDown', effect: 'To the month before and after' },
      { keys: 'Enter or Space', effect: 'Chooses the day' },
    ],
    target: { md: 40 },
    contrast: [
      { what: 'day', pair: ['text', 'surface'], minimum: '4.5:1' },
      { what: 'chosen day', pair: ['on-accent-solid', 'accent-solid'], minimum: '4.5:1' },
      { what: 'day inside a span', pair: ['text', 'accent-soft'], minimum: '4.5:1' },
      { what: 'weekday names', pair: ['text-muted', 'surface'], minimum: '4.5:1' },
      { what: 'ring on today', pair: ['line-strong', 'surface'], minimum: '3:1' },
    ],
    notes: [
      'Each day is named in full, "Thursday 19 November 2026", with "Today" said where it is today, so the weekday row is not read.',
      'A chosen day, both ends of a span and the days between them report selected.',
      'The month\'s title is a status: moving to another month announces it.',
      'A day that cannot be chosen stays in the grid, dimmed and disabled, so the weeks keep their shape.',
      'Today is marked by a ring and named in words; the chosen day by a fill. Neither depends on colour alone.',
    ],
  },
  guidance: {
    do: [
      'Use it where the days themselves matter: seeing a span, weekdays against weekends, what is taken.',
      'Pass `today` from the page\'s Logic, which knows the time and the person\'s time zone; a View has no clock.',
      'Show the chosen span in words beside the calendar, as the app formats dates.',
      'Set `firstWeekday` only when the week is the app\'s own, such as a school timetable that runs Monday to Sunday everywhere.',
      'At large text, a day\'s circle widens to hold two digits, up to its seventh of the row; give the calendar the full width.',
    ],
    dont: [
      'Use it for a birth date or any date far from today; DateInput\'s picker reaches a year faster.',
      'Hide days that cannot be chosen; disable them.',
      'Set `firstWeekday` for a personal date such as a due date or a stay; the user\'s own first weekday is right there.',
    ],
  },
  examples: [
    { name: 'Day', title: 'One day' },
    { name: 'Stay', title: 'A span of days' },
    { name: 'Weekdays', title: 'Days ruled out' },
    { name: 'MondayFirst', title: 'The app\'s own week' },
  ],
  gaps: [
    { missing: 'A field that opens the calendar, for a span typed into a form.', needs: 'nothing from the View', today: 'The calendar sits in the page; put it in a Popover or a BottomSheet.' },
    { missing: 'Jumping to a month or year, and two months side by side.', needs: 'nothing from the View', today: 'One month, a month at a time.' },
    { missing: 'Month and weekday names in a language Liuli does not translate.', needs: 'date formatting in the View\'s engine', today: 'English and Chinese; an lxapp adds a language by naming `@liuli.monthName` and `@liuli.weekdayName` in its catalogue.' },
  ],
  playground: { props: { value: '2026-11-19', label: 'Due date', today: '2026-11-15' } },
  related: ['DateInput', 'TimeInput', 'Popover'],
  gallery: {
    specimen: 'Specimen',
    tagline: 'A month of days',
    order: 9,
    page: {
      lead: 'Use a calendar where people choose a day, or a span of days, by looking at a month.',
      hero: 'Day',
      variants: [
        { name: 'Stay', title: 'A span: the first press starts it, the second ends it; past days cannot be chosen' },
        { name: 'Weekdays', title: 'Days the app rules out are dimmed' },
        { name: 'MondayFirst', title: 'A timetable\'s week, Monday first for every user' },
      ],
    },
  },
};

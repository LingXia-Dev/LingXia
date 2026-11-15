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
    'Its words come from a `locale` object; English and Chinese ones are ready to import. For a date typed into a form, DateInput opens the platform\'s own picker.',
  keywords: ['calendar', 'date picker', 'date range', 'month', 'day picker', 'range picker', 'schedule'],
  propsType: 'CalendarProps',
  otherPropsTypes: ['CalendarLocale', 'CalendarRange'],
  recipe: { map: 'calendarRecipe', parts: 'calendarParts' },
  anatomy: [
    { part: 'root', required: true, description: 'The calendar, a group named by `label`.' },
    { part: 'header', required: true, description: 'The month\'s title and the buttons to the previous and next month.' },
    { part: 'title', required: true, description: 'The month and year, announced when the month changes.' },
    { part: 'grid', required: true, description: 'The weeks, a grid named by the month.' },
    { part: 'weekday', required: true, description: 'The short weekday names over the columns; hidden from assistive tech, since each day is named in full.' },
    { part: 'slot', required: true, description: 'A day\'s seventh of the row; it carries the band of a span.' },
    { part: 'day', required: true, description: 'A day: a 40 px button, named in full, selected when chosen or inside the span.' },
  ],
  accessibility: {
    pattern: null,
    name: 'label',
    description: [],
    states: [{ state: 'selected', from: 'value, or range' }, { state: 'disabled', from: 'min, max, isDateDisabled' }],
    keyboard: [
      { keys: 'Tab', effect: 'To the month buttons, then through the days' },
      { keys: 'Enter or Space', effect: 'Chooses the day' },
    ],
    target: { md: 40 },
    contrast: [
      { what: 'day', pair: ['text', 'surface'], minimum: '4.5:1' },
      { what: 'chosen day', pair: ['on-accent', 'accent'], minimum: '4.5:1' },
      { what: 'day inside a span', pair: ['text', 'accent-soft'], minimum: '4.5:1' },
      { what: 'weekday names', pair: ['text-muted', 'surface'], minimum: '4.5:1' },
      { what: 'ring on today', pair: ['line-strong', 'surface'], minimum: '3:1' },
    ],
    notes: [
      'Each day is named in full by the locale, "Thursday 19 November 2026", with "Today" said where it is today, so the weekday row is not read.',
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
      'Spread a ready-made locale to change the first day of the week or a word.',
    ],
    dont: [
      'Use it for a birth date or any date far from today; DateInput\'s picker reaches a year faster.',
      'Hide days that cannot be chosen; disable them.',
    ],
  },
  examples: [
    { name: 'Day', title: 'One day' },
    { name: 'Stay', title: 'A span of days' },
    { name: 'Weekdays', title: 'Days ruled out' },
    { name: 'Chinese', title: 'Another language' },
  ],
  gaps: [
    {
      missing: 'Arrow keys moving through the days as one Tab stop, a week up and down.',
      needs: 'a grid whose cells take one Tab stop and move by arrows in two directions',
      today: 'Every day is its own Tab stop.',
    },
    { missing: 'A field that opens the calendar, for a span typed into a form.', needs: 'nothing from the View', today: 'The calendar sits in the page; put it in a Popover or a BottomSheet.' },
    { missing: 'Jumping to a month or year, and two months side by side.', needs: 'nothing from the View', today: 'One month, a month at a time.' },
    { missing: 'Month and weekday names from the device\'s own language.', needs: 'date formatting in the View\'s engine', today: 'The app passes a `locale`.' },
  ],
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
        { name: 'Chinese', title: 'Words and week start from the locale' },
      ],
    },
  },
};

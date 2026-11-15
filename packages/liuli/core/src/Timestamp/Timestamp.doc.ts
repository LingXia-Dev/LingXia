import type { ComponentDoc } from '../doc';

export const doc: ComponentDoc = {
  name: 'Timestamp',
  category: 'content',
  status: 'preview',
  since: '2026-11-15',
  summary: 'When something happened, in the words people use',
  description:
    'Timestamp writes a moment as people say it: "12 minutes ago" for today, "Yesterday, 18:30", then the date, with the year only when it is another year. ' +
    'It takes the moment and the present as local date-times from the app, and its words from a locale: Liuli ships English and Chinese, and an app can pass its own. ' +
    'The time follows the user\'s 12- or 24-hour clock.',
  keywords: ['timestamp', 'time', 'date', 'relative time', 'ago', 'when'],
  propsType: 'TimestampProps',
  recipe: { map: 'timestampRecipe', parts: 'timestampParts' },
  anatomy: [{ part: 'root', required: true, description: 'The text; its size and colour come from `className`.' }],
  accessibility: {
    pattern: null,
    name: null,
    description: [],
    states: [],
    keyboard: [],
    target: {},
    contrast: [{ what: 'in the muted colour', pair: ['text-muted', 'surface'], minimum: '4.5:1' }],
    notes: [
      'It is read in full, as the date and the time, whatever short form is shown.',
      'It does not update itself and is not a live region; the app passes a new `now` when it wants the words to move on.',
    ],
  },
  guidance: {
    do: [
      'Use `relative` in lists of recent things; use `date-time` where the exact moment matters, such as a receipt.',
      'Refresh `now` when the screen comes back into view, and about once a minute while it is shown.',
    ],
    dont: [
      'Use it for a length of time; that is Timer.',
      'Pass a time in another zone; convert it to the user\'s own time first.',
    ],
  },
  examples: [
    { name: 'Activity', title: 'An activity list' },
    { name: 'Formats', title: 'The four forms' },
    { name: 'Chinese', title: 'In Chinese' },
  ],
  gaps: [
    'There is no clock and no time zone in the View: the app passes `now` and local date-times.',
    'Words come from a locale module, not from the system; two locales ship.',
  ],
  related: ['Timer', 'Calendar', 'DateInput'],
  gallery: {
    specimen: 'Specimen',
    tagline: 'When it happened',
    order: 15,
    page: {
      lead: 'Use a timestamp to say when something happened.',
      hero: 'Activity',
      variants: [
        { name: 'Formats', title: 'Relative, date, time, and date with time' },
        { name: 'Chinese', title: 'From the Chinese locale module' },
      ],
    },
  },
};

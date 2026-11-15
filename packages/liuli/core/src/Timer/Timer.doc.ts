import type { ComponentDoc } from '../doc';

export const doc: ComponentDoc = {
  name: 'Timer',
  category: 'content',
  status: 'preview',
  since: '2026-11-15',
  summary: 'A length of time in steady digits',
  description:
    'Timer draws a number of seconds as `m:ss`, or `h:mm:ss` from an hour on, in digits that keep their width as they change. ' +
    'It does not count: the app passes `seconds` each time it changes, from Logic or from its own tick.',
  keywords: ['timer', 'countdown', 'stopwatch', 'duration', 'elapsed', 'clock'],
  propsType: 'TimerProps',
  recipe: { map: 'timerRecipe', parts: 'timerParts' },
  anatomy: [{ part: 'root', required: true, description: 'The digits; muted once the time is zero.' }],
  accessibility: {
    pattern: null,
    name: 'aria-label',
    description: [],
    states: [],
    keyboard: [],
    target: {},
    contrast: [
      { what: 'digits', pair: ['text', 'surface'], minimum: '4.5:1' },
      { what: 'digits at zero', pair: ['text-muted', 'surface'], minimum: '4.5:1' },
    ],
    notes: [
      'Give `aria-label` in words: "2 minutes 5 seconds left". Digits alone are read as a time of day by some screen readers.',
      'It is not a live region: a countdown read every second is noise. Announce what matters, such as the last ten seconds, with a status of your own.',
    ],
  },
  guidance: {
    do: ['Say beside it what the time is: "Code expires in", "Recording".', 'Use `hasHours` where the time may pass an hour, so the digits do not jump wider.'],
    dont: ['Use it for a time of day or a date; that is Timestamp.', 'Hide the only way to tell time is running out in the digits; change the screen when it is up.'],
  },
  examples: [
    { name: 'Countdown', title: 'A countdown' },
    { name: 'Inline', title: 'In a line of text' },
  ],
  gaps: [],
  limits: [
    'Timer has no clock of its own; the app passes the seconds.',
  ],
  related: ['Timestamp', 'ProgressBar'],
  playground: { props: { seconds: 125, size: 'lg', hasHours: false } },
  gallery: {
    specimen: 'Specimen',
    tagline: 'Minutes and seconds',
    order: 13,
    page: {
      lead: 'Use a timer to show a length of time that changes: a countdown, a recording.',
      hero: 'Countdown',
      variants: [{ name: 'Inline', title: 'Small and medium, in a line of text; hours appear from an hour on' }],
    },
  },
};

import type { ComponentDoc } from '../doc';

export const doc: ComponentDoc = {
  name: 'CircularProgress',
  category: 'feedback',
  status: 'preview',
  since: '2026-11-15',
  summary: 'How much is done, as a ring',
  description:
    'CircularProgress shows an amount from 0 to `max` as an arc around a ring, with the app\'s own words in the middle at the larger sizes. ' +
    'Without `value` the arc spins: the task is running and its length is unknown. Use it where a ProgressBar has no room: a tile, the end of a row.',
  keywords: ['progress', 'ring', 'circular', 'radial', 'meter', 'gauge', 'donut'],
  propsType: 'CircularProgressProps',
  recipe: { map: 'circularProgressRecipe', parts: 'circularProgressParts' },
  anatomy: [
    { part: 'root', required: true, description: 'The square the ring fills; it carries the progressbar role.' },
    { part: 'track', required: true, description: 'The whole ring, in faint ink.' },
    { part: 'fill', required: false, description: 'The arc from twelve o\'clock, clockwise; a quarter arc that spins while the amount is unknown.' },
    { part: 'value', required: false, description: '`valueLabel` in the middle, at `md` and `lg`.' },
  ],
  accessibility: {
    pattern: null,
    name: 'label',
    description: [],
    states: [],
    keyboard: [],
    target: {},
    contrast: [
      { what: 'arc on a card', pair: ['accent', 'surface'], minimum: '3:1' },
      { what: 'arc against the track', pair: ['accent', 'text/10'], over: 'surface', minimum: '3:1' },
      { what: 'success arc', pair: ['success-mark', 'surface'], minimum: '3:1' },
      { what: 'warning arc', pair: ['warning-mark', 'surface'], minimum: '3:1' },
      { what: 'danger arc', pair: ['danger-mark', 'surface'], minimum: '3:1' },
      { what: 'value', pair: ['text', 'surface'], minimum: '4.5:1' },
    ],
    notes: [
      'Role progressbar, named by `label`. With `value` it reports the value, 0 and `max`; `valueLabel` is read in place of a percentage.',
      'Without `value` it reports no value, which assistive tech reads as busy.',
      'The words in the middle are not read twice: the ring already carries them.',
      'Under reduced motion the unknown arc stands still.',
    ],
  },
  guidance: {
    do: [
      'Give `valueLabel` in the words the screen uses: "72%", "3 of 8".',
      'Use `sm` at the end of a row or inside a button; say the amount beside it if it matters.',
    ],
    dont: [
      'Use a ring where a bar fits; a bar is easier to compare and to read.',
      'Rely on the tone alone; say "nearly full" or "over" in words.',
    ],
  },
  examples: [
    { name: 'Goal', title: 'A goal on a tile' },
    { name: 'InRows', title: 'At the end of a row' },
    { name: 'Sizes', title: 'Sizes' },
    { name: 'Tones', title: 'Meters in a tone' },
  ],
  gaps: ['The arc jumps to a new value; it does not sweep there.'],
  related: ['ProgressBar', 'Stat'],
  gallery: {
    specimen: 'Specimen',
    tagline: 'Progress as a ring',
    order: 7,
    page: {
      lead: 'Use a ring for progress where a bar has no room.',
      hero: 'Goal',
      variants: [{ name: 'Tones', title: 'A tone for a meter' }],
      sizes: [{ name: 'Sizes', title: 'Small (24), medium (48) and large (80)' }],
      inContext: [{ name: 'InRows', title: 'Uploads, each with its ring' }],
    },
  },
};

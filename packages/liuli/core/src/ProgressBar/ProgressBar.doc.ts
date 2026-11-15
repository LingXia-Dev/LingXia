import type { ComponentDoc } from '../doc';

export const doc: ComponentDoc = {
  name: 'ProgressBar',
  category: 'feedback',
  status: 'preview',
  since: '2026-11-12',
  summary: 'How much of a task is done, or a spinner while the amount is unknown',
  description:
    'ProgressBar shows a task that runs for a while: an upload, an export, storage used. Without `value` it sweeps to say ' +
    'the task is running. Spinner, from the same import, is the small form for a wait inside a control or a list. ' +
    'Both are drawn from plain views and vectors, so they look the same on every OS.',
  keywords: ['progress', 'progressbar', 'loading', 'spinner', 'busy', 'upload', 'download', 'meter', 'indeterminate'],
  propsType: 'ProgressBarProps',
  otherPropsTypes: ['SpinnerProps'],
  recipe: { map: 'progressBarRecipe', parts: 'progressBarParts' },
  anatomy: [
    { part: 'header', required: false, description: 'The label row: `label`, then `valueLabel`; hidden from assistive tech, since the bar carries both.' },
    { part: 'track', required: true, description: 'The bar itself, and the element assistive tech reports as a progress bar.' },
    { part: 'fill', required: true, description: 'The amount done, as a share of the track width; a sweeping segment while the amount is unknown.' },
    { part: 'spinner', required: false, description: "Spinner's spinning arc." },
  ],
  accessibility: {
    pattern: null,
    name: 'label',
    description: [],
    states: [],
    keyboard: [],
    contrast: [
      { what: 'fill against the track', pair: ['accent', 'text/10'], over: 'surface', minimum: '3:1' },
      { what: 'fill and spinner on a card', pair: ['accent', 'surface'], minimum: '3:1' },
      { what: 'success fill', pair: ['success', 'surface'], minimum: '3:1' },
      { what: 'warning fill', pair: ['warning', 'surface'], minimum: '3:1' },
      { what: 'danger fill', pair: ['danger', 'surface'], minimum: '3:1' },
      { what: 'label', pair: ['text', 'surface'], minimum: '4.5:1' },
      { what: 'value', pair: ['text-muted', 'surface'], minimum: '4.5:1' },
    ],
    notes: [
      'Role progressbar, named by `label`. With `value` it reports the value, 0 and `max`; `valueLabel` is read in place of a percentage.',
      'Without `value` it reports no value, which assistive tech reads as busy.',
      'With reduced motion the sweep and the spin stop; the bar and the arc stay drawn.',
      'Spinner is a progressbar named by its `label`, with nothing drawn but the arc.',
    ],
  },
  guidance: {
    do: [
      'Name what is in progress in `label`: "Uploading photos", not "Progress".',
      'Pass `valueLabel` in the app\'s own words and units; Liuli formats no numbers.',
      'Use Spinner for waits under a few seconds, and ProgressBar when the amount is known or the wait is long.',
    ],
    dont: [
      'Show a ProgressBar and a Spinner for the same task.',
      'Use ProgressBar as a gauge the user sets; that is a slider.',
      'Put a Spinner inside a Button; Button has its own loading state.',
    ],
  },
  examples: [
    { name: 'Determinate', title: 'An amount the app words' },
    { name: 'Indeterminate', title: 'Running, amount unknown' },
    { name: 'Compact', title: 'Thin, under its own heading' },
    { name: 'Spinners', title: 'Spinner sizes' },
    { name: 'Tones', title: 'Meters in a tone' },
  ],
  gaps: [],
  related: ['Button'],
  gallery: {
    specimen: 'Specimen',
    tagline: 'Progress, or a wait',
    order: 2,
    page: {
      lead: 'Use a progress bar for a task that takes a while, and a spinner for a short wait.',
      hero: 'Determinate',
      variants: [
        { name: 'Indeterminate', title: 'Amount unknown' },
        { name: 'Spinners', title: 'Spinner, for a short wait' },
        { name: 'Tones', title: 'A tone for a meter: reached, nearly full, over' },
      ],
      sizes: [{ name: 'SizesBar', title: 'Small and medium tracks' }],
      states: [
        { name: 'StateStarted', title: 'Started' },
        { name: 'StateHalf', title: 'Halfway' },
        { name: 'StateDone', title: 'Done' },
        { name: 'StateUnknown', title: 'Unknown' },
      ],
      inContext: [{ name: 'Compact', title: 'Thin, under its own heading' }],
    },
  },
};

import type { ComponentDoc } from '@liulidesign/core/doc';

export const doc: ComponentDoc = {
  name: 'Stat',
  category: 'charts',
  status: 'preview',
  since: '2026-11-14',
  summary: 'One figure with its label, how it changed, and an optional small chart',
  description:
    'Stat shows a number a person checks often: revenue, open orders, a response time. ' +
    'Under the figure it can say how the number changed, with an arrow for the direction and a tone for whether that is good news, ' +
    'and it can hold a Sparkline of its history. The app formats every string; Stat lays them out and reads them as one group.',
  keywords: ['stat', 'statistic', 'metric', 'kpi', 'figure', 'number', 'delta', 'change', 'trend', 'dashboard', 'tile'],
  propsType: 'StatProps',
  recipe: { map: 'statRecipe', parts: 'statParts' },
  anatomy: [
    { part: 'root', required: true, description: 'The group, named by `label`.' },
    { part: 'label', required: true, description: 'What the figure measures; hidden from assistive tech, since the group carries it as its name.' },
    { part: 'value', required: true, description: 'The figure, in tabular figures so it does not shift as it updates.' },
    { part: 'change', required: false, description: 'The line under the figure: the change, then the note.' },
    { part: 'delta', required: false, description: 'The arrow and `delta`, in the tone\'s colour.' },
    { part: 'deltaSpoken', required: false, description: '`deltaLabel`, read in place of the arrow and `delta`; not drawn.' },
    { part: 'description', required: false, description: 'A short note after the change.' },
    { part: 'chart', required: false, description: 'The slot under the figure, as wide as the Stat.' },
    { part: 'skeleton', required: false, description: 'Stands in for the figure while loading.' },
  ],
  accessibility: {
    pattern: null,
    name: 'label',
    description: [],
    states: [{ state: 'busy', from: 'isLoading' }],
    keyboard: [],
    contrast: [
      { what: 'figure', pair: ['text', 'surface'], minimum: '4.5:1' },
      { what: 'label and note', pair: ['text-muted', 'surface'], minimum: '4.5:1' },
      { what: 'positive change', pair: ['success', 'surface'], minimum: '4.5:1' },
      { what: 'negative change', pair: ['danger', 'surface'], minimum: '4.5:1' },
      { what: 'positive change, straight on the page', pair: ['success', 'canvas'], minimum: '4.5:1' },
      { what: 'negative change, straight on the page', pair: ['danger', 'canvas'], minimum: '4.5:1' },
      { what: 'positive change in a sunken card', pair: ['success', 'surface-sunken'], minimum: '4.5:1' },
      { what: 'negative change in a sunken card', pair: ['danger', 'surface-sunken'], minimum: '4.5:1' },
      { what: 'label and note in a sunken card', pair: ['text-muted', 'surface-sunken'], minimum: '4.5:1' },
    ],
    notes: [
      'A group named by `label`, read in order: the figure, the change, the note, then the chart by its own label.',
      'The arrow is hidden. With `deltaLabel` the drawn change is hidden too and the words are read: "Up 12.4% from last month". Without it only `delta` is read, which loses the direction.',
      'Direction is drawn as an arrow and tone as colour, so the change never depends on colour alone.',
      'While loading the group is reported busy and keeps its name; the skeleton is hidden.',
      'It takes no focus. To make it open a page, put it in a ClickableCard.',
    ],
  },
  guidance: {
    do: [
      'Format `value` and `delta` in the app, in the person\'s locale and units.',
      'Pass `deltaLabel` whenever there is a `delta`, saying the direction and what it is compared with.',
      'Set `deltaTone` when up is bad news: errors, cost, waiting time.',
      'Keep one period across the figures of a dashboard, and say it once in `description` or a heading.',
    ],
    dont: [
      'Put a unit or a sign in `label`; it names the measure.',
      'Show more than one figure in a Stat; use one Stat each.',
      'Use the `sm` figure alone on a page; it is for cards two to a row.',
      'Leave a figure blank while it loads; set `isLoading`.',
    ],
  },
  examples: [
    { name: 'Revenue', title: 'A figure, its change and its history' },
    { name: 'Plain', title: 'A figure alone' },
    { name: 'Tones', title: 'Tone set apart from direction' },
    { name: 'Sizes', title: 'Small and medium' },
    { name: 'Dashboard', title: 'Four figures, two to a row' },
  ],
  gaps: [
    {
      missing: 'A figure that counts to its new value.',
      needs: 'a text content transition',
      today: 'The new figure replaces the old one at once.',
    },
    {
      missing: 'A figure larger than display type.',
      needs: 'a larger step in Liuli\'s type scale',
      today: '`md` is display type, 28 px.',
    },
  ],
  related: ['Sparkline'],
  gallery: {
    specimen: 'Specimen',
    tagline: 'A number to watch',
    order: 1,
    page: {
      lead: 'Use a stat for a number people come back to check, with how it changed since last time.',
      hero: 'Revenue',
      variants: [
        { name: 'Plain', title: 'A figure alone' },
        { name: 'Tones', title: 'Down can be good news and up bad: the tone is set apart from the direction' },
      ],
      sizes: [{ name: 'Sizes', title: 'Small for cards two to a row, medium by default' }],
      states: [
        { name: 'StateUp', title: 'Up' },
        { name: 'StateDown', title: 'Down' },
        { name: 'StateFlat', title: 'No change' },
        { name: 'StateLoading', title: 'While loading' },
      ],
      inContext: [{ name: 'Dashboard', title: 'Four figures on a phone, two to a row' }],
    },
  },
};

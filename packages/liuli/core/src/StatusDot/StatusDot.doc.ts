import type { ComponentDoc } from '../doc';

export const doc: ComponentDoc = {
  name: 'StatusDot',
  category: 'feedback',
  status: 'preview',
  since: '2026-11-15',
  summary: 'How something stands, as a dot and a word',
  description:
    'StatusDot is a small dot in a status mark colour with the status in words beside it: a service that is running, a record that needs review. ' +
    'The word carries the meaning; the dot lets the eye find it down a column. Neutral is a hollow ring. For a status that should stand out in a line of text, use a Badge.',
  keywords: ['status', 'dot', 'indicator', 'presence', 'state', 'health'],
  propsType: 'StatusDotProps',
  recipe: { map: 'statusDotRecipe', parts: 'statusDotParts' },
  anatomy: [
    { part: 'root', required: true, description: 'The dot and its word in a row.' },
    { part: 'dot', required: true, description: 'A filled dot in the tone\'s mark colour; a hollow ring for neutral.' },
    { part: 'label', required: false, description: 'The status in words.' },
  ],
  accessibility: {
    pattern: null,
    name: 'label',
    description: [],
    states: [],
    keyboard: [],
    target: {},
    contrast: [
      { what: 'success dot', pair: ['success-mark', 'surface'], minimum: '3:1' },
      { what: 'warning dot', pair: ['warning-mark', 'surface'], minimum: '3:1' },
      { what: 'danger dot', pair: ['danger-mark', 'surface'], minimum: '3:1' },
      { what: 'info dot', pair: ['info-mark', 'surface'], minimum: '3:1' },
      { what: 'accent dot', pair: ['accent', 'surface'], minimum: '3:1' },
      { what: 'hue 1 dot', pair: ['hue-1', 'surface'], minimum: '3:1' },
      { what: 'hue 2 dot', pair: ['hue-2', 'surface'], minimum: '3:1' },
      { what: 'hue 3 dot', pair: ['hue-3', 'surface'], minimum: '3:1' },
      { what: 'hue 4 dot', pair: ['hue-4', 'surface'], minimum: '3:1' },
      { what: 'hue 5 dot', pair: ['hue-5', 'surface'], minimum: '3:1' },
      { what: 'hue 6 dot', pair: ['hue-6', 'surface'], minimum: '3:1' },
      { what: 'hue 7 dot', pair: ['hue-7', 'surface'], minimum: '3:1' },
      { what: 'hue 8 dot', pair: ['hue-8', 'surface'], minimum: '3:1' },
      { what: 'neutral ring', pair: ['line-strong', 'surface'], minimum: '3:1' },
      { what: 'label', pair: ['text', 'surface'], minimum: '4.5:1' },
    ],
    notes: [
      'The word is read; the dot is not. With `isLabelHidden` the dot is an image named by the word.',
      'Colour is never the only sign: the word says the status, and neutral differs by shape.',
      'It does not announce a change; say that with a Toast or a status of your own.',
    ],
  },
  guidance: {
    do: [
      'Write the status as one or two words: "Running", "Needs review".',
      'Keep one meaning per tone across the app.',
    ],
    dont: [
      'Hide the word unless the same word is already beside the dot.',
      'Use a dot for a count or an unread mark; that is a Badge.',
    ],
  },
  examples: [
    { name: 'Services', title: 'Down a column' },
    { name: 'Tones', title: 'The six tones' },
    { name: 'Hues', title: 'Categories in the eight hues' },
    { name: 'DotOnly', title: 'Without its words' },
  ],
  gaps: [],
  limits: [
    'It does not pulse or blink.',
  ],
  related: ['Badge', 'Avatar', 'Banner'],
  playground: { props: { label: 'Running', tone: 'success', size: 'md', isLabelHidden: false } },
  gallery: {
    specimen: 'Specimen',
    tagline: 'A dot and a word',
    order: 8,
    page: {
      lead: 'Use a status dot to say how something stands, in a row or a column of like things.',
      hero: 'Services',
      variants: [
        { name: 'Tones', title: 'Success, warning, danger, info and accent (one colour until a pack sets its own info), and neutral as a ring' },
        { name: 'Hues', title: 'A hue for a category: which calendar an event is on' },
        { name: 'DotOnly', title: 'The dot alone, named by its word' },
      ],
    },
  },
};

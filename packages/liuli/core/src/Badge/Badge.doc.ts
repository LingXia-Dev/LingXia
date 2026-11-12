import type { ComponentDoc } from '../doc';

export const doc: ComponentDoc = {
  name: 'Badge',
  category: 'feedback',
  status: 'preview',
  summary: 'A short status or category label in a tinted pill',
  description:
    'Badge marks the state or kind of the thing next to it: Draft, Paid, Overdue, New. It is text, not a control, and is ' +
    'read as it is shown. Five tones come soft; the four status tones also come solid for the one label that must stand out.',
  keywords: ['badge', 'tag', 'label', 'pill', 'chip', 'status', 'lozenge', 'tone'],
  propsType: 'BadgeProps',
  recipe: { map: 'badgeRecipe', parts: 'badgeParts' },
  anatomy: [
    { part: 'root', required: true, description: 'The pill: tint or fill by tone and emphasis.' },
    { part: 'icon', required: false, description: '`startIcon`, hidden from assistive tech.' },
    { part: 'label', required: true, description: 'The text; keep it to a word or two.' },
  ],
  accessibility: {
    pattern: null,
    name: 'label',
    description: [],
    states: [],
    keyboard: [],
    contrast: [
      { what: 'neutral', pair: ['text', 'surface-sunken'], minimum: '4.5:1' },
      { what: 'soft accent', pair: ['on-accent-soft', 'accent-soft'], minimum: '4.5:1' },
      { what: 'soft success', pair: ['text', 'success-soft'], minimum: '4.5:1' },
      { what: 'soft warning', pair: ['text', 'warning-soft'], minimum: '4.5:1' },
      { what: 'soft danger', pair: ['text', 'danger-soft'], minimum: '4.5:1' },
      { what: 'solid accent', pair: ['on-accent', 'accent'], minimum: '4.5:1' },
      { what: 'solid success', pair: ['on-success', 'success'], minimum: '4.5:1' },
      { what: 'solid warning', pair: ['on-warning', 'warning'], minimum: '4.5:1' },
      { what: 'solid danger', pair: ['on-danger', 'danger'], minimum: '4.5:1' },
    ],
    notes: [
      'Not a control and not announced on change. A badge whose change matters sits next to a status region the app keeps.',
      'The tone is colour only; the label carries the meaning in words.',
    ],
  },
  guidance: {
    do: [
      'Say the status in the label; the colour only repeats it.',
      'Keep one solid badge per view at most.',
      'In a pressable Item, repeat the status in the row description; the end content is not read there.',
    ],
    dont: [
      'Make a badge pressable; use a Button or a row.',
      'Use a badge for a count on an icon; that needs its own placement and name.',
      'Pick tones for decoration; each tone means the status it is named for.',
    ],
  },
  examples: [
    { name: 'Tones', title: 'Soft tones' },
    { name: 'Solid', title: 'Solid, to stand out' },
    { name: 'Sizes', title: 'Sizes and an icon' },
    { name: 'InARow', title: 'At the end of a row' },
  ],
  gaps: [
    {
      missing: 'An info tone and category hues (tags, labels, people).',
      needs: 'info and categorical hue roles with soft and on- pairs',
      today: 'Five tones; use neutral for categories.',
    },
  ],
  related: ['Item', 'Card'],
  gallery: {
    specimen: 'Specimen',
    tagline: 'A status label',
    order: 1,
    page: {
      lead: 'Use a badge to mark the status or kind of the thing beside it, in a word or two.',
      hero: 'InARow',
      variants: [
        { name: 'Tones', title: 'Soft, in five tones' },
        { name: 'Solid', title: 'Solid, for the one label that must stand out' },
      ],
      sizes: [{ name: 'Sizes', title: 'Small and medium, with an icon' }],
    },
  },
};

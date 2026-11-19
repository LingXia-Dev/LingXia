import type { ComponentDoc } from '../doc';

export const doc: ComponentDoc = {
  name: 'SegmentedControl',
  category: 'selection',
  status: 'preview',
  since: '2026-11-13',
  summary: 'One value from a few, as a row of segments with a thumb on the chosen one',
  description:
    'SegmentedControl sets one value from two to five short choices that fit a row: a unit, a theme, a sort order. It is a ' +
    'radio group drawn as equal segments, with a raised thumb that slides to the chosen one. A label is never cut: a long ' +
    'one widens its segment, and when the row cannot hold them the segments stack. It sets a value; to switch ' +
    'what a panel shows, use Tabs.',
  keywords: ['segmented control', 'segments', 'radio', 'choice', 'switcher', 'units', 'mode', 'picker'],
  propsType: 'SegmentedControlProps',
  recipe: { map: 'segmentedControlRecipe', parts: 'segmentedControlParts' },
  anatomy: [
    { part: 'root', required: true, description: 'The sunken track and the radio group, named by `label`.' },
    { part: 'track', required: true, description: 'Holds the thumb; mirrors in right-to-left so it slides toward the end.' },
    { part: 'thumb', required: true, description: 'The raised box, one segment wide, under the chosen segment.' },
    { part: 'segment', required: true, description: 'A radio per option, named by its label.' },
    { part: 'label', required: true, description: 'The option\'s text, whole; cut short only if a stacked segment cannot hold it.' },
  ],
  accessibility: {
    pattern: 'radioGroup',
    name: 'label',
    description: [],
    states: [
      { state: 'checked', from: 'value, or bind' },
      { state: 'disabled', from: 'options[].isDisabled' },
    ],
    keyboard: [
      { keys: 'Tab', effect: 'Into the row, on the chosen segment; again, out of it.' },
      { keys: 'Left, Right', effect: 'The previous or next segment, chosen on arrival.' },
    ],
    target: { sm: 28, md: 36 },
    contrast: [
      { what: 'chosen label', pair: ['text', 'surface-raised'], minimum: '4.5:1' },
      { what: 'other labels', pair: ['text-muted', 'surface-sunken'], minimum: '4.5:1' },
    ],
    notes: [
      'The chosen segment is in the raised thumb and the label\'s colour together, and is read as checked.',
      'Segments are 28 and 36 px tall, below the 44 pt touch minimum; keep it to pointer layouts and compact rows with room around them.',
      'The thumb\'s edge is decorative and has no contrast minimum: the chosen segment is carried by its label and the raised thumb.',
    ],
  },
  guidance: {
    do: [
      'Keep to two to five labels short enough to fit their share of the row.',
      'At large text, let it stack: a column of segments keeps every choice in view, where a scrolling row would hide some.',
      'Apply the choice at once, where people can see what it changed.',
    ],
    dont: [
      'Use it to switch between panels of content; that is Tabs.',
      'Use it for more than five choices or long labels; use a Select or a RadioGroup.',
      'Use it for an action; each segment is a value, not a command.',
    ],
  },
  examples: [
    { name: 'Units', title: 'A unit for the numbers below it' },
    { name: 'Appearance', title: 'A setting row' },
    { name: 'Sizes', title: 'Sizes' },
  ],
  gaps: [
    {
      missing: 'Icon-only segments.',
      needs: 'an icon set for Liuli',
      today: 'Segments carry a text label.',
    },
  ],
  related: ['Tabs', 'RadioGroup', 'ToggleButton'],
  gallery: {
    specimen: 'Specimen',
    tagline: 'One of a few',
    order: 5,
    page: {
      lead: 'Use a segmented control to set one value from a few short choices, applied at once.',
      hero: 'Units',
      sizes: [{ name: 'Sizes', title: 'Small and medium' }],
      states: [
        { name: 'StateTwo', title: 'Two choices' },
        { name: 'StateFive', title: 'Five choices' },
        { name: 'StateDisabled', title: 'A disabled segment' },
      ],
      inContext: [{ name: 'Appearance', title: 'At the end of a setting row' }],
    },
  },
};

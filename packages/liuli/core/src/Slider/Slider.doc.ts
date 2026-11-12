import type { ComponentDoc } from '../doc';

export const doc: ComponentDoc = {
  name: 'Slider',
  category: 'inputs',
  status: 'preview',
  summary: 'A value picked along a track, with its label and the value in words',
  description:
    'Slider picks one number from a range: a volume, a size, a position in a recording. ' +
    'A label row names it and can show the value in words the app writes; the track and thumb are the primitive slider.',
  keywords: ['slider', 'range', 'scrubber', 'volume', 'seek', 'position', 'track', 'thumb'],
  propsType: 'SliderProps',
  recipe: { map: 'sliderRecipe', parts: 'sliderParts' },
  anatomy: [
    { part: 'label', required: true, description: 'The visible label; visually hidden with `isLabelHidden`, still the name.' },
    { part: 'value', required: false, description: 'The value in words, from `valueLabel`; read as the description.' },
    { part: 'control', required: true, description: 'The slider primitive: track, filled part and thumb, and the accessible node.' },
    { part: 'description', required: false, description: 'Help text, read after the value.' },
  ],
  accessibility: {
    pattern: 'slider',
    name: 'label',
    description: ['valueLabel', 'description'],
    states: [{ state: 'disabled', from: 'isDisabled' }],
    keyboard: [
      { keys: 'Left / Down, Right / Up', effect: 'One step down or up.' },
      { keys: 'Home, End', effect: 'The minimum or the maximum.' },
    ],
    target: { md: 40 },
    contrast: [
      { what: 'thumb and filled track', pair: ['accent', 'surface'], minimum: '3:1', proven: true },
      { what: 'empty track', pair: ['line', 'surface'], minimum: '3:1', proven: false },
    ],
    notes: [
      'The device reads the number as the value; the words in `valueLabel` follow as the description until sliders take a value text.',
      'One size until the primitive takes thumb and track styling; the press area is 40 px tall.',
    ],
  },
  guidance: {
    do: [
      'Show the value in words when the number alone means little: "Large", "1:24 of 3:10".',
      'Pick a step the person can hit with a finger: at most a hundred stops along the track.',
    ],
    dont: [
      'Use a slider for an exact number the person knows; use a text input.',
      'Act on every change when the work is slow; apply when the value settles.',
    ],
  },
  examples: [
    { name: 'Volume', title: 'With the value in words' },
    { name: 'Stepped', title: 'Whole steps, with help text' },
    { name: 'InList', title: 'As a list row' },
    { name: 'Position', title: 'A position in a recording' },
  ],
  gaps: [
    {
      missing: 'A second, quieter range on the track for what is loaded (buffered media).',
      needs: 'a secondary range value on the slider primitive, painted between track and fill',
      today: 'Not drawn; a media scrubber shows no buffer.',
    },
    {
      missing: 'Liuli\'s own thumb: a surface disc with a hairline and shadow that grows slightly while pressed.',
      needs: 'slider thumb and track styling (fill, border, shadow, size, pressed state) on the primitive',
      today: 'The primitive\'s solid 24 px accent thumb and 4 px track, tinted with text-accent.',
    },
    {
      missing: 'A spoken value in words ("Large") in place of the number.',
      needs: 'aria-valuetext on the slider primitive, as progressbar has',
      today: 'The words are read as the description, after the number.',
    },
    {
      missing: 'The track lined up with the label above it.',
      needs: 'the slider inset to read per-side padding (`p-0` sets the sides, not the shorthand)',
      today: 'The track starts 12 px in from the label; `p-0` falls back to the default 12 px inset.',
    },
  ],
  related: ['ProgressBar'],
  gallery: {
    specimen: 'Specimen',
    tagline: 'A value along a track',
    order: 6,
    page: {
      lead: 'Use a slider when the value is easier to feel than to type: a volume, a size, a place in a recording.',
      hero: 'Volume',
      variants: [
        { name: 'Stepped', title: 'Whole steps, named in words' },
        { name: 'InList', title: 'As a row in a list, beside Item rows' },
      ],
      states: [
        { name: 'StateDefault', title: 'Default' },
        { name: 'StateDisabled', title: 'Disabled' },
      ],
      inContext: [{ name: 'Position', title: 'A position in a recording' }],
    },
  },
};

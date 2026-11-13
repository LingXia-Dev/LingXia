import type { ComponentDoc } from '../doc';

export const doc: ComponentDoc = {
  name: 'Slider',
  category: 'inputs',
  status: 'preview',
  summary: 'A value picked along a track, with its label and the value in words',
  description:
    'Slider picks one number from a range: a volume, a size, a position in a recording. ' +
    'A label row names it and can show the value in words the app writes, which is also what a screen reader says. ' +
    'A second, quieter range can show how much of a recording has loaded.',
  keywords: ['slider', 'range', 'scrubber', 'volume', 'seek', 'position', 'track', 'thumb'],
  propsType: 'SliderProps',
  recipe: { map: 'sliderRecipe', parts: 'sliderParts' },
  anatomy: [
    { part: 'label', required: true, description: 'The visible label; visually hidden with `isLabelHidden`, still the name.' },
    { part: 'value', required: false, description: 'The value in words, from `valueLabel`; read as the slider\'s value.' },
    { part: 'control', required: true, description: 'The slider primitive: track, range, the optional secondary range, the thumb, and the accessible node.' },
    { part: 'description', required: false, description: 'Help text, read as the description.' },
  ],
  accessibility: {
    pattern: 'slider',
    name: 'label',
    description: ['description'],
    states: [{ state: 'disabled', from: 'isDisabled' }],
    keyboard: [
      { keys: 'Left / Down, Right / Up', effect: 'One step down or up.' },
      { keys: 'Home, End', effect: 'The minimum or the maximum.' },
    ],
    target: { sm: 32, md: 40 },
    contrast: [
      { what: 'filled range', pair: ['accent', 'surface'], minimum: '3:1' },
      { what: 'thumb against the range', pair: ['surface-raised', 'accent'], minimum: '3:1' },
      { what: 'keyboard focus ring', pair: ['focus', 'surface'], minimum: '3:1' },
    ],
    notes: [
      '`valueLabel` is the spoken value: "Large" in place of 4, "1:24 of 3:10" in place of 84.',
      'The press area is the whole control, 40 px tall (32 at `sm`), whatever the thumb\'s drawn size.',
      'The empty track and the secondary range are decorative and have no contrast minimum: the value is carried by the range and the thumb.',
      'The thumb is drawn on the raised surface, not the accent, so it reads on the range and the track alike; its edge is a hairline plus a contact shadow.',
      'Disabled, the range keeps its accent at reduced strength, so the value still shows as the filled part.',
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
    { name: 'Position', title: 'A position in a recording, with what has loaded' },
    { name: 'Sizes', title: 'Small and medium' },
  ],
  gaps: [],
  related: ['ProgressBar'],
  gallery: {
    specimen: 'Specimen',
    tagline: 'A value on a track',
    order: 6,
    page: {
      lead: 'Use a slider when the value is easier to feel than to type: a volume, a size, a place in a recording.',
      hero: 'Volume',
      variants: [
        { name: 'Stepped', title: 'Whole steps, named in words' },
        { name: 'InList', title: 'As a row in a list, beside Item rows' },
      ],
      sizes: [{ name: 'Sizes', title: 'Small for dense rows and media, medium by default' }],
      states: [
        { name: 'StateDefault', title: 'Default' },
        { name: 'StateDisabled', title: 'Disabled' },
      ],
      inContext: [{ name: 'Position', title: 'A position in a recording, with what has loaded' }],
    },
  },
};

import type { ComponentDoc } from '../doc';

export const doc: ComponentDoc = {
  name: 'Slider',
  category: 'inputs',
  status: 'preview',
  since: '2026-11-12',
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
    description: ['description', 'disabledMessage, in its place while disabled'],
    states: [{ state: 'disabled', from: 'isDisabled' }],
    keyboard: [
      { keys: 'Left / Down, Right / Up', effect: 'One step down or up.' },
      { keys: 'Home, End', effect: 'The minimum or the maximum.' },
    ],
    target: { sm: 32, md: 40 },
    contrast: [
      { what: 'filled range against the surface', pair: ['surface', 'accent/80'], over: 'surface', minimum: '3:1' },
      { what: 'thumb rim against the track', pair: ['accent', 'line'], minimum: '3:1' },
      { what: 'thumb against the range', pair: ['knob', 'accent/80'], over: 'surface', minimum: '3:1' },
      { what: 'thumb against the range, straight on the page', pair: ['knob', 'accent/80'], over: 'canvas', minimum: '3:1' },
      { what: 'keyboard focus ring', pair: ['focus', 'surface'], minimum: '3:1' },
    ],
    notes: [
      '`valueLabel` is the spoken value: "Large" in place of 4, "1:24 of 3:10" in place of 84.',
      'A range is two sliders in a group named by `label`, each named by its `thumbLabels` entry and its own Tab stop; each reads its number, so `valueLabel` there is drawn only.',
      'The press area is the whole control, 40 px tall (32 at `sm`), whatever the thumb\'s drawn size.',
      'The empty track and the secondary range are decorative and have no contrast minimum: the value is carried by the range and the thumb.',
      'The thumb is a `knob` disc, white in both appearances, with an accent rim, so it reads on the range, the track and a dark page alike. On the solid dark accent it would fall to 2.72:1, so the range is the accent at 80 %.',
      'Disabled, the range keeps its accent at reduced strength, so the value still shows as the filled part; the thumb takes the surface, so it does not look pressable in dark either.',
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
    { name: 'Range', title: 'A range, with two thumbs' },
    { name: 'DisabledWithReason', title: 'Disabled, with the reason' },
    { name: 'Stepped', title: 'Whole steps, with help text' },
    { name: 'InList', title: 'As a list row' },
    { name: 'Marks', title: 'With marks' },
    { name: 'SaveOnRelease', title: 'Acting when the drag ends' },
    { name: 'Position', title: 'A position in a recording, with what has loaded' },
    { name: 'Sizes', title: 'Small and medium' },
  ],
  gaps: [
    {
      missing: 'A tone for use over a picture, sizing of the control box, and a forced left-to-right direction.',
      needs: 'a media tone and part-level layout props on Slider',
      today: 'VideoControls builds its scrubber on `<slider>` directly.',
    },
  ],
  playground: { props: { value: 40, label: 'Volume', min: 0, max: 100 } },
  related: ['ProgressBar'],
  gallery: {
    specimen: 'Specimen',
    tagline: 'A value on a track',
    order: 5,
    page: {
      lead: 'Use a slider when the value is easier to feel than to type: a volume, a size, a place in a recording.',
      hero: 'Volume',
      variants: [
        { name: 'Stepped', title: 'Whole steps, named in words' },
        { name: 'Range', title: 'A range: two thumbs that stop at each other, each with its own name' },
        { name: 'InList', title: 'As a row in a list, beside Item rows' },
        { name: 'Marks', title: 'Marks: ticks under the track, labelled where it helps' },
        { name: 'SaveOnRelease', title: 'Saving once, when the drag ends' },
      ],
      sizes: [{ name: 'Sizes', title: 'Small for dense rows and media, medium by default' }],
      states: [
        { name: 'StateDefault', title: 'Default' },
        { name: 'StateDisabled', title: 'Disabled' },
        { name: 'DisabledWithReason', title: 'Disabled, with the reason in place of the help text' },
      ],
      inContext: [{ name: 'Position', title: 'A position in a recording, with what has loaded' }],
    },
  },
};

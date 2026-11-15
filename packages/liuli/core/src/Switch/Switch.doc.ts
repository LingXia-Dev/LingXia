import type { ComponentDoc } from '../doc';

export const doc: ComponentDoc = {
  name: 'Switch',
  category: 'selection',
  status: 'preview',
  since: '2026-11-12',
  summary: 'A setting that turns on or off the moment it flips',
  description:
    'Switch turns one setting on or off and applies it at once, with no Save. ' +
    'The label, its help text and the track are one press target; the help text is read as the description.',
  keywords: ['switch', 'toggle', 'on off', 'setting', 'preference', 'enable', 'disable'],
  propsType: 'SwitchProps',
  recipe: { map: 'switchRecipe', parts: 'switchParts' },
  anatomy: [
    { part: 'control', required: true, description: 'The switch primitive: its content at the start, the track and thumb at the end, and the accessible node.' },
    { part: 'label', required: true, description: 'The visible label, also the name.' },
    { part: 'description', required: false, description: 'Help text under the label, inside the press target, read as the description.' },
  ],
  accessibility: {
    pattern: 'switch',
    name: 'label',
    description: ['description'],
    states: [
      { state: 'checked', from: 'value or bind' },
      { state: 'disabled', from: 'isDisabled' },
    ],
    keyboard: [{ keys: 'Space', effect: 'Flips it.' }],
    target: { sm: 32, md: 32 },
    contrast: [
      { what: 'track, on, against the surface', pair: ['surface', 'accent/80'], over: 'surface', minimum: '3:1' },
      { what: 'thumb against the track, on', pair: ['knob', 'accent/80'], over: 'surface', minimum: '3:1' },
      { what: 'thumb against the track, on, straight on the page', pair: ['knob', 'accent/80'], over: 'canvas', minimum: '3:1' },
      { what: 'thumb against the track, on, inside a popover or sheet', pair: ['knob', 'accent/80'], over: 'surface-overlay', minimum: '3:1' },
      { what: 'keyboard focus ring', pair: ['focus', 'surface'], minimum: '3:1' },
      { what: 'label', pair: ['text', 'surface'], minimum: '4.5:1' },
    ],
    notes: [
      'On and off differ in the thumb\'s side as well as the track colour, never in colour alone.',
      'The standalone switch is 32 px tall at both sizes; the row variant gives it a 48 px row.',
      'The off track is decorative and has no contrast minimum: off is carried by the thumb at the start and the missing accent.',
      'Disabled and on, the track keeps its accent at reduced strength, so the state still reads.',
      'The thumb is the `knob` role, white in both appearances. On the solid dark accent it would fall to 2.72:1, so the on track is the accent at 80 %.',
      'Keyboard focus rings a standalone switch; on a row it is a fill across the row and its help text, as on an Item.',
    ],
  },
  guidance: {
    do: [
      'Name the setting, not the action: "Wi-Fi", not "Turn on Wi-Fi".',
      'Use a switch only when the change applies at once.',
    ],
    dont: [
      'Use a switch on a form with a Save button; use a CheckboxInput.',
      'Put the state in the label ("Wi-Fi on"); the switch shows it.',
    ],
  },
  examples: [
    { name: 'Basic', title: 'With help text' },
    { name: 'InList', title: 'As list rows' },
    { name: 'Sizes', title: 'Small and medium' },
    { name: 'Applying', title: 'Waiting for the app to apply it' },
  ],
  gaps: [
    {
      missing: 'Keyboard focus shown only by the thumb ring or the row fill.',
      needs: 'the switch primitive\'s built-in focus ring giving way to authored focus styles',
      today: 'The built-in ring also draws around the track.',
    },
  ],
  related: ['CheckboxInput', 'Item'],
  gallery: {
    specimen: 'Specimen',
    tagline: 'On or off, at once',
    order: 3,
    page: {
      lead: 'Use a switch for a setting that takes effect the moment it flips. On a form that is sent later, use a checkbox.',
      hero: 'Basic',
      variants: [
        { name: 'InList', title: 'As rows in a list, beside an Item' },
        { name: 'Applying', title: 'Loading: waiting for the app to apply the change' },
      ],
      sizes: [{ name: 'Sizes', title: 'Small (36 by 20) for dense rows, medium (44 by 26) by default' }],
      states: [
        { name: 'StateOff', title: 'Off' },
        { name: 'StateOn', title: 'On' },
        { name: 'StateDisabledOff', title: 'Disabled, off' },
        { name: 'StateDisabledOn', title: 'Disabled, on' },
      ],
    },
  },
};

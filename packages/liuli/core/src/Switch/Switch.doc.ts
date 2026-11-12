import type { ComponentDoc } from '../doc';

export const doc: ComponentDoc = {
  name: 'Switch',
  category: 'inputs',
  status: 'preview',
  summary: 'A setting that turns on or off the moment it flips',
  description:
    'Switch turns one setting on or off and applies it at once, with no Save. ' +
    'The label and the track are one press target; help text below is read as the description.',
  keywords: ['switch', 'toggle', 'on off', 'setting', 'preference', 'enable', 'disable'],
  propsType: 'SwitchProps',
  recipe: { map: 'switchRecipe', parts: 'switchParts' },
  anatomy: [
    { part: 'control', required: true, description: 'The switch primitive: the label at the start, the track and thumb at the end, and the accessible node.' },
    { part: 'description', required: false, description: 'Help text under the label, read as the description.' },
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
      { what: 'track, on', pair: ['accent', 'surface'], minimum: '3:1' },
      { what: 'thumb against the track, on', pair: ['surface-raised', 'accent'], minimum: '3:1' },
      { what: 'keyboard focus ring', pair: ['focus', 'surface'], minimum: '3:1' },
      { what: 'label', pair: ['text', 'surface'], minimum: '4.5:1' },
    ],
    notes: [
      'On and off differ in the thumb\'s side as well as the track colour, never in colour alone.',
      'The standalone switch is 32 px tall at both sizes; the row variant gives it a 48 px row.',
      'The off track is decorative and has no contrast minimum: off is carried by the thumb at the start and the missing accent.',
      'Disabled and on, the track keeps its accent at reduced strength, so the state still reads.',
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
  ],
  gaps: [
    {
      missing: 'Help text that toggles the switch when tapped.',
      needs: 'a switch that takes element children, as a checkbox does',
      today: 'Only the label row is the press target; the help text sits outside it.',
    },
  ],
  related: ['CheckboxInput', 'Item'],
  gallery: {
    specimen: 'Specimen',
    tagline: 'On or off, at once',
    order: 5,
    page: {
      lead: 'Use a switch for a setting that takes effect the moment it flips. On a form that is sent later, use a checkbox.',
      hero: 'Basic',
      variants: [{ name: 'InList', title: 'As rows in a list, beside an Item' }],
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

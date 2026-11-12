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
    target: { md: 32 },
    contrast: [
      { what: 'track, on', pair: ['accent', 'surface'], minimum: '3:1', proven: true },
      { what: 'track, off', pair: ['line-strong', 'surface'], minimum: '3:1', proven: true },
      { what: 'label', pair: ['text', 'surface'], minimum: '4.5:1', proven: true },
    ],
    notes: [
      'On and off differ in the thumb\'s side as well as the track colour, never in colour alone.',
      'The standalone switch is 32 px tall; the row variant gives it a 48 px row.',
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
  ],
  gaps: [
    {
      missing: 'Liuli\'s own track and thumb: a thumb with a hairline and contact shadow, a smaller track for dense rows.',
      needs: 'switch track and thumb styling (size, fill, border, shadow) on the primitive',
      today: 'The primitive\'s 52 by 32 track and plain white thumb, at one size.',
    },
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
      states: [
        { name: 'StateOff', title: 'Off' },
        { name: 'StateOn', title: 'On' },
        { name: 'StateDisabledOff', title: 'Disabled, off' },
        { name: 'StateDisabledOn', title: 'Disabled, on' },
      ],
    },
  },
};

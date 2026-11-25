import type { ComponentDoc } from '../doc';

export const doc: ComponentDoc = {
  name: 'ToggleButton',
  category: 'actions',
  status: 'preview',
  since: '2026-11-13',
  summary: 'A button that turns one option on or off and stays down while on',
  description:
    'ToggleButton turns an option on or off where it applies, such as a filter over a list or a view option beside what ' +
    'it changes. It keeps its label in both states and reports pressed; while on it takes the soft accent fill. For a ' +
    'setting in a form use Switch; to choose one of a few values use SegmentedControl.',
  keywords: ['toggle button', 'toggle', 'pressed', 'filter', 'chip', 'on off', 'option', 'button'],
  propsType: 'ToggleButtonProps',
  recipe: { map: 'toggleButtonRecipe', parts: 'toggleButtonParts' },
  anatomy: [
    { part: 'root', required: true, description: 'The pressable box: a secondary button while off, the soft accent fill and an accent hairline while on.' },
    { part: 'glyph', required: false, description: 'The icon before the label, or alone in a square; `pressedIcon` while on.' },
    { part: 'label', required: false, description: 'The text and the name; the same on and off. Not drawn with `isLabelHidden`, but still the name.' },
  ],
  accessibility: {
    pattern: 'toggleButton',
    name: 'label',
    description: [],
    states: [
      { state: 'pressed', from: 'value or bind' },
      { state: 'disabled', from: 'isDisabled' },
    ],
    keyboard: [
      { keys: 'Enter, Space', effect: 'Turns it on or off.' },
    ],
    target: { sm: 32, md: 40, lg: 48 },
    contrast: [
      { what: 'label, off', pair: ['text', 'surface'], minimum: '4.5:1' },
      { what: 'label, on', pair: ['text', 'accent-soft'], minimum: '4.5:1' },
      { what: 'boundary, off', pair: ['line-strong', 'surface'], minimum: '3:1' },
      { what: 'disabled label, off, on a card', pair: ['text-muted', 'text/6'], over: 'surface', minimum: '4.5:1' },
      { what: 'disabled label, on', pair: ['text-muted', 'accent-soft'], minimum: '4.5:1' },
    ],
    notes: [
      'The state is read as pressed and drawn as the fill and the edge, never as the label\'s colour alone.',
      'Disabled keeps the soft fill when on, so a locked option still shows its state.',
      'An icon-only toggle keeps its label as its name; `pressedIcon` changes the picture, never the name.',
      'Sizes sm and md are shorter than the 44 pt touch minimum; a finger still reaches them in a 44 pt band around the box, which the host adds without changing what is drawn. Leave room between neighbours, since two bands that meet share the gap.',
    ],
  },
  guidance: {
    do: [
      'Label the option, not the action: "Unread", not "Show unread".',
      'Put it next to what it changes, and apply the change at once.',
    ],
    dont: [
      'Change the label with the state; the pressed state already says which.',
      'Line up loose toggle buttons where only one may be on; that is a ToggleButtonGroup with `selectionMode="single"`, or a SegmentedControl for a setting.',
      'Use it for a setting saved with a form; that is a Switch or a Checkbox.',
    ],
  },
  examples: [
    { name: 'Filters', title: 'Filters over a list' },
    { name: 'WordWrap', title: 'A view option' },
    { name: 'Icons', title: 'With icons, and icon-only' },
    { name: 'Sizes', title: 'Sizes' },
  ],
  gaps: [],
  playground: { props: { label: 'Unread', value: false } },
  related: ['ToggleButtonGroup', 'Button', 'SegmentedControl', 'Switch'],
  gallery: {
    specimen: 'Specimen',
    tagline: 'Stays down while on',
    order: 2,
    page: {
      lead: 'Use a toggle button to turn one option on or off right where it applies.',
      hero: 'Filters',
      sizes: [{ name: 'Sizes', title: 'Small, medium and large, on' }],
      states: [
        { name: 'StateOff', title: 'Off' },
        { name: 'StateOn', title: 'On' },
        { name: 'StateDisabled', title: 'Disabled, off' },
        { name: 'StateDisabledOn', title: 'Disabled, on' },
      ],
      variants: [{ name: 'Icons', title: 'An icon beside the label, an icon that changes when on, and icon-only' }],
      inContext: [{ name: 'WordWrap', title: 'A view option beside what it changes' }],
    },
  },
};

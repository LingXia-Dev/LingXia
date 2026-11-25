import type { ComponentDoc } from '../doc';

export const doc: ComponentDoc = {
  name: 'ToggleButtonGroup',
  category: 'actions',
  status: 'preview',
  since: '2026-11-25',
  summary: 'Toggle buttons that belong together, one on at a time or any number, as one keyboard stop',
  description:
    'ToggleButtonGroup lays out toggles that act on the same thing, such as filters over a list or a toolbar\'s layout choice. ' +
    'With `selectionMode="single"` one is on at a time and it is a radio group; with `"multiple"` each turns on or off on its own and it is a toolbar of toggle buttons. ' +
    'Either way the group is one Tab stop and the arrow keys move between the toggles. For a setting in a form use SegmentedControl.',
  keywords: ['toggle button group', 'toggle group', 'toolbar', 'radio', 'filters', 'view switcher', 'pressed'],
  propsType: 'ToggleButtonGroupProps',
  recipe: { map: 'toggleButtonGroupRecipe', parts: 'toggleButtonGroupParts' },
  anatomy: [
    { part: 'root', required: true, description: 'The row, named by `label`: a radio group with one on at a time, a toolbar otherwise. Each toggle is drawn as a ToggleButton.' },
  ],
  accessibility: {
    pattern: null,
    name: 'label',
    description: [],
    states: [
      { state: 'checked', from: 'value or bind, with selectionMode single' },
      { state: 'pressed', from: 'value or bind, with selectionMode multiple' },
      { state: 'disabled', from: 'isDisabled, or an item\'s isDisabled' },
    ],
    keyboard: [
      { keys: 'Tab', effect: 'Enters the group on one toggle and leaves it' },
      { keys: 'Left, Right', effect: 'Move between the toggles; with one on at a time, the arrival turns it on' },
      { keys: 'Enter or Space', effect: 'Turns the focused toggle on, or on and off with any number on' },
    ],
    contrast: [
      { what: 'boundary, on', pair: ['accent', 'accent-soft'], minimum: '3:1' },
    ],
    notes: [
      'One on at a time: role radiogroup, each toggle a radio read as checked. Any number on: role toolbar, each toggle a button read as pressed.',
      'An icon-only toggle keeps its label as its name. The state is drawn as the soft accent fill and the accent edge, never as the icon\'s colour alone.',
    ],
  },
  guidance: {
    do: [
      'Name the group for what its toggles share: "Filters", "Layout".',
      'Keep icon-only toggles to pictures people know, and keep the label as the name.',
    ],
    dont: [
      'Use it for a setting saved with a form; that is a SegmentedControl, a RadioGroup or a CheckboxList.',
      'Mix actions into it; a button that does something once belongs in a ButtonGroup.',
    ],
  },
  examples: [
    { name: 'Filters', title: 'Any number on' },
    { name: 'Layout', title: 'One on at a time, icon-only' },
    { name: 'Single', title: 'One on at a time, with labels' },
    { name: 'Sizes', title: 'Sizes' },
  ],
  gaps: [],
  playground: { props: { label: 'Pen', selectionMode: 'single', value: 'fine' }, fixture: true },
  related: ['ToggleButton', 'ButtonGroup', 'SegmentedControl'],
  gallery: {
    specimen: 'Specimen',
    tagline: 'Toggles together',
    order: 5,
    page: {
      lead: 'Use a toggle button group for toggles that belong together, with one on at a time or any number.',
      hero: 'Filters',
      variants: [
        { name: 'Layout', title: 'One on at a time, icon-only, read as a radio group' },
        { name: 'Single', title: 'One on at a time, with labels' },
      ],
      sizes: [{ name: 'Sizes', title: 'Small, medium and large' }],
      states: [{ name: 'StateDisabled', title: 'Disabled' }],
    },
  },
};

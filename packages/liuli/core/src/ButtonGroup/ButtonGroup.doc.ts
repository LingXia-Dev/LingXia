import type { ComponentDoc } from '../doc';

export const doc: ComponentDoc = {
  name: 'ButtonGroup',
  category: 'actions',
  status: 'preview',
  since: '2026-11-14',
  summary: 'Buttons that belong together, apart or joined edge to edge, as one keyboard stop',
  description:
    'ButtonGroup lays out Buttons and IconButtons that act on the same thing. Apart, they keep a gap and wrap; attached, they share edges and read as one shape. ' +
    'The group gives its buttons a size and a variant, can disable them all, and is a toolbar for assistive tech: one Tab stop, with the arrow keys moving among the buttons.',
  keywords: ['button group', 'toolbar', 'action group', 'attached buttons', 'split actions', 'button bar'],
  propsType: 'ButtonGroupProps',
  recipe: { map: 'buttonGroupRecipe', parts: 'buttonGroupParts' },
  anatomy: [
    { part: 'root', required: true, description: 'The row, and the toolbar assistive tech names by `label`. In an attached group each Button rounds only the group\'s two ends.' },
  ],
  accessibility: {
    pattern: null,
    name: 'label',
    description: [],
    states: [{ state: 'disabled', from: 'isDisabled, on every button' }],
    keyboard: [
      { keys: 'Tab', effect: 'Enters the group on one button and leaves it' },
      { keys: 'Left, Right', effect: 'Move focus between the buttons without pressing them' },
      { keys: 'Enter or Space', effect: 'Presses the focused button' },
    ],
    contrast: [],
    notes: [
      'Role toolbar, named by `label`; each button keeps its own name and state.',
      'The shared edge of attached secondary buttons is decorative and below 3:1 by design: each button is told apart by its label or icon, not by the line.',
      'A group is not a choice among values: nothing in it stays selected. For that use SegmentedControl or a ToggleButton.',
    ],
  },
  guidance: {
    do: [
      'Group actions on the same object, with the main action last.',
      'Attach buttons of one kind and equal weight: undo and redo, zoom out and in.',
      'Name the group for what its buttons share: "Text style", "Message actions".',
      'Set `size` and `variant` on the group, and on a button only to make one stand out.',
    ],
    dont: [
      'Attach a primary button to others; let it stand apart.',
      'Use a group to choose a value or a view; that is a SegmentedControl or Tabs.',
      'Put more buttons in an attached group than fit one line; attached groups do not wrap.',
    ],
  },
  examples: [
    { name: 'Actions', title: 'Actions on one thing' },
    { name: 'Attached', title: 'Attached' },
    { name: 'IconsAttached', title: 'Attached icon buttons' },
    { name: 'Sizes', title: 'Sizes' },
    { name: 'Toolbar', title: 'A toolbar' },
  ],
  gaps: [
    {
      missing: 'An attached group that wraps or scrolls when it does not fit.',
      needs: 'reading a laid-out box\'s size (`useBox`) to fold buttons into a menu',
      today: 'An attached group stays on one line and can be clipped; keep it short.',
    },
    {
      missing: 'A vertical group.',
      needs: 'nothing from the View',
      today: 'Rows only.',
    },
  ],
  related: ['Button', 'IconButton', 'SegmentedControl'],
  gallery: {
    specimen: 'Specimen',
    tagline: 'Buttons together',
    order: 4,
    page: {
      lead: 'Use a button group for actions on the same thing, so they read and are reached as one set.',
      hero: 'Actions',
      variants: [
        { name: 'Attached', title: 'Attached: actions of one kind share edges' },
        { name: 'IconsAttached', title: 'Attached icon buttons' },
      ],
      sizes: [{ name: 'Sizes', title: 'The group sets the size of its buttons' }],
      states: [{ name: 'StateDisabled', title: 'Disabled, all at once' }],
      inContext: [{ name: 'Toolbar', title: 'A quiet group of icon buttons, with a labelled action apart' }],
    },
  },
};

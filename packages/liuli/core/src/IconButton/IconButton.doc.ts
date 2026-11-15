import type { ComponentDoc } from '../doc';

export const doc: ComponentDoc = {
  name: 'IconButton',
  category: 'actions',
  status: 'preview',
  since: '2026-11-14',
  summary: 'A button that shows only an icon, with a name that is always read',
  description:
    'IconButton is a Button drawn as a square with its icon alone, for actions people know by sight: like, share, close, more. ' +
    'It has no visible text, so `label` is required and is what a screen reader says. ' +
    'It takes Button\'s variants, sizes, loading and disabled states, and is quiet by default.',
  keywords: ['icon button', 'toolbar button', 'action icon', 'square button', 'close button', 'more button'],
  propsType: 'IconButtonProps',
  recipe: { map: 'iconButtonRecipe', parts: 'iconButtonParts' },
  anatomy: [
    { part: 'root', required: true, description: 'The button: a square of 32, 40 or 48 points with the icon centred, in Button\'s variant; the spinner takes the icon\'s place while loading.' },
  ],
  accessibility: {
    pattern: 'button',
    name: 'label',
    description: ['disabledMessage, while disabled'],
    states: [{ state: 'disabled', from: 'isDisabled' }, { state: 'busy', from: 'isLoading, or a pending clickAction' }],
    keyboard: [],
    target: { sm: 32, md: 40, lg: 48 },
    contrast: [
      { what: 'quiet icon on a card', pair: ['text', 'surface'], minimum: '3:1' },
      { what: 'quiet icon under the pointer', pair: ['text', 'text/5'], over: 'surface', minimum: '3:1' },
      { what: 'primary icon', pair: ['on-accent', 'accent'], minimum: '3:1' },
      { what: 'destructive icon', pair: ['on-danger', 'danger'], minimum: '3:1' },
      { what: 'disabled icon', pair: ['text-muted', 'text/6'], over: 'surface', minimum: '3:1' },
    ],
    notes: [
      'Role button, named by `label`; the icon is hidden. The name stays the same while loading.',
      'On touch, a finger reaches a 32 or 40 point button within 44 points around it, never over a neighbour; the drawn square stays its size.',
      'The quiet variant has no edge at rest, so it relies on its place (a toolbar, the end of a row) to read as a control.',
    ],
  },
  guidance: {
    do: [
      'Name the action in `label`, with its object when rows repeat: "Remove Budget.xlsx".',
      'Use it for actions with an icon people already know; otherwise use a Button with text.',
      'Keep it quiet in a toolbar or a row; use `secondary` or `primary` when it stands alone.',
      'Group related icon buttons in a ButtonGroup, so arrow keys move among them.',
    ],
    dont: [
      'Use an icon button as the only way to a destructive action without a confirmation.',
      'Put two icon buttons with the same icon and different meanings on one screen.',
      'Shrink it with a class; use `size`.',
    ],
  },
  examples: [
    { name: 'Toolbar', title: 'A row of actions' },
    { name: 'Variants', title: 'Variants' },
    { name: 'Sizes', title: 'Sizes' },
    { name: 'InRows', title: 'At the end of a row' },
  ],
  gaps: [
    {
      missing: 'The label shown as a tooltip on hover and keyboard focus.',
      needs: 'a hint that opens from any control',
      today: 'The label is read by assistive tech only.',
    },
  ],
  related: ['Button', 'ButtonGroup', 'Icon'],
  gallery: {
    specimen: 'Specimen',
    tagline: 'An icon that acts',
    order: 3,
    page: {
      lead: 'Use an icon button for an action people know by its icon, where a word would crowd the row.',
      hero: 'Toolbar',
      variants: [{ name: 'Variants', title: 'Primary, secondary, quiet and destructive; quiet by default' }],
      sizes: [{ name: 'Sizes', title: '32, 40 and 48 points square' }],
      states: [
        { name: 'StateDefault', title: 'Default' },
        { name: 'StateBusy', title: 'Busy' },
        { name: 'StateDisabled', title: 'Disabled' },
      ],
      inContext: [{ name: 'InRows', title: 'At the end of a row, named with the row it acts on' }],
    },
  },
};

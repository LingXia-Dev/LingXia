import type { ComponentDoc } from '../doc';

export const doc: ComponentDoc = {
  name: 'ActionSheet',
  category: 'overlays',
  status: 'preview',
  since: '2026-11-25',
  summary: 'A short list of actions about one thing, with Cancel apart from them',
  description:
    'ActionSheet asks what to do with one thing: a photo, a draft, a message. Its rows are the answers, each a verb, ' +
    'a destructive one in the danger colour; Cancel stands apart under a hairline. On a phone it is a sheet on the ' +
    'bottom edge, in reach of a thumb, that drags off to cancel; on a wide window it is a small panel in the middle. ' +
    'Choosing a row closes the sheet and runs its action. The window feedback layer draws `lx.ui.actionSheet` with it.',
  keywords: ['action sheet', 'actions', 'choice', 'sheet', 'menu', 'modal', 'overlay', 'destructive'],
  propsType: 'ActionSheetProps',
  recipe: { map: 'actionSheetRecipe', parts: 'actionSheetParts' },
  anatomy: [
    { part: 'panel', required: true, description: 'The dialog: on the bottom edge on a phone, rounded at the top; in the middle when wider.' },
    { part: 'grabber', required: false, description: 'On a phone, the handle at the top edge; decoration, hidden from assistive tech.' },
    { part: 'header', required: false, description: 'The title (`label`, unless `isLabelHidden`) and `description`, in footnote type above the rows.' },
    { part: 'actions', required: true, description: 'The rows, in order; each a button with an optional icon. With any icon, every row keeps the icon\'s space, so the labels line up.' },
    { part: 'cancel', required: true, description: 'Cancel, under a hairline, apart from the actions.' },
  ],
  accessibility: {
    pattern: 'dialog',
    name: 'label',
    description: ['description'],
    states: [{ state: 'modal', from: 'isOpen' }, { state: 'disabled', from: 'items[].isDisabled' }],
    keyboard: [
      { keys: 'Tab, Shift+Tab', effect: 'Moves between the rows and Cancel; focus stays in the sheet.' },
      { keys: 'Enter, Space', effect: 'Chooses the focused row: the sheet closes and its action runs.' },
      { keys: 'Escape', effect: 'onOpenChange(false): cancels.' },
    ],
    contrast: [
      { what: 'row label', pair: ['text', 'surface-overlay'], minimum: '4.5:1' },
      { what: 'destructive row label', pair: ['danger', 'surface-overlay'], minimum: '4.5:1' },
      { what: 'description', pair: ['text-muted', 'surface-overlay'], minimum: '4.5:1' },
    ],
    notes: [
      'Android back, the screen reader dismiss gesture, a tap outside and, on a phone, a drag off screen all cancel, like Escape.',
      'The sheet pads its bottom by the safe area, so Cancel clears the home indicator.',
    ],
  },
  guidance: {
    do: [
      'Name each row by what it does: "Delete", "Save draft".',
      'Put the destructive action last among the actions, in the `destructive` variant.',
      'Say the consequence in `description` when an action cannot be undone.',
    ],
    dont: [
      'Use it for a long list or a choice of values; use `Select` or a `BottomSheet` with a list.',
      'Put the only way forward in it; an action sheet is always safe to cancel.',
      'Repeat Cancel as an item; the sheet has one.',
    ],
  },
  examples: [
    { name: 'Basic', title: 'Actions on a photo, with icons, the destructive one last' },
    { name: 'WithMessage', title: 'A question with its consequence under the title' },
    { name: 'Untitled', title: 'No drawn title: the rows speak for themselves' },
  ],
  gaps: [],
  playground: { props: { isOpen: false, label: 'This photo' }, fixture: true, opener: 'Photo options' },
  related: ['BottomSheet', 'DropdownMenu', 'AlertDialog'],
  gallery: {
    specimen: 'Specimen',
    tagline: 'Pick what to do',
    order: 6,
    page: {
      lead: 'Use an action sheet to ask what to do with one thing; Cancel is always there.',
      hero: 'Basic',
      parts: { name: 'Parts', title: 'The title, the actions, then Cancel apart' },
      variants: [
        { name: 'WithMessage', title: 'A question with its consequence spelled out' },
        { name: 'Untitled', title: 'No drawn title' },
      ],
    },
  },
};

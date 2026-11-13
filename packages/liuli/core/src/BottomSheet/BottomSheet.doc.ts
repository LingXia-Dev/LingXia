import type { ComponentDoc } from '../doc';

export const doc: ComponentDoc = {
  name: 'BottomSheet',
  category: 'overlays',
  status: 'preview',
  summary: 'A modal panel on the bottom edge that drags between detents and off to close',
  description:
    'BottomSheet holds a short task or a choice that belongs to the page under it: where to move a note, the comments on ' +
    'it. It is a dialog on the bottom edge, in reach of a thumb. A `fit` sheet is as tall as its content; an `expandable` ' +
    'one opens halfway and drags up to nearly the full height. Dragging it off screen closes it.',
  keywords: ['bottom sheet', 'sheet', 'drawer', 'action sheet', 'detents', 'modal', 'panel', 'overlay'],
  propsType: 'BottomSheetProps',
  recipe: { map: 'bottomSheetRecipe', parts: 'bottomSheetParts' },
  anatomy: [
    { part: 'panel', required: true, description: 'The dialog on the bottom edge, rounded at the top; it follows the finger between its detents.' },
    { part: 'grabber', required: true, description: 'The handle at the top edge; decoration, hidden from assistive tech.' },
    { part: 'header', required: false, description: 'The title, and the close button when `closeLabel` is set.' },
    { part: 'description', required: false, description: 'One or two sentences under the title, read as the description.' },
    { part: 'body', required: false, description: '`children`, scrolling when the sheet reaches its height; a drag in it moves the sheet first.' },
    { part: 'footer', required: false, description: '`actions`, end-aligned, wrapping when narrow.' },
  ],
  accessibility: {
    pattern: 'dialog',
    name: 'label',
    description: ['description'],
    states: [{ state: 'modal', from: 'isOpen' }],
    keyboard: [
      { keys: 'Escape', effect: 'onOpenChange(false), unless dismissOn is none' },
      { keys: 'Up, Down', effect: 'With the sheet focused, the next detent up or down.' },
    ],
    contrast: [
      { what: 'title', pair: ['text', 'surface-raised'], minimum: '4.5:1' },
      { what: 'description', pair: ['text-muted', 'surface-raised'], minimum: '4.5:1' },
    ],
    notes: [
      'Screen readers announce the sheet as adjustable: increment and decrement step it between detents.',
      'Android back, the screen reader dismiss gesture and a drag off screen are close requests, like Escape.',
      'The sheet pads its bottom by the safe area, so its last control clears the home indicator.',
    ],
  },
  guidance: {
    do: [
      'Use it for a choice or a short task about what is under it, on touch layouts.',
      'Use `expandable` for a list that can grow long; `fit` for a few rows or a question.',
      'Keep the main action in reach at the bottom.',
    ],
    dont: [
      'Open a sheet from a sheet; finish or close the first.',
      'Use `dismissOn="none"` unless the step must be answered.',
      'Put a whole page in a sheet; navigate to the page.',
    ],
  },
  examples: [
    { name: 'MoveTo', title: 'A choice, as tall as its list' },
    { name: 'Comments', title: 'A long list, opening halfway' },
    { name: 'Required', title: 'A question that must be answered' },
  ],
  gaps: [],
  related: ['Dialog', 'DropdownMenu'],
  gallery: {
    specimen: 'Specimen',
    tagline: 'In reach of a thumb',
    order: 4,
    page: {
      lead: 'Use a bottom sheet for a choice or a short task about the page under it.',
      hero: 'MoveTo',
      parts: { name: 'Parts', title: 'The grabber, the title, then the body' },
      variants: [
        { name: 'Comments', title: 'Expandable: opens halfway, drags up' },
        { name: 'Required', title: 'A question that must be answered' },
      ],
    },
  },
};

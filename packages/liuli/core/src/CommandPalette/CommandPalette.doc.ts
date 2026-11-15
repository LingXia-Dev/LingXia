import type { ComponentDoc } from '../doc';

export const doc: ComponentDoc = {
  name: 'CommandPalette',
  category: 'overlays',
  status: 'preview',
  since: '2026-11-15',
  summary: 'Type to find a command or a place, and run it',
  description:
    'CommandPalette is a dialog near the top of the screen with a search line over one list of results in named groups. Typing narrows the list, best match first; the arrow keys move a highlight that stays in view, and Return runs the row. ' +
    'On a phone it spans the width from the top edge, so the results scroll above the keyboard. The app opens it, from a button or its own shortcut, and can take over the filtering to fetch results.',
  keywords: ['command palette', 'command menu', 'quick open', 'search', 'launcher', 'spotlight'],
  propsType: 'CommandPaletteProps',
  recipe: { map: 'commandPaletteRecipe', parts: 'commandPaletteParts' },
  anatomy: [
    { part: 'panel', required: true, description: 'The dialog primitive, anchored near the top.' },
    { part: 'search', required: true, description: 'The search line: an icon, the editor, a spinner while loading.' },
    { part: 'list', required: false, description: 'The results: a listbox that scrolls.' },
    { part: 'group', required: false, description: 'A named group with its quiet heading.' },
    { part: 'option', required: false, description: 'One result: icon, label, a second line, and quiet text at the end.' },
    { part: 'note', required: false, description: '`emptyText`, when nothing matches.' },
  ],
  accessibility: {
    pattern: 'dialog',
    name: 'label',
    description: [],
    states: [
      { state: 'selected', from: 'the command the arrow keys are on' },
      { state: 'disabled', from: 'isDisabled on a command' },
    ],
    keyboard: [
      { keys: 'Up and Down', effect: 'Move the highlight; rows that cannot be chosen are passed over' },
      { keys: 'Home and End', effect: 'To the first and the last row' },
      { keys: 'Return', effect: 'Runs the highlighted row and closes' },
      { keys: 'Escape', effect: 'Closes' },
    ],
    target: { lg: 48 },
    contrast: [
      { what: 'row', pair: ['text', 'surface-overlay'], minimum: '4.5:1' },
      { what: 'second line and end text', pair: ['text-muted', 'surface-overlay'], minimum: '4.5:1' },
    ],
    notes: [
      'Focus stays in the search line, a combobox that names the highlighted row as it moves.',
      'The results are a listbox in named groups; a row with `endText` is read as "label, end text".',
      'When nothing matches, the note is a status and is read.',
      'A long list is cut by the bottom edge of the panel on purpose: a row cut in half says there is more to scroll.',
      'Closing returns focus to what had it.',
    ],
  },
  guidance: {
    do: [
      'Offer every command somewhere else too; the palette is a shortcut.',
      'Start each label with the word people will type: "New note", not "Create a new note".',
    ],
    dont: [
      'Use it to choose a value in a form; that is a Combobox.',
      'Put more than a few dozen rows in it unfiltered; hand filtering to the app and fetch.',
    ],
  },
  examples: [{ name: 'Commands', title: 'Commands and places' }],
  gaps: [],
  limits: [
    'It binds no shortcut of its own: the app opens it.',
    'A row does not lead to a second page of rows.',
  ],
  related: ['Combobox', 'SearchField', 'DropdownMenu', 'Kbd'],
  gallery: {
    specimen: 'Specimen',
    tagline: 'Find and run',
    order: 9,
    page: {
      lead: 'Use a command palette to reach any command or place by typing.',
      hero: 'Commands',
    },
  },
};

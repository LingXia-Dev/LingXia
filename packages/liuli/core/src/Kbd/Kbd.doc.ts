import type { ComponentDoc } from '../doc';

export const doc: ComponentDoc = {
  name: 'Kbd',
  category: 'content',
  status: 'preview',
  since: '2026-11-15',
  summary: 'A keyboard shortcut, drawn as key caps',
  description:
    'Kbd shows the keys of one shortcut, each as a small key cap, in the order they are held. It only shows them: the app binds the shortcut. ' +
    'It belongs where a pointer and a keyboard are present, at the end of a menu row, in a tooltip or in a list of shortcuts.',
  keywords: ['kbd', 'keyboard', 'shortcut', 'key', 'hotkey', 'accelerator'],
  propsType: 'KbdProps',
  recipe: { map: 'kbdRecipe', parts: 'kbdParts' },
  anatomy: [
    { part: 'root', required: true, description: 'The row of key caps.' },
    { part: 'key', required: true, description: 'One key cap: an outlined box at least as wide as it is tall.' },
    { part: 'label', required: true, description: 'The key\'s symbol or name.' },
  ],
  accessibility: {
    pattern: null,
    name: 'label',
    description: [],
    states: [],
    keyboard: [],
    target: {},
    contrast: [
      { what: 'key', pair: ['text-muted', 'surface'], minimum: '4.5:1' },
    ],
    notes: [
      'With `label` the shortcut is read once, as the label says: "Command K". Give it whenever a key is a symbol or short for a word.',
      'Without it each key is read as written, which suits keys that are words: "Enter".',
      'It takes no focus and no press.',
    ],
  },
  guidance: {
    do: [
      'Write the keys the way the platform shows them: ⌘ ⌥ ⇧ on a Mac, Ctrl and Shift elsewhere.',
      'Show a shortcut where its action is: beside the menu item, in the button\'s tooltip.',
    ],
    dont: [
      'Show shortcuts on a phone; there is no keyboard to press.',
      'Put several shortcuts in one Kbd; use one for each.',
    ],
  },
  examples: [
    { name: 'Shortcuts', title: 'A list of shortcuts' },
    { name: 'Hint', title: 'Beside a hint' },
  ],
  gaps: [],
  limits: [
    'The keys are whatever the app passes; Liuli does not know the platform\'s modifier names.',
  ],
  playground: { props: { label: 'Command K' }, fixture: true },
  related: ['Tooltip', 'DropdownMenu'],
  gallery: {
    specimen: 'Specimen',
    tagline: 'Keys of a shortcut',
    order: 12,
    page: {
      lead: 'Use Kbd to show the keys of a shortcut beside the action it runs.',
      hero: 'Shortcuts',
      variants: [{ name: 'Hint', title: 'In a line of help, one size up' }],
    },
  },
};

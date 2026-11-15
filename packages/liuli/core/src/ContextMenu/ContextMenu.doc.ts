import type { ComponentDoc } from '../doc';

export const doc: ComponentDoc = {
  name: 'ContextMenu',
  category: 'overlays',
  status: 'preview',
  since: '2026-11-15',
  summary: 'A menu for one thing, asked for on the thing',
  description:
    'ContextMenu wraps a row, a card or a picture and opens a menu of actions for it when the user asks: a finger held on it, a secondary click, the Menu key. ' +
    'From a finger the menu docks to the bottom edge with the thing\'s name over it; from a mouse or the keyboard it opens at the point. Its items are DropdownMenu\'s. ' +
    'Nothing shows that the menu is there, so every action in it must also be reachable another way.',
  keywords: ['context menu', 'long press', 'right click', 'secondary click', 'actions', 'menu'],
  propsType: 'ContextMenuProps',
  recipe: { map: 'contextMenuRecipe', parts: 'contextMenuParts' },
  anatomy: [
    { part: 'target', required: true, description: 'The wrapper around what the menu is for; it listens for the ask.' },
    { part: 'menu', required: true, description: 'A popover with the menu role: at the point, or docked to the bottom edge from a finger.' },
    { part: 'title', required: false, description: 'Docked: `label` over the items.' },
  ],
  accessibility: {
    pattern: null,
    name: 'label',
    description: [],
    states: [],
    keyboard: [
      { keys: 'Menu key or Shift+F10', effect: 'Opens the menu for the focused thing' },
      { keys: 'Up and Down', effect: 'Move between items' },
      { keys: 'Enter or Space', effect: 'Runs the item and closes the menu' },
      { keys: 'Escape', effect: 'Closes the menu' },
    ],
    target: { lg: 48 },
    contrast: [
      { what: 'item', pair: ['text', 'surface-overlay'], minimum: '4.5:1' },
      { what: 'docked title', pair: ['text-muted', 'surface-overlay'], minimum: '4.5:1' },
    ],
    notes: [
      'The menu is named by `label`; its items are menu items, and choosing one closes it.',
      'A screen reader user asks for the menu with the platform\'s own gesture for it.',
      'After a long press the same finger does not also press the thing.',
    ],
  },
  guidance: {
    do: [
      'Offer the same actions in view too: a More button on the row, or on the page the row opens.',
      'Keep the menu to the actions for this one thing, the destructive one last.',
    ],
    dont: [
      'Put an action only in a context menu; many people never find it.',
      'Wrap something that already uses a long press, such as selectable text.',
    ],
  },
  examples: [{ name: 'Rows', title: 'A menu on each row' }],
  gaps: [
    'It holds no submenu: a menu item that opens another menu closes the first.',
    'At a point the menu is placed from the size of the page; a very long menu can still reach past its edge.',
  ],
  related: ['DropdownMenu', 'Item', 'BottomSheet'],
  gallery: {
    specimen: 'Specimen',
    tagline: 'Hold for actions',
    order: 8,
    page: {
      lead: 'Use a context menu as a shortcut to the actions for one thing.',
      hero: 'Rows',
    },
  },
};

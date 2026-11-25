import type { ComponentDoc } from '../doc';

export const doc: ComponentDoc = {
  name: 'DropdownMenu',
  category: 'overlays',
  status: 'preview',
  since: '2026-11-12',
  summary: 'A short menu of actions that opens from its trigger',
  description:
    'DropdownMenu groups a few related actions behind one trigger: sort options, a "more" menu on a row, account actions. ' +
    'Its children are the trigger, a Button or IconButton; `menu` holds DropdownMenuItems and DropdownMenuDividers, and choosing an item runs its onClick and closes the menu. ' +
    'DropdownMenuCheckboxItem turns an option on or off, and DropdownMenuRadioItems in a DropdownMenuRadioGroup check one ' +
    'choice; both keep the menu open. ' +
    '`items` takes the same rows as data instead of `menu`: actions, `{ type: \'divider\' }`, `{ type: \'group\', label, items }`, and an action with `items` of its own as a submenu; checkable items are composed in `menu`.',
  keywords: ['dropdown', 'menu', 'more', 'overflow', 'kebab', 'actions', 'popover', 'context'],
  propsType: 'DropdownMenuProps',
  otherPropsTypes: ['DropdownMenuItemData', 'DropdownMenuItemProps', 'DropdownMenuCheckboxItemProps', 'DropdownMenuRadioGroupProps', 'DropdownMenuRadioItemProps'],
  recipe: { map: 'dropdownMenuRecipe', parts: 'dropdownMenuParts' },
  anatomy: [
    { part: 'menu', required: true, description: 'A popover with the menu role, anchored to the trigger, in the top layer.' },
    { part: 'item', required: true, description: 'DropdownMenuItem: a menu item button with icon, label and description.' },
    { part: 'itemCheck', required: false, description: 'A check at the end of a checked checkbox or radio item; hidden from assistive tech.' },
    { part: 'divider', required: false, description: 'DropdownMenuDivider: a hairline between groups; hidden from assistive tech.' },
    { part: 'submenuChevron', required: false, description: 'DropdownMenuSubmenu: an item with a chevron that opens a second menu at its side; choosing there closes both.' },
    { part: 'back', required: false, description: 'On a narrow screen a submenu takes the panel instead: this first row, "Back to" and the menu\'s label, goes back to the menu.' },
  ],
  accessibility: {
    pattern: 'menuButton',
    name: 'label',
    description: [],
    states: [
      { state: 'expanded', from: 'the trigger, while the menu is open (set by the View)' },
      { state: 'checked', from: "a checkbox item's value; a radio item matching its group's value" },
      { state: 'disabled', from: "the trigger's isDisabled; an item's isDisabled" },
    ],
    keyboard: [
      { keys: 'Enter, Space on the trigger', effect: 'opens the menu on its first item' },
      { keys: 'Up, Down', effect: 'moves between items, wrapping' },
      { keys: 'Home, End', effect: 'first and last item' },
      { keys: 'a letter', effect: 'the next item whose name starts with it' },
      { keys: 'Escape', effect: 'closes and returns focus to the trigger; inside a submenu that took the panel, goes back to the menu first' },
    ],
    target: { sm: 32, md: 40, lg: 48 },
    contrast: [
      { what: 'item text', pair: ['text', 'surface-overlay'], minimum: '4.5:1' },
      { what: 'destructive icon', pair: ['danger', 'surface-overlay'], minimum: '3:1' },
    ],
    notes: [
      'An item is named by its label and then its description, both inside the item.',
      'Sizes sm (32) and md (40) are shorter than the 44 pt touch minimum; a finger still reaches them in a 44 pt band around the box, which the host adds without changing what is drawn.',
      'A tap outside, Escape and back close the menu; that tap reaches nothing else.',
      'On a narrow screen the row that opens a submenu and the row that goes back are menu items that keep the menu open, so the arrow keys reach them; Escape and back go up one level before they close the menu.',
      'Checkbox and radio items are read as checked or not, and keep the menu open so the change can be seen.',
    ],
  },
  guidance: {
    do: [
      'At the end of a row, open it from a ghost Button with `edge="end"`: its padding hangs past the edge, so the chevron lines up with the values and badges in the rows around it.',
      'Use a `Link` as the trigger for words in a sentence: put the overlay in the `<text>`, around the Link, and the panel opens from the linked words.',
      'Keep it to a handful of actions; put destructive ones last, after a divider.',
      'Use the end alignment for a trigger at the end of a row.',
      'Give a labelled trigger the `chevron-down` end icon, so it reads as opening a menu.',
    ],
    dont: [
      'Use it to choose a value that should stay visible.',
      'Put it inside a Button; a popover cannot sit in a pressable control.',
      'Nest menus.',
    ],
  },
  examples: [
    { name: 'Basic', title: 'Labelled trigger' },
    { name: 'Shortcuts', title: 'Shortcuts at the trailing edge' },
    { name: 'ShortcutsRtl', title: 'Shortcuts, right to left' },
    { name: 'More', title: 'Icon-only "more" menu on a row' },
    { name: 'InList', title: 'A menu on every row' },
    { name: 'Value', title: 'A menu that sets a value' },
    { name: 'Checkable', title: 'Options and a choice, checked' },
    { name: 'StateMixed', title: 'An item for a set, partly on' },
    { name: 'Grouped', title: 'Groups under headings' },
    { name: 'Submenu', title: 'A submenu' },
    { name: 'Data', title: 'The rows as data' },
    { name: 'InSentence', title: 'On a link in a sentence' },
  ],
  gaps: [
    {
      missing: 'A submenu inside a submenu, and the menu itself opening at the trigger\'s side.',
      needs: 'nothing from the View',
      today: 'One level of submenu; the menu opens under or over its trigger.',
    },
    {
      missing: 'Anchoring when opened from code with isOpen.',
      needs: 'a popover anchor that does not depend on which control opened it',
      today: 'Opened from code it is placed by its classes; the trigger is the normal way in.',
    },
    {
      missing: 'A destructive item in danger-coloured text.',
      needs: 'a text-grade danger role (4.5:1 on surface-overlay)',
      today: 'The icon carries the danger colour; the text stays in the text role.',
    },
  ],
  playground: { props: { isOpen: false, label: 'Sort by' }, fixture: true },
  related: ['Button', 'ContextMenu'],
  gallery: {
    specimen: 'Specimen',
    tagline: 'A menu of actions',
    order: 2,
    page: {
      lead: 'Use a dropdown menu for a short list of actions that do not need to stay on screen.',
      hero: 'Basic',
      variants: [
        { name: 'Shortcuts', title: 'Shortcuts at the trailing edge, muted' },
        { name: 'ShortcutsRtl', title: 'Right to left: the shortcuts mirror to the left edge' },
        { name: 'More', title: 'Icon only, at the end of a row' },
        { name: 'Value', title: 'Showing the current choice' },
        { name: 'Checkable', title: 'Checkbox and radio items' },
        { name: 'Grouped', title: 'Named groups, for a menu with more than one kind of item' },
        { name: 'Submenu', title: 'A submenu at the side of its item, one level deep' },
        { name: 'Data', title: 'The same rows given as data: a submenu, a group and a divider' },
      ],
      sizes: [{ name: 'Sizes', title: 'Small, medium and large triggers' }],
      states: [
        { name: 'StateEnabled', title: 'Default' },
        { name: 'StateDisabled', title: 'Disabled' },
        { name: 'StateMixed', title: 'Mixed: an item for a set, partly on' },
      ],
      inContext: [{ name: 'InList', title: 'A menu on every row' }, { name: 'InSentence', title: 'On a link in a sentence' }],
    },
  },
};

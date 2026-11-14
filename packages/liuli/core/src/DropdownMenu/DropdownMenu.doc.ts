import type { ComponentDoc } from '../doc';

export const doc: ComponentDoc = {
  name: 'DropdownMenu',
  category: 'overlays',
  status: 'preview',
  summary: 'Button that opens a short menu of actions',
  description:
    'DropdownMenu groups a few related actions behind one button: sort options, a "more" menu on a row, account actions. ' +
    'Its children are DropdownMenuItems and DropdownMenuDividers; choosing an item runs its onClick and closes the menu. ' +
    'DropdownMenuCheckboxItem turns an option on or off, and DropdownMenuRadioItems in a DropdownMenuRadioGroup check one ' +
    'choice; both keep the menu open.',
  keywords: ['dropdown', 'menu', 'more', 'overflow', 'kebab', 'actions', 'popover', 'context'],
  propsType: 'DropdownMenuProps',
  otherPropsTypes: ['DropdownMenuItemProps', 'DropdownMenuCheckboxItemProps', 'DropdownMenuRadioGroupProps', 'DropdownMenuRadioItemProps'],
  recipe: { map: 'dropdownMenuRecipe', parts: 'dropdownMenuParts' },
  anatomy: [
    { part: 'chevron', required: false, description: 'The trigger is a Button-styled popover invoker; the chevron marks it as opening a menu.' },
    { part: 'menu', required: true, description: 'A popover with the menu role, anchored to the trigger, in the top layer.' },
    { part: 'item', required: true, description: 'DropdownMenuItem: a menu item button with icon, label and description.' },
    { part: 'itemCheck', required: false, description: 'A check at the end of a checked checkbox or radio item; hidden from assistive tech.' },
    { part: 'divider', required: false, description: 'DropdownMenuDivider: a hairline between groups; hidden from assistive tech.' },
  ],
  accessibility: {
    pattern: 'menuButton',
    name: 'label',
    description: [],
    states: [
      { state: 'expanded', from: 'the trigger, while the menu is open (set by the View)' },
      { state: 'checked', from: "a checkbox item's isChecked; a radio item matching its group's value" },
      { state: 'disabled', from: "isDisabled; an item's isDisabled" },
    ],
    keyboard: [
      { keys: 'Enter, Space on the trigger', effect: 'opens the menu on its first item' },
      { keys: 'Up, Down', effect: 'moves between items, wrapping' },
      { keys: 'Home, End', effect: 'first and last item' },
      { keys: 'a letter', effect: 'the next item whose name starts with it' },
      { keys: 'Escape', effect: 'closes and returns focus to the trigger' },
    ],
    target: { sm: 32, md: 40, lg: 48 },
    contrast: [
      { what: 'item text', pair: ['text', 'surface-overlay'], minimum: '4.5:1' },
      { what: 'destructive icon', pair: ['danger', 'surface-overlay'], minimum: '3:1' },
    ],
    notes: [
      'An item is named by its label and then its description, both inside the item.',
      'Sizes sm (32) and md (40) are below the 44 pt touch minimum; on touch layouts prefer lg until hit slop lets the press area extend past the paint.',
      'A tap outside, Escape and back close the menu; that tap reaches nothing else.',
      'Checkbox and radio items are read as checked or not, and keep the menu open so the change can be seen.',
    ],
  },
  guidance: {
    do: [
      'Keep it to a handful of actions; put destructive ones last, after a divider.',
      'Use the end alignment for a trigger at the end of a row.',
    ],
    dont: [
      'Use it to choose a value that should stay visible.',
      'Put it inside a Button; a popover cannot sit in a pressable control.',
      'Nest menus.',
    ],
  },
  examples: [
    { name: 'Basic', title: 'Labelled trigger' },
    { name: 'More', title: 'Icon-only "more" menu on a row' },
    { name: 'InList', title: 'A menu on every row' },
    { name: 'Value', title: 'A menu that sets a value' },
    { name: 'Checkable', title: 'Options and a choice, checked' },
  ],
  gaps: [
    {
      missing: 'Submenus and side placement.',
      needs: 'inline-start and inline-end position areas',
      today: 'One level; top or bottom only.',
    },
    {
      missing: 'Opening by long press or secondary click.',
      needs: 'a long-press and secondary-click invoker',
      today: 'Tap on the trigger only.',
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
  related: ['Button'],
  gallery: {
    specimen: 'Specimen',
    tagline: 'A menu of actions',
    order: 2,
    page: {
      lead: 'Use a dropdown menu for a short list of actions that do not need to stay on screen.',
      hero: 'Basic',
      variants: [
        { name: 'More', title: 'Icon only, at the end of a row' },
        { name: 'Value', title: 'Showing the current choice' },
        { name: 'Checkable', title: 'Checkbox and radio items' },
      ],
      sizes: [{ name: 'Sizes', title: 'Small, medium and large triggers' }],
      states: [
        { name: 'StateEnabled', title: 'Default' },
        { name: 'StateDisabled', title: 'Disabled' },
      ],
      inContext: [{ name: 'InList', title: 'A menu on every row' }],
    },
  },
};

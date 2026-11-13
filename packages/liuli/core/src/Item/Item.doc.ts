import type { ComponentDoc } from '../doc';

export const doc: ComponentDoc = {
  name: 'Item',
  category: 'content',
  status: 'preview',
  summary: 'One row of a list: a label, a second line, and content at either end',
  description:
    'Item is the row of a settings screen, an inbox or a file list. With `onClick` the whole row is one button that ' +
    'opens or selects something; without it the row is plain content and its `endContent` may hold its own control. ' +
    'Rows carry no border or fill, so the container decides how a list is grouped and divided.',
  keywords: ['item', 'row', 'list', 'cell', 'list item', 'settings row', 'table row', 'navigation row'],
  propsType: 'ItemProps',
  recipe: { map: 'itemRecipe', parts: 'itemParts' },
  anatomy: [
    { part: 'root', required: true, description: 'A `button` when `onClick` is set, otherwise a `view`; at least the large control height.' },
    { part: 'start', required: false, description: '`startContent`: an icon or avatar.' },
    { part: 'label', required: true, description: 'The main text; it wraps rather than truncating.' },
    { part: 'description', required: false, description: '`description`, or `disabledMessage` while disabled.' },
    { part: 'end', required: false, description: '`endContent`: a value, a badge, or a control on a plain row.' },
    { part: 'chevron', required: false, description: 'Shown with `hasChevron`; mirrors for right-to-left text.' },
  ],
  accessibility: {
    pattern: 'button',
    name: 'label',
    description: ['description, or disabledMessage while disabled'],
    states: [{ state: 'disabled', from: 'isDisabled' }],
    keyboard: [],
    target: { lg: 48 },
    contrast: [
      { what: 'label', pair: ['text', 'surface'], minimum: '4.5:1' },
      { what: 'second line and end values', pair: ['text-muted', 'surface'], minimum: '4.5:1' },
      { what: 'chevron', pair: ['text-faint', 'surface'], minimum: '3:1' },
    ],
    notes: [
      'The button pattern applies to a row with `onClick`. A plain row is not a control; its texts are read in order.',
      'On a pressable row the start and end content are drawn only, so the row is read as its label and second line.',
      'Every row is at least 48 px tall, above the 44 pt touch minimum.',
    ],
  },
  guidance: {
    do: [
      'Use `onClick` with `hasChevron` for a row that opens another page.',
      'Keep the label short and put detail in `description`.',
      'Group rows in a container with a border or fill and divide them (`divide-y divide-line`).',
    ],
    dont: [
      'Put a button or other control in `endContent` of a row with `onClick`; the View refuses a button inside a button.',
      'Use a row for a single action; that is a Button.',
      'Rely on the end value alone on a pressable row; it is not read, so repeat it in `description` when it matters.',
    ],
  },
  examples: [
    { name: 'Navigation', title: 'Rows that open a page' },
    { name: 'Values', title: 'Read-only rows with values' },
    { name: 'WithControl', title: 'A control at the end' },
    { name: 'Disabled', title: 'Disabled, with the reason' },
    { name: 'Destructive', title: 'A destructive row' },
  ],
  gaps: [],
  related: ['DropdownMenu', 'Card'],
  gallery: {
    specimen: 'Specimen',
    tagline: 'One row of a list',
    order: 2,
    page: {
      stage: 'bare',
      lead: 'Use an item for one row of a list: a setting, a message, a file.',
      hero: 'Navigation',
      variants: [
        { name: 'Values', title: 'Read-only, with a value at the end' },
        { name: 'WithControl', title: 'Holding its own control' },
        { name: 'Destructive', title: 'Danger tone, for a row that deletes' },
      ],
      states: [
        { name: 'StatePressable', title: 'Pressable' },
        { name: 'StateDisabled', title: 'Disabled' },
      ],
      inContext: [{ name: 'Disabled', title: 'Disabled, with the reason' }],
    },
  },
};

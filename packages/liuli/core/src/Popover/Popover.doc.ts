import type { ComponentDoc } from '../doc';

export const doc: ComponentDoc = {
  name: 'Popover',
  category: 'overlays',
  status: 'preview',
  summary: 'A button that opens a panel of content above or below it, closed by a tap outside',
  description:
    'Popover shows a small panel beside the button that opened it: view options, a share link, a short form. It is not ' +
    'modal; a tap outside, Escape or back closes it and focus goes back to the button. For a list of actions use ' +
    'DropdownMenu; for a note about something use Tooltip; for a decision use Dialog.',
  keywords: ['popover', 'popup', 'panel', 'flyout', 'options', 'share', 'overlay', 'anchored'],
  propsType: 'PopoverProps',
  recipe: { map: 'popoverRecipe', parts: 'popoverParts' },
  anatomy: [
    { part: 'panel', required: true, description: 'The raised card above or below the trigger, named by `title` or else `label`.' },
    { part: 'title', required: false, description: '`title`, drawn as a heading at the top of the panel.' },
  ],
  accessibility: {
    pattern: null,
    name: 'label',
    description: [],
    states: [{ state: 'expanded', from: 'whether the panel is open' }],
    keyboard: [
      { keys: 'Enter, Space', effect: 'On the trigger, opens or closes the panel.' },
      { keys: 'Escape', effect: 'Closes the panel; focus returns to the trigger.' },
    ],
    target: { sm: 32, md: 40, lg: 48 },
    contrast: [
      { what: 'title', pair: ['text', 'surface-overlay'], minimum: '4.5:1' },
      { what: 'panel edge', pair: ['line-strong', 'surface'], minimum: '3:1' },
    ],
    notes: [
      'The trigger reports expanded while the panel is open.',
      'A tap outside closes the panel and reaches nothing else, as with any light-dismiss popover.',
      'It opens below the trigger or above it only; near the bottom of the screen open it above, as it stays under the tab bar.',
    ],
  },
  guidance: {
    do: [
      'Keep the panel short: a title and a few controls or lines.',
      'Apply its controls at once; it has no Save.',
      'Open it upward when the trigger sits near the bottom of the screen.',
    ],
    dont: [
      'Put a task that must be finished in a popover; use a Dialog or a BottomSheet.',
      'Use it for a list of actions; that is a DropdownMenu.',
      'Open a popover from a popover.',
    ],
  },
  examples: [
    { name: 'ViewOptions', title: 'View options under the trigger' },
    { name: 'Share', title: 'A share link above the trigger' },
  ],
  gaps: [],
  related: ['DropdownMenu', 'Tooltip', 'Dialog'],
  gallery: {
    specimen: 'Specimen',
    tagline: 'A panel beside it',
    order: 5,
    page: {
      lead: 'Use a popover for a few controls or lines that belong to the button that opens them.',
      hero: 'ViewOptions',
      parts: { name: 'Parts', title: 'The trigger, then the panel with its title' },
      variants: [{ name: 'Share', title: 'Above, lined up with the trigger\'s end' }],
    },
  },
};

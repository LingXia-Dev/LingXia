import type { ComponentDoc } from '../doc';

export const doc: ComponentDoc = {
  name: 'Popover',
  category: 'overlays',
  status: 'preview',
  since: '2026-11-13',
  summary: 'A panel of content that opens beside its trigger, closed by a tap outside',
  description:
    'Popover shows a small panel, its `content`, beside the trigger that opened it: view options, a share link, a short form. Its children are the trigger, ' +
    'a Button or IconButton, or any element, which is wrapped in a plain button. The panel is a ' +
    'non-modal dialog: Tab can leave it, and a tap outside, Escape or back closes it and focus goes back to the trigger. For a list of actions use ' +
    'DropdownMenu; for a note about something use InfoTip; for a decision use Dialog.',
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
      { keys: 'Tab', effect: 'Moves through the panel\'s controls and on past it; the panel stays open.' },
      { keys: 'Escape', effect: 'Closes the panel; focus returns to the trigger.' },
    ],
    target: { sm: 32, md: 40, lg: 48 },
    contrast: [
      { what: 'title', pair: ['text', 'surface-overlay'], minimum: '4.5:1' },
      { what: 'panel edge', pair: ['line-strong', 'surface'], minimum: '3:1' },
    ],
    notes: [
      'The panel is a dialog named by `title`, else `label`; it is not modal, so the page stays readable around it.',
      'The trigger reports expanded while the panel is open.',
      'A tap outside closes the panel and reaches nothing else, as with any light-dismiss popover.',
      'It opens below the trigger or above it only; near the bottom of the screen open it above, as it stays under the tab bar.',
    ],
  },
  guidance: {
    do: [
      'Use a `Link` as the trigger for words in a sentence: put the overlay in the `<text>`, around the Link, and the panel opens from the linked words.',
      'Keep the panel short: a title and a few controls or lines.',
      'Apply its controls at once; it has no Save.',
      'Open it upward when the trigger sits near the bottom of the screen.',
    ],
    dont: [
      'Put a task that must be finished in a popover; use a Dialog or a BottomSheet.',
      'Use it for a list of actions; that is a DropdownMenu.',
      'Open a popover from a popover.',
      'Reach for `anchor` when a trigger cannot hold it, such as a chart mark; the caller then opens and closes it.',
    ],
  },
  examples: [
    { name: 'ViewOptions', title: 'View options under the trigger' },
    { name: 'Share', title: 'A share link above the trigger' },
    { name: 'Beside', title: 'Beside the trigger' },
    { name: 'Anchored', title: 'Against an element that is not its trigger' },
    { name: 'InSentence', title: 'On a link in a sentence' },
  ],
  gaps: [
    {
      missing: 'A panel beside its trigger, at the start or end side.',
      needs: 'side values for a popover\'s `positionArea` (`inline-end`, `inline-start`), flipping to the other side where there is no room',
      today: 'Above or below the trigger, lined up with its start, centre or end.',
    },
  ],
  playground: { props: { isOpen: false, label: 'View options', title: 'Show in the list' }, fixture: true },
  related: ['DropdownMenu', 'InfoTip', 'Dialog'],
  gallery: {
    specimen: 'Specimen',
    tagline: 'A panel beside it',
    order: 7,
    page: {
      lead: 'Use a popover for a few controls or lines that belong to the button that opens them.',
      hero: 'ViewOptions',
      parts: { name: 'Parts', title: 'The trigger, then the panel with its title' },
      variants: [{ name: 'Share', title: 'Above, lined up with the trigger\'s end' }, { name: 'Beside', title: 'At the side of the trigger; it flips or drops below when there is no room' }],
      inContext: [{ name: 'InSentence', title: 'On a link in a sentence' }],
    },
  },
};

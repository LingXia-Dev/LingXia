import type { ComponentDoc } from '../doc';

export const doc: ComponentDoc = {
  name: 'InfoTip',
  category: 'overlays',
  status: 'preview',
  since: '2026-11-12',
  summary: 'An info button that opens a short note about what is beside it',
  description:
    'InfoTip explains something the screen cannot say in place: why a limit applies, what a term means. ' +
    'It opens on a press (a tap, Enter or Space), not on hover or focus, so it works the same with touch, mouse and keyboard. To name an icon button in words, use Tooltip.',
  keywords: ['infotip', 'toggletip', 'tooltip', 'info', 'help', 'hint', 'explain', 'popover', 'note'],
  propsType: 'InfoTipProps',
  recipe: { map: 'infoTipRecipe', parts: 'infoTipParts' },
  anatomy: [
    { part: 'trigger', required: true, description: 'The info button, named by `label`; it reports whether the note is open.' },
    { part: 'icon', required: true, description: 'The info glyph.' },
    { part: 'bubble', required: true, description: 'The note: a popover with the tooltip role beside the button, in the inverse of the page.' },
    { part: 'text', required: true, description: 'The note\'s text; the button is described by it while open.' },
  ],
  accessibility: {
    pattern: 'button',
    name: 'label',
    description: ['the note, while open'],
    states: [{ state: 'expanded', from: 'isOpen, or a press' }],
    keyboard: [
      { keys: 'Enter, Space', effect: 'Opens or closes the note; focus alone does not open it.' },
      { keys: 'Escape', effect: 'Closes it.' },
    ],
    target: { sm: 32, md: 40 },
    contrast: [
      { what: 'note text', pair: ['surface', 'text'], minimum: '4.5:1' },
      { what: 'info glyph', pair: ['text-muted', 'surface'], minimum: '3:1' },
    ],
    notes: [
      'A tap anywhere else closes the note, and that tap reaches nothing else.',
      'Put only words in the note; anything to press belongs in a DropdownMenu or a Dialog.',
    ],
  },
  guidance: {
    do: [
      'Name the button for its subject: "About storage".',
      'Keep the note to one or two short sentences.',
    ],
    dont: [
      'Hide what someone needs to finish a task in a note; write it on the page.',
      'Give every label a note; add one only where people get stuck.',
    ],
  },
  examples: [
    { name: 'Storage', title: 'Beside the thing it explains' },
    { name: 'Below', title: 'Opening below' },
    { name: 'InForm', title: 'Next to a field label' },
    { name: 'Sizes', title: 'Sizes' },
    { name: 'Beside', title: 'Beside the button' },
  ],
  gaps: [
    {
      missing: 'A note beside its button, at the start or end side.',
      needs: 'side values for a popover\'s `positionArea` (`inline-end`, `inline-start`)',
      today: 'Above or below the button.',
    },
    {
      missing: 'A live trigger shown pressed in the gallery.',
      needs: 'a way to hold a view in a forced state (pressed, open) for display',
      today: 'The pressed and open pictures are still drawings.',
    },
  ],
  playground: { props: { isOpen: false, label: 'About seats', content: 'A seat is one person who can edit. People who only read are free.' } },
  related: ['Tooltip', 'DropdownMenu', 'Dialog'],
  gallery: {
    specimen: 'Specimen',
    tagline: 'A note on demand',
    order: 4,
    page: {
      lead: 'Use an info tip for a short note that explains what is beside it. Tap the info button to open it.',
      hero: 'Storage',
      parts: { name: 'Parts', title: 'The note over its button' },
      variants: [{ name: 'Below', title: 'Opening below, where there is no room above' }, { name: 'Beside', title: 'At the side of the button' }],
      states: [
        { name: 'StateClosed', title: 'Closed' },
        { name: 'StatePressed', title: 'Pressed' },
        { name: 'StateOpen', title: 'Open' },
      ],
      sizes: [{ name: 'Sizes', title: 'Small and medium' }],
      inContext: [{ name: 'InForm', title: 'Next to a field label' }],
    },
  },
};

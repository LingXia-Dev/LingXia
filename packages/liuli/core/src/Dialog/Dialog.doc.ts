import type { ComponentDoc } from '../doc';

export const doc: ComponentDoc = {
  name: 'Dialog',
  category: 'overlays',
  status: 'preview',
  since: '2026-11-12',
  summary: 'Modal panel over the page for a decision or a short task',
  description:
    'Dialog interrupts the page to ask for a decision or a short piece of input, and keeps focus and assistive tech inside until it closes. ' +
    'It is controlled: `isOpen` shows it, `onOpenChange(false)` reports every close request.',
  keywords: ['dialog', 'modal', 'confirm', 'alert', 'popup', 'overlay', 'prompt'],
  propsType: 'DialogProps',
  recipe: { map: 'dialogRecipe', parts: 'dialogParts' },
  anatomy: [
    { part: 'panel', required: true, description: 'The dialog primitive: a centred panel on the overlay surface, in the top layer above a scrim; at `full` it covers the screen on the page\'s canvas.' },
    { part: 'media', required: false, description: '`media`: a picture above the title, reaching the panel\'s top edges; hidden from assistive tech.' },
    { part: 'mediaClose', required: false, description: 'With `media`, the close button sits over the picture on its own dock.' },
    { part: 'title', required: true, description: '`label` as a level-2 heading, unless `isLabelHidden`.' },
    { part: 'close', required: false, description: 'A close button: an alert has none unless you ask for one; any other dialog has one unless you turn it off. `closeLabel` names it.' },
    { part: 'description', required: false, description: 'Text under the title.' },
    { part: 'body', required: false, description: '`children`, in a scroll that takes the remaining height. A hairline shows at an edge while content is scrolled under it.' },
    { part: 'headerAction', required: false, description: '`headerAction`: one small control beside the title that acts on the whole panel, such as Refresh.' },
    { part: 'footer', required: false, description: '`actions`: on a phone they share the width equally, the main one last; wider, they keep their own widths at the end, wrapping when narrow.' },
  ],
  accessibility: {
    pattern: 'dialog',
    name: 'label',
    description: ['description'],
    states: [{ state: 'modal', from: 'isOpen; announced as an alert dialog with isAlert' }],
    keyboard: [{ keys: 'Escape', effect: 'onOpenChange(false), unless dismissOn is none' }],
    contrast: [
      { what: 'title', pair: ['text', 'surface-overlay'], minimum: '4.5:1' },
      { what: 'close button over a picture', pair: ['on-media', 'media'], minimum: '3:1' },
      { what: 'description', pair: ['text-muted', 'surface-overlay'], minimum: '4.5:1' },
      { what: 'title, full screen', pair: ['text', 'canvas'], minimum: '4.5:1' },
      { what: 'description, full screen', pair: ['text-muted', 'canvas'], minimum: '4.5:1' },
    ],
    notes: [
      'Opened from a hardware keyboard, focus moves to the first control and returns to the opener on close; opened by touch, focus stays put.',
      'Android back and the screen reader dismiss gesture are close requests, like Escape.',
      'The close button is 32 px; a finger reaches it in a 44 pt band around it. Offer an action button too, so closing is never the only way on.',
    ],
  },
  guidance: {
    do: [
      'Title it with the question or the task ("Delete this note?").',
      'Name the actions by their outcome; put the main one last.',
      'Keep a draft in state declared outside the dialog.',
    ],
    dont: [
      'Open a dialog from a dialog; finish or close the first.',
      'Put a long scrolling list in a dialog; use a page.',
      'Place it inside a Button; overlays cannot sit in a pressable control.',
    ],
  },
  examples: [
    { name: 'Confirm', title: 'Confirm a destructive action, as an alert dialog' },
    { name: 'Form', title: 'A short form' },
    { name: 'Full', title: 'Full screen' },
    { name: 'Hero', title: 'With a picture above the title' },
  ],
  gaps: [
    {
      missing: 'A theme-wide motion token for the enter and exit timing.',
      needs: 'duration and easing utilities that read the theme pack\'s motion tokens',
      today: 'duration-200 ease-out in the recipe.',
    },
  ],
  playground: { props: { isOpen: false, label: 'Delete the file?', description: 'It goes to the bin for 30 days.' }, fixture: true, opener: 'Delete the file' },
  related: ['Button', 'DropdownMenu'],
  gallery: {
    specimen: 'Specimen',
    tagline: 'A decision, on top',
    order: 1,
    page: {
      lead: 'Use a dialog for a decision or a short task that must be finished before going on.',
      hero: 'Confirm',
      parts: { name: 'Parts', title: 'Title, then the body, then the actions with the main one last' },
      variants: [{ name: 'Form', title: 'A short task: renaming' }, { name: 'Full', title: 'Full screen, for a task with its own content' }, { name: 'Hero', title: 'A picture above the title, for a dialog that introduces something' }],
    },
  },
};

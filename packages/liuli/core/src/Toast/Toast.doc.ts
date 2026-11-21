import type { ComponentDoc } from '../doc';

export const doc: ComponentDoc = {
  name: 'Toast',
  category: 'feedback',
  status: 'preview',
  since: '2026-11-13',
  summary: 'A short message about something that just happened, over the bottom of the screen',
  description:
    'A toast confirms an action that worked away from where it was pressed, or offers to undo it: a note archived, a link ' +
    'copied. A page holds one `ToastRegion`; `useToast()` shows a toast from anywhere inside it. One shows at a time and ' +
    'the rest wait in order; a toast with the same `id` replaces the one queued, so a running count updates in place. ' +
    'One without an action closes by itself after `duration`; one with an action stays until it is closed.',
  keywords: ['toast', 'snackbar', 'notification', 'undo', 'message', 'confirmation', 'feedback', 'status'],
  propsType: 'ToastProps',
  recipe: { map: 'toastRecipe', parts: 'toastParts' },
  anatomy: [
    { part: 'region', required: true, description: 'The top-layer box the toast sits in, clear of the system bars and the keyboard.' },
    { part: 'toast', required: true, description: 'The raised card; a sideways swipe closes it.' },
    { part: 'icon', required: false, description: 'The tone\'s glyph, hidden from assistive tech; neutral toasts have none.' },
    { part: 'message', required: true, description: 'What happened, in a few words.' },
    { part: 'action', required: false, description: 'One link button that follows up, such as Undo; the toast closes after it.' },
    { part: 'dismiss', required: true, description: 'A close button named by `dismissLabel`.' },
    { part: 'announcer', required: true, description: 'Hidden live texts, polite and assertive pairs, mounted with the region; each new toast takes the one of its pair cleared last, so a repeated message is spoken again.' },
  ],
  accessibility: {
    pattern: null,
    name: null,
    description: [],
    states: [],
    keyboard: [
      { keys: 'Tab', effect: 'Reaches the action and the dismiss button after the page.' },
      { keys: 'Escape', effect: 'Nothing on the toast; it stays with what has focus.' },
    ],
    target: { md: 36 },
    contrast: [
      { what: 'message', pair: ['text', 'surface-overlay'], minimum: '4.5:1' },
      { what: 'dismiss icon', pair: ['text-muted', 'surface-overlay'], minimum: '3:1' },
      { what: 'action', pair: ['accent', 'surface-overlay'], minimum: '4.5:1' },
    ],
    notes: [
      'Each new message is announced politely; a danger toast is announced assertively.',
      'The announcement comes from live text that lives as long as the region, as a region inserted with its message is not announced.',
      'The tone is in the glyph and the words, never in colour alone.',
      'A toast without an action closes after `duration`; the count pauses while it is pressed, under a resting pointer or holding focus. A toast with an action stays until closed.',
      'A toast is not modal: Escape and back go to whatever has focus, never to the toast. The dismiss button is its keyboard path.',
    ],
  },
  guidance: {
    do: [
      'Say what happened in a few words: "Note archived", not "Success".',
      'Offer Undo for an action that is easy to regret.',
      'Give a running message an `id`, so each update replaces it.',
    ],
    dont: [
      'Put something people must read or act on in a toast; use a Banner or a Dialog.',
      'Show a toast for the result people can already see where they pressed.',
      'Use a toast for an error that stops work; show it where the work is.',
    ],
  },
  examples: [
    { name: 'Archive', title: 'Archive with Undo' },
    { name: 'Tones', title: 'Tones, queued' },
    { name: 'Progress', title: 'Updated in place' },
    { name: 'Faces', title: 'Each face, at phone width' },
  ],
  gaps: [],
  playground: { props: { message: 'Changes saved', tone: 'success' } },
  related: ['Banner', 'Dialog'],
  gallery: {
    specimen: 'Specimen',
    tagline: 'Just happened',
    order: 6,
    page: {
      lead: 'Use a toast to confirm an action that worked, or to offer to undo it.',
      hero: 'Archive',
      variants: [
        { name: 'Tones', title: 'Info, success, warning and danger, one at a time' },
        { name: 'Progress', title: 'A count updated in place' },
        { name: 'Faces', title: 'A message, success, with Undo, and danger with Retry' },
      ],
      inContext: [],
    },
  },
};

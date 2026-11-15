import type { ComponentDoc } from '../doc';

export const doc: ComponentDoc = {
  name: 'MessageBubble',
  category: 'content',
  status: 'preview',
  since: '2026-11-15',
  summary: 'One message of a conversation',
  description:
    'MessageBubble draws one message: the user\'s own at the end edge in the accent\'s soft step, anyone else\'s at the start on a quiet tint, with a name and a face over the first of a run. ' +
    'Under it goes a line the app writes, the time or how the message stands, and for one that failed a way to send it again. ' +
    'A conversation is the View\'s list of these with a Composer under it; the Conversation pattern shows them together.',
  keywords: ['message', 'bubble', 'chat', 'conversation', 'comment', 'reply'],
  propsType: 'MessageBubbleProps',
  recipe: { map: 'messageBubbleRecipe', parts: 'messageBubbleParts' },
  anatomy: [
    { part: 'root', required: true, description: 'The row: it puts the bubble at its side.' },
    { part: 'avatar', required: false, description: 'The writer\'s Avatar, or its width kept empty down a run.' },
    { part: 'author', required: false, description: 'Who wrote it, over the first bubble of a run.' },
    { part: 'bubble', required: true, description: 'The message: up to four fifths of the row, 520 px at most.' },
    { part: 'meta', required: false, description: 'The line under the bubble: time, state, and the retry action.' },
  ],
  accessibility: {
    pattern: null,
    name: null,
    description: [],
    states: [],
    keyboard: [],
    target: {},
    contrast: [
      { what: 'my message', pair: ['on-accent-soft', 'accent-soft'], minimum: '4.5:1' },
      { what: 'their message', pair: ['text', 'text/6'], over: 'canvas', minimum: '4.5:1' },
      { what: 'meta', pair: ['text-muted', 'canvas'], minimum: '4.5:1' },
      { what: 'failure words', pair: ['danger', 'canvas'], minimum: '4.5:1' },
    ],
    notes: [
      'It is read in order: the name, the message, then the line under it. Put the list in the View\'s list so a screen reader steps message by message.',
      'A failed message says so in words and with an error mark, not only with its red edge.',
      'The side tells whose it is to the eye; say it in `author` for the ear, or name the list by who is speaking.',
    ],
  },
  guidance: {
    do: [
      'Write `meta` in full: "Not sent", "Sending", a time.',
      'Join the bubbles of one person\'s run and name the run once.',
    ],
    dont: [
      'Colour bubbles per person; side and name tell them apart.',
      'Put a whole form in a bubble; a bubble holds a message, a picture or a file.',
    ],
  },
  examples: [
    { name: 'Exchange', title: 'A short exchange' },
    { name: 'States', title: 'Sending, sent and not sent' },
  ],
  gaps: [],
  limits: [
    'No reactions, threads or read receipts.',
    'No typing mark.',
  ],
  related: ['Composer', 'Avatar', 'Markdown'],
  gallery: {
    specimen: 'Specimen',
    tagline: 'A message',
    order: 16,
    page: {
      lead: 'Use a message bubble for each message of a conversation.',
      hero: 'Exchange',
      variants: [{ name: 'States', title: 'On its way, sent, and not sent with a way to try again' }],
    },
  },
};

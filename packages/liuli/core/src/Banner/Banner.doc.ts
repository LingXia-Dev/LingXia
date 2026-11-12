import type { ComponentDoc } from '../doc';

export const doc: ComponentDoc = {
  name: 'Banner',
  category: 'feedback',
  status: 'preview',
  summary: 'A message in the page about the page or the account, that stays until resolved',
  description:
    'Banner tells people about something that affects what they are looking at: sync stopped, a card expiring, a notebook ' +
    'locked. It sits in the page\'s flow, above what it is about, with at most one action and an optional dismiss button. ' +
    'It is not a toast: it does not time out, and the app removes it when the condition ends or it is dismissed.',
  keywords: ['banner', 'alert', 'callout', 'notice', 'message', 'status', 'warning', 'error', 'inline'],
  propsType: 'BannerProps',
  recipe: { map: 'bannerRecipe', parts: 'bannerParts' },
  anatomy: [
    { part: 'root', required: true, description: 'The tinted box, in the tone\'s soft fill with a hairline of the tone.' },
    { part: 'icon', required: true, description: 'The tone\'s glyph, hidden from assistive tech; the tone is never carried by colour alone.' },
    { part: 'text', required: true, description: 'The column holding title, message and action.' },
    { part: 'title', required: true, description: 'The point, in a few words.' },
    { part: 'message', required: false, description: 'What it means or what to do, in a sentence.' },
    { part: 'action', required: false, description: 'One link button that resolves it.' },
    { part: 'dismiss', required: false, description: 'A close button named by `dismissLabel`.' },
  ],
  accessibility: {
    pattern: null,
    name: null,
    description: [],
    states: [],
    keyboard: [],
    contrast: [
      { what: 'title and message on info', pair: ['text', 'accent-soft'], minimum: '4.5:1', proven: false },
      { what: 'title and message on success', pair: ['text', 'success-soft'], minimum: '4.5:1', proven: false },
      { what: 'title and message on warning', pair: ['text', 'warning-soft'], minimum: '4.5:1', proven: false },
      { what: 'title and message on danger', pair: ['text', 'danger-soft'], minimum: '4.5:1', proven: false },
    ],
    notes: [
      'Role alert for danger, so it is announced when it appears; role status for the other tones, announced politely.',
      'Render it only when the condition holds; announcing needs it to appear, not to be there from the start.',
      'The text stays in the text role on every tint; the icon carries the tone.',
    ],
  },
  guidance: {
    do: [
      'Put it above the content it is about, and say what to do in `message` or `action`.',
      'Use danger only when something has stopped working.',
      'Let people dismiss a banner that is news; keep one that blocks work until it is resolved.',
    ],
    dont: [
      'Stack several banners; show the most urgent one.',
      'Use a banner to confirm a small action that just worked; the control itself can show that.',
      'Give it more than one action.',
    ],
  },
  examples: [
    { name: 'Payment', title: 'A problem with one action, dismissible' },
    { name: 'Tones', title: 'Tones' },
    { name: 'OverList', title: 'Above the list it is about' },
  ],
  gaps: [],
  related: ['Badge', 'Dialog', 'ProgressBar'],
  gallery: {
    specimen: 'Specimen',
    tagline: 'Stays until resolved',
    order: 3,
    page: {
      lead: 'Use a banner to say something about the page or the account that people should act on or know.',
      hero: 'Payment',
      variants: [{ name: 'Tones', title: 'Info, success, warning and danger' }],
      states: [
        { name: 'StateTitleOnly', title: 'A title only' },
        { name: 'StateWithMessage', title: 'With a message' },
        { name: 'StateWithAction', title: 'With an action' },
        { name: 'StateDismissible', title: 'Dismissible' },
      ],
      inContext: [{ name: 'OverList', title: 'Above the list it is about' }],
    },
  },
};

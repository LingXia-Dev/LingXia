import type { ComponentDoc } from '../doc';

export const doc: ComponentDoc = {
  name: 'Banner',
  category: 'feedback',
  status: 'preview',
  since: '2026-11-12',
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
      { what: 'title and message on neutral', pair: ['text', 'text/6'], over: 'canvas', minimum: '4.5:1' },
      { what: 'neutral icon', pair: ['text-muted', 'text/6'], over: 'canvas', minimum: '3:1' },
      { what: 'title and message on info', pair: ['text', 'info-soft'], minimum: '4.5:1' },
      { what: 'title and message on success', pair: ['text', 'success-soft'], minimum: '4.5:1' },
      { what: 'title and message on warning', pair: ['text', 'warning-soft'], minimum: '4.5:1' },
      { what: 'title and message on danger', pair: ['text', 'danger-soft'], minimum: '4.5:1' },
      { what: 'action on neutral', pair: ['accent', 'text/6'], over: 'canvas', minimum: '4.5:1' },
      { what: 'action on info', pair: ['accent', 'info-soft'], minimum: '4.5:1' },
      { what: 'action on success', pair: ['accent', 'success-soft'], minimum: '4.5:1' },
      { what: 'action on warning', pair: ['accent', 'warning-soft'], minimum: '4.5:1' },
      { what: 'action on danger', pair: ['accent', 'danger-soft'], minimum: '4.5:1' },
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
      'Use neutral for a plain notice, and danger only when something has stopped working.',
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
    { name: 'Collapsible', title: 'Folded to its title' },
  ],
  gaps: [],
  related: ['Badge', 'Dialog', 'ProgressBar'],
  playground: { props: { title: 'Storage almost full', message: 'You have used 9.6 of 10 GB.', tone: 'warning' } },
  gallery: {
    specimen: 'Specimen',
    tagline: 'Stays until fixed',
    order: 3,
    page: {
      lead: 'Use a banner to say something about the page or the account that people should act on or know.',
      hero: 'Payment',
      variants: [{ name: 'Tones', title: 'Neutral, info, success, warning and danger' }, { name: 'Collapsible', title: 'Collapsible: the title stays, the rest opens on demand' }],
      statesLayout: 'stack',
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

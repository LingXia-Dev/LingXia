import type { ComponentDoc } from '../doc';

export const doc: ComponentDoc = {
  name: 'HoverCard',
  category: 'overlays',
  status: 'preview',
  since: '2026-11-15',
  summary: 'A preview of what a word names',
  description:
    'HoverCard shows a small card about the thing its trigger names, a person, a term, a reference, while a pointer rests on the trigger or keyboard focus is on it. Where there is no pointer, a tap shows it. ' +
    'It leaves by itself, so it holds nothing people must reach: for controls or a short form, use a Popover.',
  keywords: ['hover card', 'preview', 'peek', 'profile card', 'definition', 'hint'],
  propsType: 'HoverCardProps',
  recipe: { map: 'hoverCardRecipe', parts: 'hoverCardParts' },
  anatomy: [
    { part: 'trigger', required: true, description: 'A plain button around the trigger the author gives: words under a dotted line that says there is more and is not a link.' },
    { part: 'card', required: true, description: 'A hint popover anchored to the trigger, in the top layer.' },
  ],
  accessibility: {
    pattern: null,
    name: 'label',
    description: [],
    states: [],
    keyboard: [
      { keys: 'Tab', effect: 'Moves to the trigger and shows the card' },
      { keys: 'Escape', effect: 'Hides the card' },
    ],
    target: {},
    contrast: [
      { what: 'trigger', pair: ['text', 'surface'], minimum: '4.5:1' },
      { what: 'card text', pair: ['text', 'surface-overlay'], minimum: '4.5:1' },
    ],
    notes: [
      'Words as the trigger are wrapped in one button named by them; the card is named by `label`.',
      'The card shows on keyboard focus as well as on hover, and on a tap on touch screens.',
      'Focus does not move into the card, so put nothing in it that needs focus.',
    ],
  },
  guidance: {
    do: [
      'Use a `Link` as the trigger for words in a sentence: put the overlay in the `<text>`, around the Link, and the panel opens from the linked words.',
      'Keep the card to a few lines that add to the trigger: who, what, a status.',
      'Make sure everything in the card can also be reached another way, such as the page the trigger stands for.',
    ],
    dont: [
      'Put buttons, links or fields in the card; it goes away when the pointer leaves.',
      'Use it for help about a control; that is an InfoTip.',
    ],
  },
  examples: [
    { name: 'Person', title: 'A person, from their name' },
    { name: 'Term', title: 'A term, opening beside it' },
    { name: 'InSentence', title: 'On a link in a sentence' },
  ],
  gaps: [],
  limits: [
    'The trigger is its own row or wrapped word: it cannot sit inside a sentence as a run of text.',
  ],
  related: ['Popover', 'Tooltip', 'Link'],
  gallery: {
    specimen: 'Specimen',
    tagline: 'More, on rest',
    order: 8,
    page: {
      lead: 'Use a hover card to preview what a word names without leaving the page.',
      hero: 'Person',
      variants: [{ name: 'Term', title: 'Beside the trigger, for a term in small text' }],
      inContext: [{ name: 'InSentence', title: 'On a link in a sentence' }],
    },
  },
};

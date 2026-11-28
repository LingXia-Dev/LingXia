import type { ComponentDoc } from '../doc';

export const doc: ComponentDoc = {
  name: 'Citation',
  category: 'content',
  status: 'preview',
  since: '2026-11-27',
  summary: 'Where an answer\'s claims come from',
  description:
    'CitationGroup holds an answer and the sources it rests on. A Citation puts a source\'s number after the claim it backs, among the words of a `<text>`; ' +
    'resting a pointer on it, focusing it or tapping it shows the source\'s card: its title, where it is from and the words it was cited for. ' +
    'Under the answer the sources sit as numbered pills, and a pill opens its source through `onOpen`. Numbers follow the order of `sources`, so the same source keeps its number wherever it is cited.',
  keywords: ['citation', 'source', 'reference', 'footnote', 'grounding', 'cite', 'assistant', 'answer', 'link'],
  propsType: 'CitationGroupProps',
  otherPropsTypes: ['CitationProps', 'CitationSource'],
  recipe: { map: 'citationRecipe', parts: 'citationParts' },
  anatomy: [
    { part: 'mark', required: true, description: 'The source\'s number among the words, on the accent\'s soft step; it shows the card.' },
    { part: 'sources', required: false, description: 'The sources under the answer: a group named "Sources".' },
    { part: 'source', required: false, description: 'One source\'s pill: its number and its site, else its title; it opens the source.' },
    { part: 'card', required: false, description: 'The source\'s card, shown from its mark: title, site and the cited words.' },
  ],
  accessibility: {
    pattern: null,
    name: 'label',
    description: [],
    states: [],
    keyboard: [
      { keys: 'Tab', effect: 'To each mark among the words, then each source\'s pill' },
      { keys: 'Enter', effect: 'On a mark, shows its card; on a pill, opens the source' },
    ],
    target: {},
    contrast: [
      { what: 'mark', pair: ['accent', 'surface'], minimum: '4.5:1' },
      { what: 'pill number', pair: ['on-accent-soft', 'accent-soft'], minimum: '4.5:1' },
      { what: 'pill words', pair: ['text-muted', 'surface'], minimum: '4.5:1' },
    ],
    notes: [
      'A mark reads its number and is described by its card, so a screen reader hears which source backs the claim without opening anything.',
      'Each pill is named "Source 1: its title, its site".',
    ],
  },
  guidance: {
    do: [
      'Put the mark straight after the claim it backs, before the full stop.',
      'Give each source an `excerpt`: the words it was cited for, so the card shows why.',
    ],
    dont: [
      'Cite a source no claim uses; leave it out of `sources`.',
      'Use it for links the user is meant to follow as part of a task; that is a Link.',
    ],
  },
  examples: [
    { name: 'Answer', title: 'An assistant\'s answer citing three sources' },
  ],
  gaps: [],
  limits: [
    'The card shows text only; a picture of the page is not drawn.',
    'Marks are numbers; a source named in the words is a Link.',
  ],
  related: ['MessageBubble', 'Markdown', 'Link', 'HoverCard'],
  gallery: {
    specimen: 'Specimen',
    tagline: 'An answer\'s sources',
    order: 22,
    page: {
      lead: 'Use citations to show which source backs each claim of an answer.',
      hero: 'Answer',
      variants: [],
    },
  },
};

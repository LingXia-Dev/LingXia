import type { ComponentDoc } from '../doc';

export const doc: ComponentDoc = {
  name: 'Link',
  category: 'navigation',
  status: 'preview',
  since: '2026-11-15',
  summary: 'A link among the words of a sentence',
  description:
    'Link is a run of text that goes somewhere: it sits inside a `<text>`, takes the size and weight of the words around it, and is told apart by the accent colour and an underline. ' +
    'It calls `onClick`, where the app opens the page or the address, or goes to a page of the app itself with `to`. A link that stands alone, outside a sentence, is a Button with `variant="link"`.',
  keywords: ['link', 'anchor', 'hyperlink', 'inline', 'text'],
  propsType: 'LinkProps',
  recipe: { map: 'linkRecipe', parts: 'linkParts' },
  anatomy: [{ part: 'root', required: true, description: 'The linked words, underlined in the accent colour.' }],
  accessibility: {
    pattern: null,
    name: 'label',
    description: [],
    states: [{ state: 'disabled', from: 'isDisabled' }],
    keyboard: [
      { keys: 'Tab', effect: 'Moves to the link' },
      { keys: 'Enter', effect: 'Follows it' },
    ],
    target: {},
    contrast: [
      { what: 'link', pair: ['accent', 'surface'], minimum: '4.5:1' },
      { what: 'link on the page', pair: ['accent', 'canvas'], minimum: '4.5:1' },
    ],
    notes: [
      'It is read as a link, by its label. Write the label so it makes sense out of its sentence: "the March invoice", not "here".',
      'The underline tells it from the text around it without colour.',
      'A link in a line of text is as tall as the line; a finger press that misses goes to the nearest link within reach.',
      'A disabled link is read as unavailable and takes no press.',
    ],
  },
  guidance: {
    do: [
      'To open a Popover, DropdownMenu, HoverCard or Tooltip from words in a sentence, wrap the Link in it, inside the `<text>`.',
      'Link the words that name the destination: "terms of service".',
      'Keep a link to a few words; a whole sentence underlined is hard to read.',
    ],
    dont: [
      'Link "here" or "click here".',
      'Use a link for an action that changes something; that is a Button.',
      'Set a link apart on its own line; use Button with `variant="link"`.',
    ],
  },
  examples: [
    { name: 'InSentence', title: 'In a sentence' },
    { name: 'InBody', title: 'In body text' },
    { name: 'Disabled', title: 'Not available yet' },
    { name: 'ToPage', title: 'Going to a page of the app' },
  ],
  gaps: [],
  limits: [
    'A link has no hover or focus style of its own beyond the system focus ring.',
  ],
  related: ['Button', 'Breadcrumb', 'Markdown'],
  gallery: {
    specimen: 'Specimen',
    tagline: 'Words that lead on',
    order: 6,
    page: {
      lead: 'Use a link for words in a sentence that lead to another page.',
      hero: 'InSentence',
      variants: [{ name: 'InBody', title: 'At the size of the text around it' }],
      statesLayout: 'stack',
      states: [{ name: 'Disabled', title: 'Disabled' }],
    },
  },
};

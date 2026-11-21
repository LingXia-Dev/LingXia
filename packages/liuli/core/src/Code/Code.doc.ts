import type { ComponentDoc } from '../doc';

export const doc: ComponentDoc = {
  name: 'Code',
  category: 'content',
  status: 'preview',
  since: '2026-11-15',
  summary: 'Code as it was written, in a sentence or as a block',
  description:
    'Code is a run inside a sentence, in the fixed-width face on a tint: a file name, a key, a value to type exactly. ' +
    'CodeBlock keeps lines of code as they were written: it never wraps a line, it scrolls sideways, and it can carry a title and one action, usually copy. ' +
    'It can number its lines, mark some of them, and show a change as a unified diff. Neither colours the code by language.',
  keywords: ['code', 'code block', 'snippet', 'monospace', 'pre', 'terminal', 'command'],
  propsType: 'CodeBlockProps',
  recipe: { map: 'codeRecipe', parts: 'codeParts' },
  anatomy: [
    { part: 'inline', required: false, description: 'Code: a fixed-width run on a tint, inside a `<text>`.' },
    { part: 'block', required: false, description: 'CodeBlock: a tinted box, a group named by `label`.' },
    { part: 'header', required: false, description: 'The title and the action, over a hairline.' },
    { part: 'title', required: false, description: 'A file name or a language.' },
    { part: 'scroll', required: false, description: 'The sideways scroll around the code.' },
    { part: 'tint', required: false, description: 'A row behind one line: marked, added, removed, or a diff\'s header. In a diff a mark tints only an unchanged line.' },
    { part: 'gutter', required: false, description: 'The line numbers, before a hairline.' },
    { part: 'signs', required: false, description: 'In a diff, each line\'s `+`, `-` or space, in its own column.' },
    { part: 'text', required: false, description: 'The code, fixed-width, with its own line breaks.' },
  ],
  accessibility: {
    pattern: null,
    name: 'label',
    description: [],
    states: [],
    keyboard: [],
    target: {},
    contrast: [
      { what: 'code on its tint', pair: ['text', 'text/8'], over: 'surface', minimum: '4.5:1' },
      { what: 'title and line numbers', pair: ['text-muted', 'text/6'], over: 'surface', minimum: '4.5:1' },
      { what: 'code on an added line', pair: ['text', 'success-soft'], minimum: '4.5:1' },
      { what: 'code on a removed line', pair: ['text', 'danger-soft'], minimum: '4.5:1' },
      { what: 'code on a marked line', pair: ['text', 'accent-soft'], minimum: '4.5:1' },
      { what: 'a sign and number on an added line', pair: ['success', 'success-soft'], minimum: '4.5:1' },
      { what: 'a sign and number on a removed line', pair: ['danger', 'danger-soft'], minimum: '4.5:1' },
      { what: 'a number on a marked line', pair: ['text-muted', 'accent-soft'], minimum: '4.5:1' },
      { what: 'a hunk or file header on its tint, over the block', pair: ['text-muted', 'text/12'], over: 'surface', minimum: '4.5:1' },
    ],
    notes: [
      'A block is a group named by `label`; its code is read as text, line by line.',
      'The action is the app\'s own Button or IconButton, with its own name.',
      'Line numbers are not read; a diff\'s signs are, so a change reads without its colour.',
      'A marked line is marked by its tint alone; say in the text around the block why it is marked.',
      'Inline code is read with its sentence; nothing announces the change of face.',
    ],
  },
  guidance: {
    do: [
      'Use Code for what must be typed exactly; use plain emphasis for a term.',
      'Give a block a copy action when people will paste it somewhere.',
    ],
    dont: [
      'Put prose in a block to set it apart; that is a Card or a Banner.',
      'Break a long line by hand to make it fit; it scrolls.',
    ],
  },
  examples: [
    { name: 'Command', title: 'A command to copy' },
    { name: 'Plain', title: 'A block without a header' },
    { name: 'Numbered', title: 'Line numbers and marked lines' },
    { name: 'Diff', title: 'A change as a diff' },
    { name: 'InSentence', title: 'In a sentence' },
  ],
  gaps: [],
  limits: [
    'No syntax colouring: the page carries no grammar for any language.',
    'With numbers, a diff or marked lines, a line is never wrapped, even at large text; it scrolls.',
    'Copying is the app\'s: Liuli has no clipboard.',
  ],
  playground: { props: { code: 'npm run build', label: 'Build command', title: 'Terminal' } },
  related: ['Markdown', 'Kbd'],
  gallery: {
    specimen: 'Specimen',
    tagline: 'Code, as written',
    order: 14,
    page: {
      lead: 'Use code for text that must be read or typed exactly.',
      hero: 'Command',
      variants: [
        { name: 'Plain', title: 'No header; long lines scroll sideways' },
        { name: 'Numbered', title: 'Line numbers from 41, two lines marked' },
        { name: 'Diff', title: 'A change, with signs and tints' },
        { name: 'InSentence', title: 'Code, inside a sentence' },
      ],
    },
  },
};

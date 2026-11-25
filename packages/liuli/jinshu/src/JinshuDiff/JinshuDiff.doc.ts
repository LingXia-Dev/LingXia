import type { ComponentDoc } from '@liulidesign/core/internal/doc';

export const doc: ComponentDoc = {
  name: 'JinshuDiff',
  category: 'documents',
  status: 'preview',
  since: '2026-11-24',
  summary: 'What changed between two versions of a Jinshu document, block by block, with words struck and inserted',
  description:
    'JinshuDiff compares two versions of a document and shows what changed: blocks added, removed, moved and modified, each in a ' +
    'card tinted by its kind, with the unchanged runs between them folded. A summary counts each kind and steps between the changes. ' +
    'A modified block shows its words struck and inserted in place, or the whole block before and after when the edit is too heavy ' +
    'to read word by word; `mode` chooses.',
  keywords: ['diff', 'compare', 'changes', 'revision', 'version history', 'track changes', 'redline', 'jinshu'],
  propsType: 'JinshuDiffProps',
  recipe: { map: 'jinshuDiffRecipe', parts: 'jinshuDiffParts' },
  anatomy: [
    { part: 'root', required: true, description: 'The summary, then the changes in document order.' },
    { part: 'summary', required: true, description: 'A bar with the number of changes, a badge per kind, and the stepper.' },
    { part: 'counts', required: true, description: 'The total and one badge each for added, changed, removed and moved.' },
    { part: 'stepper', required: false, description: 'Previous and Next, with the position: "2 / 5"; each step scrolls to that change and rings it.' },
    { part: 'entries', required: true, description: 'The changes and the folded runs between them.' },
    { part: 'expand', required: false, description: 'A dashed row standing for unchanged blocks; pressing it shows them.' },
    { part: 'change', required: false, description: 'One change, in a card tinted by its kind.' },
    { part: 'changeHeader', required: false, description: 'The change\'s kind and the block\'s type.' },
    { part: 'words', required: false, description: 'A modified block\'s text with removed words struck through and inserted words tinted.' },
    { part: 'sides', required: false, description: 'A heavily modified block, before then after, each labelled.' },
    { part: 'attrs', required: false, description: 'Attribute changes that have no text: a callout\'s tone, an image\'s size.' },
    { part: 'empty', required: false, description: 'One line when the versions are the same.' },
  ],
  accessibility: {
    pattern: null,
    name: null,
    description: [],
    states: [],
    keyboard: [],
    contrast: [
      { what: 'removed words', pair: ['danger', 'danger-soft'], minimum: '4.5:1' },
      { what: 'inserted words', pair: ['success', 'success-soft'], minimum: '4.5:1' },
      { what: 'text in an added card', pair: ['text', 'success-soft/60'], over: 'canvas', minimum: '4.5:1' },
      { what: 'text in a removed card', pair: ['text-muted', 'danger-soft/60'], over: 'canvas', minimum: '4.5:1' },
      { what: 'before label', pair: ['danger', 'danger-soft/50'], over: 'canvas', minimum: '4.5:1' },
      { what: 'after label', pair: ['success', 'success-soft/50'], over: 'canvas', minimum: '4.5:1' },
      { what: 'step position', pair: ['text-muted', 'surface'], minimum: '4.5:1' },
    ],
    notes: [
      'Each card names its kind in words (added, removed, moved, changed), so the tint is never the only sign.',
      'Removed words are struck through as well as tinted; the before and after sides of a heavy edit are labelled.',
      'The stepper\'s buttons are named Previous and Next, and the position is read beside them.',
    ],
  },
  guidance: {
    do: [
      'Leave `mode` at `smart` for review: light edits read as words, rewrites as before and after.',
      'Compute the diff in Logic with `diffDocuments` and pass `diff` for a long document, so the page does not.',
      'Use it beside the history, or before an agent\'s save, to show what will be kept.',
    ],
    dont: [
      'Use it to review an agent\'s change as it happens; the editor marks those blocks and offers undo for the batch.',
    ],
  },
  examples: [
    { name: 'Revision', title: 'A draft against the saved version' },
    { name: 'Inline', title: 'Inline: words, always' },
    { name: 'Block', title: 'Block: before and after, always' },
    { name: 'NoChanges', title: 'No changes' },
  ],
  gaps: [],
  limits: [
    'Inserted words are told apart by their tint alone; a screen reader reads the block\'s new text without marking them.',
    'It compares two versions; it does not merge them or accept a change.',
  ],
  related: ['JinshuEditor', 'JinshuView', 'Badge'],
  gallery: {
    specimen: 'Specimen',
    tagline: 'What changed',
    order: 3,
    page: {
      lead: 'Use JinshuDiff to show what changed between two versions of a document.',
      stage: 'bare',
      hero: 'Revision',
      variants: [{ name: 'Inline', title: 'Inline' }, { name: 'Block', title: 'Block' }],
      states: [{ name: 'NoChanges', title: 'No changes' }],
      statesLayout: 'stack',
    },
  },
};

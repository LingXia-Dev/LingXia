import type { ComponentDoc } from '@liulidesign/core/internal/doc';

export const doc: ComponentDoc = {
  name: 'JinshuOutline',
  category: 'documents',
  status: 'preview',
  since: '2026-11-24',
  summary: 'A Jinshu document\'s headings as a contents list that moves the reader and shows each section\'s progress',
  description:
    'JinshuOutline lists a document\'s headings, indented by level, as a contents list. Shared with a JinshuView or JinshuEditor ' +
    'through `useJinshuNav()`, pressing a heading scrolls the document to it and tints that heading as the one being read; ' +
    'with `nav.onVisibleChange` on the scroll holding a JinshuView, the tint follows the reader\'s scrolling too. ' +
    'An app can mark each section done, current or locked, as a course does.',
  keywords: ['outline', 'table of contents', 'toc', 'contents', 'headings', 'navigation', 'sections', 'jinshu'],
  propsType: 'JinshuOutlineProps',
  recipe: { map: 'jinshuOutlineRecipe', parts: 'jinshuOutlineParts' },
  anatomy: [
    { part: 'root', required: true, description: 'A group named by `title`.' },
    { part: 'title', required: true, description: 'The list\'s heading: "Contents" in the page\'s language unless `title` is set.' },
    { part: 'item', required: true, description: 'A heading, indented by its level; the one being read is tinted in accent.' },
    { part: 'itemText', required: true, description: 'The heading\'s text on one line, or "(Untitled)" for an empty heading.' },
    { part: 'status', required: false, description: 'With `status`: a tick for done, a dot for current, a ring for locked.' },
  ],
  accessibility: {
    pattern: null,
    name: 'title',
    description: [],
    states: [],
    keyboard: [],
    contrast: [
      { what: 'heading', pair: ['text', 'canvas'], minimum: '4.5:1' },
      { what: 'nested heading', pair: ['text-muted', 'canvas'], minimum: '4.5:1' },
      { what: 'heading being read', pair: ['accent', 'accent-soft'], minimum: '4.5:1' },
      { what: 'done', pair: ['success', 'canvas'], minimum: '3:1' },
      { what: 'locked', pair: ['text-muted', 'canvas'], minimum: '3:1' },
    ],
    notes: [
      'Each heading is a button named by its text, followed by its status in words when it has one; the status mark itself is hidden.',
    ],
  },
  guidance: {
    do: [
      'Share one `useJinshuNav()` handle between the outline and the document it moves, and pass `nav.onVisibleChange` to the scroll holding the document.',
      'Put it in a side column on a wide screen, and behind a Contents button on a phone.',
      'Take the headings out in Logic with `extractOutline` and pass `items` when the page does not hold the document.',
    ],
    dont: [
      'Use it for a document with one or two headings; there is nothing to move between.',
    ],
  },
  examples: [
    { name: 'Progress', title: 'With each section\'s progress' },
    { name: 'BesideTheDocument', title: 'Beside the document' },
    { name: 'InPopover', title: 'Behind a Contents button' },
  ],
  gaps: [],
  limits: [
    'The tint follows scrolling only beside a JinshuView whose scroll reports to the nav; beside a JinshuEditor it is the heading last chosen.',
    'A locked section stays pressable; the app decides what opening it does.',
    'An empty document draws nothing.',
  ],
  related: ['JinshuView', 'JinshuEditor', 'Popover'],
  gallery: {
    specimen: 'Specimen',
    tagline: 'Contents, progress',
    order: 4,
    page: {
      lead: 'Use JinshuOutline to let people see a long document\'s sections and move between them.',
      stage: 'bare',
      hero: 'Progress',
      inContext: [
        { name: 'BesideTheDocument', title: 'Beside the document' },
        { name: 'InPopover', title: 'Behind a Contents button' },
      ],
    },
  },
};

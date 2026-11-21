import type { ComponentDoc } from '../doc';

export const doc: ComponentDoc = {
  name: 'Section',
  category: 'content',
  status: 'preview',
  since: '2026-11-15',
  summary: 'A titled part of a page: a heading, then its content',
  description:
    'Section gives a part of a page its heading, an optional line of explanation and an action at the end of the heading\'s line, and keeps one distance between the heading and what follows. ' +
    'It draws no frame: put a Card, an ItemList or plain content inside.',
  keywords: ['section', 'heading', 'group', 'page section', 'header', 'titled block'],
  propsType: 'SectionProps',
  recipe: { map: 'sectionRecipe', parts: 'sectionParts' },
  anatomy: [
    { part: 'root', required: true, description: 'The heading and the content, a fixed distance apart.' },
    { part: 'title', required: true, description: 'The heading, at `headingLevel`.' },
    { part: 'description', required: false, description: 'A line under the heading.' },
    { part: 'action', required: false, description: 'At the end of the heading\'s line.' },
  ],
  accessibility: {
    pattern: null,
    name: null,
    description: [],
    states: [],
    keyboard: [],
    target: {},
    contrast: [
      { what: 'heading', pair: ['text', 'canvas'], minimum: '4.5:1' },
      { what: 'line under it, and a level-4 heading', pair: ['text-muted', 'canvas'], minimum: '4.5:1' },
    ],
    notes: [
      'The title is a heading at `headingLevel`, so a screen reader can move through a page by its sections.',
      'Step the level down inside another section: 2, then 3, then 4.',
    ],
  },
  guidance: {
    do: [
      'Name the section by what it holds: "Recent notes", not "Section 2".',
      'Keep one action at most on the heading\'s line, and make it about the whole section.',
    ],
    dont: [
      'Wrap a single field or row in a section; a heading over one thing is noise.',
      'Skip a heading level to get a smaller size.',
    ],
  },
  examples: [
    { name: 'Recent', title: 'A heading, a line and an action' },
    { name: 'Nested', title: 'Sections inside a section' },
  ],
  gaps: [],
  playground: { props: { title: 'Recent notes', description: 'Edited in the last week' }, fixture: true },
  related: ['Card', 'Item', 'Divider'],
  gallery: {
    specimen: 'Specimen',
    tagline: 'A titled part',
    order: 10,
    page: {
      stage: 'bare',
      lead: 'Use a section to give a part of a page its heading.',
      hero: 'Recent',
      variants: [{ name: 'Nested', title: 'Sections inside a section, their headings a level down' }],
    },
  },
};

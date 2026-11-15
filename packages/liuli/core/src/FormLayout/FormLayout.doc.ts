import type { ComponentDoc } from '../doc';

export const doc: ComponentDoc = {
  name: 'FormLayout',
  category: 'inputs',
  status: 'preview',
  since: '2026-11-15',
  summary: 'A form\'s fields at one rhythm: a column on a phone, two where there is room',
  description:
    'FormLayout sets the distance between a form\'s fields and puts its buttons after them. With `columns={2}` the fields pair up where the form is at least 560 points wide and stay in one column below that; ' +
    'FormSpan, from the same import, gives a field the whole line. It lays fields out and nothing more: each field keeps its own label, help and status.',
  keywords: ['form', 'form layout', 'fields', 'two column form', 'form grid', 'form actions'],
  propsType: 'FormLayoutProps',
  otherPropsTypes: ['FormSpanProps'],
  recipe: { map: 'formLayoutRecipe', parts: 'formLayoutParts' },
  anatomy: [
    { part: 'root', required: true, description: 'The fields, then the actions.' },
    { part: 'fields', required: true, description: 'A column, or a two-column grid where the form is wide enough.' },
    { part: 'span', required: false, description: 'FormSpan: a field that takes the whole line.' },
    { part: 'actions', required: false, description: 'The form\'s buttons, at the end edge; they wrap rather than shrink.' },
  ],
  accessibility: {
    pattern: null,
    name: null,
    description: [],
    states: [],
    keyboard: [{ keys: 'Tab', effect: 'Through the fields in the order they are written, across a line before down, then the actions' }],
    target: {},
    contrast: [],
    notes: [
      'The reading and Tab order is the order the fields are written in, in one column and in two.',
      'It adds no role: the page\'s heading names the form, and each field names itself.',
    ],
  },
  guidance: {
    do: [
      'Write the fields in the order people fill them in; pair only fields that belong together, such as first and last name.',
      'Give long fields a whole line with FormSpan: an address line, a note.',
      'Put the primary action last, at the end edge.',
      'Mark the fewer kind: where most fields are needed, label the optional ones; where most are optional, star the required ones.',
    ],
    dont: [
      'Use two columns for a long form of unrelated fields; one column is filled in faster.',
      'Put a heading inside it for one or two fields; use Section around groups of fields.',
      'Star required fields and label optional ones in the same form.',
    ],
  },
  examples: [
    { name: 'Address', title: 'A delivery address' },
    { name: 'Profile', title: 'Mostly optional, the required field starred' },
  ],
  gaps: [
    { missing: 'Actions that stack full-width on a phone.', needs: 'nothing from the View', today: 'The buttons sit at the end edge and wrap.' },
    { missing: 'More than two columns, and fields that span some of them.', needs: 'nothing from the View', today: 'One or two columns; a field takes one or the whole line.' },
  ],
  related: ['Field', 'TextInput', 'Section'],
  gallery: {
    specimen: 'Specimen',
    tagline: 'Fields in a rhythm',
    order: 11,
    page: {
      lead: 'Use a form layout to set a form\'s fields at one rhythm and put its buttons after them.',
      hero: 'Address',
      variants: [{ name: 'Profile', title: 'Where most fields are optional, the required one is starred' }],
    },
  },
};

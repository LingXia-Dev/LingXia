import type { ComponentDoc } from '../doc';

export const doc: ComponentDoc = {
  name: 'Table',
  category: 'content',
  status: 'preview',
  since: '2026-11-15',
  summary: 'Records in rows and fields in columns, under headings that stay in view',
  description:
    'Table shows a set of records that share the same fields: invoices, members, files. Pass the records as `rows` and say which fields to show as `columns`; ' +
    'a column formats its value, sets its width and aligns numbers to the end edge so their digits line up. ' +
    'With `onRowClick` each row is one button that opens its record. With `maxHeight` the rows scroll under the headings. ' +
    'Where the columns would be too narrow to read, as with four of them on a phone, the table stacks: each record becomes a block with its first field and its number on one line and the other fields, each beside its heading, under them.',
  keywords: ['table', 'data table', 'grid', 'rows', 'columns', 'records', 'spreadsheet', 'data display'],
  propsType: 'TableProps',
  otherPropsTypes: ['TableColumn'],
  recipe: { map: 'tableRecipe', parts: 'tableParts' },
  anatomy: [
    { part: 'root', required: true, description: 'The bordered table, a grid named by `label`.' },
    { part: 'header', required: true, description: 'The row of headings; it stays above scrolling rows.' },
    { part: 'heading', required: true, description: 'One column\'s heading, in muted footnote type; cut short when its column is narrower.' },
    { part: 'body', required: false, description: 'The rows, divided by hairlines; a scroll when `maxHeight` is set.' },
    { part: 'row', required: false, description: 'One record: a grid of the same tracks as the headings. A `button` with `onRowClick`.' },
    { part: 'cell', required: false, description: 'One field of one record; it wraps rather than truncating, except numbers.' },
    { part: 'empty', required: false, description: '`emptyLabel`, in place of the rows.' },
    { part: 'block', required: false, description: 'Stacked: one record, in place of a row of cells.' },
    { part: 'lead', required: false, description: 'Stacked: the first column\'s value in medium weight and, at the end edge, the last number column\'s, so numbers still line up down the table.' },
    { part: 'pairs', required: false, description: 'Stacked: the other fields, each a heading and its value in footnote type, wrapping by whole pairs.' },
  ],
  accessibility: {
    pattern: null,
    name: 'label',
    description: [],
    states: [{ state: 'selected', from: 'selectedKey, on a pressable row' }],
    keyboard: [
      { keys: 'Tab', effect: 'With `onRowClick`: moves through the rows' },
      { keys: 'Enter or Space', effect: 'With `onRowClick`: opens the row' },
    ],
    target: { sm: 36, md: 44 },
    contrast: [
      { what: 'cell text', pair: ['text', 'surface'], minimum: '4.5:1' },
      { what: 'headings and the empty line', pair: ['text-muted', 'surface'], minimum: '4.5:1' },
      { what: 'cell text, selected row', pair: ['text', 'accent-soft'], minimum: '4.5:1' },
    ],
    notes: [
      'The table is a grid named by `label`. Each cell is read with its column\'s heading, "Amount: $420.50", so the row of headings is not read on its own.',
      'With `onRowClick` a row is one button read as all its cells in order, and reports selected when its key is `selectedKey`.',
      'A cell drawn by `cell` is still read as its text, so a badge\'s meaning must be in the field\'s value or `format`.',
      'Stacked or in columns, a record is read the same way: each field with its heading. Stacked, the heading row is not drawn, since every field is beside its own heading or leads the record.',
      '`auto` stacks when a text column would be narrower than 88 points, or narrower than its longest value where that is shorter; the value\'s width is an estimate from its length, since text cannot be measured before layout.',
      'A comfortable row is at least 44 px tall; a `compact` row is 36 px, for tables read with a mouse.',
      'The hairlines between rows are decoration with no contrast minimum.',
    ],
  },
  guidance: {
    do: [
      'Show the few columns a person compares; put the rest on the page a row opens.',
      'Align numbers to the end edge and format them alike, with the same number of decimals.',
      'Give a narrow, fixed-content column (a date, a count) a width in points and let the text columns share the rest.',
      'Put the field that names the record first and its most compared number last: stacked, those two share the first line.',
      'Leave `layout` on `auto`: a table whose text columns fit stays in columns on a phone, and one whose do not stacks.',
    ],
    dont: [
      'Use a table for layout, or for a list whose rows have one field; that is an ItemList.',
      'Pass thousands of rows; page them, or filter first.',
      'Set `isHeaderSticky` on a table that is not inside a `<scroll>`; the View refuses a sticky box there.',
    ],
  },
  examples: [
    { name: 'Invoices', title: 'Records with a number column' },
    { name: 'Pressable', title: 'Rows that open a record' },
    { name: 'Scrolling', title: 'Compact rows under a heading that stays' },
    { name: 'Members', title: 'Four columns, stacked where they do not fit' },
    { name: 'MembersColumns', title: 'The same table held in columns' },
  ],
  gaps: [
    { missing: 'Columns as wide as their content.', needs: 'text measurement before layout', today: 'A width in points or a share of the table; rows are separate grids, so a track sized by content would not line up.' },
    { missing: 'Sorting by a column, and choosing several rows.', needs: 'nothing from the View', today: 'The app sorts `rows`; one row can be selected.' },
    { missing: 'Sideways scrolling with the first column held.', needs: 'a sticky box on the horizontal axis', today: 'Columns share the table\'s width, and the table stacks its records where they would be too narrow.' },
    { missing: 'Rows drawn only while in view.', needs: 'grid rows inside a `<List>`', today: 'Every row is laid out; page a long set.' },
    { missing: 'Column headings read as headers by a screen reader.', needs: 'a column header role', today: 'Each cell is named with its heading.' },
  ],
  related: ['Item', 'Card', 'Badge'],
  gallery: {
    specimen: 'Specimen',
    tagline: 'Rows and columns',
    order: 7,
    page: {
      stage: 'bare',
      lead: 'Use a table for records that share the same fields, when people compare them across rows.',
      hero: 'Invoices',
      variants: [
        { name: 'Pressable', title: 'Rows that open a record, the open one selected; a cell drawn as a badge' },
        { name: 'Scrolling', title: 'Compact rows that scroll under the headings' },
        { name: 'Members', title: 'Four columns: stacked where they do not fit, in columns where they do' },
        { name: 'MembersColumns', title: 'The same table held in columns; on a phone its text columns are too narrow' },
      ],
      states: [{ name: 'StateEmpty', title: 'Empty' }],
    },
  },
};

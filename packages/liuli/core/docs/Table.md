<!-- Generated from the doc file by scripts/generate.mjs; do not edit. -->

# Table

Records in rows and fields in columns, under headings that stay in view. Status: preview.

```ts
import { Table } from '@liulidesign/core/Table';
```

Table shows a set of records that share the same fields: invoices, members, files. Pass the records as `rows` and say which fields to show as `columns`; a column formats its value, sets its width and aligns numbers to the end edge so their digits line up. With `onChange` each row is one button that opens its record. With `maxHeight` the rows scroll under the headings. Where the columns would be too narrow to read, as with four of them on a phone, the table stacks: each record becomes a block with its first field and its number on one line and the other fields, each beside its heading, under them.

## Examples

### Records with a number column

```tsx
import { Table, type TableColumn } from '@liulidesign/core/Table';

const invoices = [
  { id: 'INV-1042', customer: 'Northwind', due: '3 Nov', amount: 1280, status: 'Paid' },
  { id: 'INV-1043', customer: 'Blue Harbor', due: '9 Nov', amount: 420.5, status: 'Due' },
  { id: 'INV-1044', customer: 'Kestrel & Co', due: '12 Nov', amount: 96, status: 'Overdue' },
  { id: 'INV-1045', customer: 'Lumen Studio', due: '20 Nov', amount: 3150, status: 'Due' },
];
const columns: TableColumn[] = [
  { field: 'customer', header: 'Customer', width: '2fr' },
  { field: 'due', header: 'Due' },
  { field: 'amount', header: 'Amount', align: 'end', format: money },
];

/** Records in rows, with the number column at the end edge so its digits line up. */
export function Invoices() {
  return <Table label="Invoices" columns={columns} rows={invoices} keyField="id" data-testid="invoices" />;
}
```

### Rows that open a record

```tsx
import { useLocal } from '@lingxia/view';
import { Badge } from '@liulidesign/core/Badge';
import { Table, type TableColumn, type TableRow } from '@liulidesign/core/Table';

const invoices = [
  { id: 'INV-1042', customer: 'Northwind', due: '3 Nov', amount: 1280, status: 'Paid' },
  { id: 'INV-1043', customer: 'Blue Harbor', due: '9 Nov', amount: 420.5, status: 'Due' },
  { id: 'INV-1044', customer: 'Kestrel & Co', due: '12 Nov', amount: 96, status: 'Overdue' },
  { id: 'INV-1045', customer: 'Lumen Studio', due: '20 Nov', amount: 3150, status: 'Due' },
];
const money = (value: unknown) => `$${grouped(value, 2)}`;
const status = (row: TableRow) => (row.status === 'Paid' ? <Badge label="Paid" tone="success" size="sm" />
  : row.status === 'Overdue' ? <Badge label="Overdue" tone="danger" size="sm" /> : <Badge label={String(row.status)} size="sm" />);
const columns: TableColumn[] = [
  { field: 'customer', header: 'Customer', width: '2fr' },
  { field: 'due', header: 'Due' },
  { field: 'amount', header: 'Amount', align: 'end', format: money },
];

/** Each row is one button that opens its record; the open one is selected. A cell can draw a badge. */
export function Pressable() {
  const open = useLocal<string | null>('open', 'INV-1043');
  return (
    <Table
      label="Invoices"
      columns={[{ field: 'customer', header: 'Customer', width: '2fr' }, { field: 'status', header: 'Status', cell: status }, { field: 'amount', header: 'Amount', align: 'end', format: money }]}
      rows={invoices}
      keyField="id"
      value={open.value}
      onChange={open.set}
      data-testid="pressable"
    />
  );
}
```

### Compact rows under a heading that stays

```tsx
import { Table, type TableColumn } from '@liulidesign/core/Table';

const columns: TableColumn[] = [
  { field: 'customer', header: 'Customer', width: '2fr' },
  { field: 'due', header: 'Due' },
  { field: 'amount', header: 'Amount', align: 'end', format: money },
];
const cities = [
  ['Lagos', 'Nigeria', 15.4], ['Cairo', 'Egypt', 10.0], ['Kinshasa', 'DR Congo', 9.5], ['Johannesburg', 'South Africa', 5.6], ['Nairobi', 'Kenya', 4.4],
  ['Addis Ababa', 'Ethiopia', 3.9], ['Casablanca', 'Morocco', 3.7], ['Accra', 'Ghana', 2.6], ['Dakar', 'Senegal', 1.3], ['Kigali', 'Rwanda', 1.2],
].map(([city, country, people]) => ({ city, country, people }));

/** Compact rows that scroll under a heading that stays. */
export function Scrolling() {
  return (
    <Table
      label="Cities by population"
      columns={[{ field: 'city', header: 'City' }, { field: 'country', header: 'Country' }, { field: 'people', header: 'Millions', align: 'end', width: 72, format: value => Number(value).toFixed(1) }]}
      rows={cities}
      density="compact"
      maxHeight={216}
      data-testid="cities"
    />
  );
}
```

### Four columns, stacked where they do not fit

```tsx
import { Table, type TableColumn } from '@liulidesign/core/Table';

const columns: TableColumn[] = [
  { field: 'customer', header: 'Customer', width: '2fr' },
  { field: 'due', header: 'Due' },
  { field: 'amount', header: 'Amount', align: 'end', format: money },
];
const members = [
  { name: 'Alexandra Papadopoulou-Vandersteen', role: 'Engineering manager', joined: '12 Jan 2021', hours: 1642.5 },
  { name: 'Ben Okafor', role: 'Designer', joined: '3 Mar 2023', hours: 918 },
  { name: 'Chloé Martin', role: 'Support', joined: '28 Oct 2024', hours: 86.25 },
];
const memberColumns: TableColumn[] = [
  { field: 'name', header: 'Name', width: '2fr' },
  { field: 'role', header: 'Role', width: '1.5fr' },
  { field: 'joined', header: 'Joined', width: 84 },
  { field: 'hours', header: 'Hours', align: 'end', width: 64, format: value => grouped(value, 1) },
];

/** Four columns do not fit a phone, so each record is stacked: its name and number on one line, the rest under them. */
export function Members() {
  return <Table label="Members" columns={memberColumns} rows={members} data-testid="members" />;
}
```

### The same table held in columns

```tsx
import { Table, type TableColumn } from '@liulidesign/core/Table';

const columns: TableColumn[] = [
  { field: 'customer', header: 'Customer', width: '2fr' },
  { field: 'due', header: 'Due' },
  { field: 'amount', header: 'Amount', align: 'end', format: money },
];
const members = [
  { name: 'Alexandra Papadopoulou-Vandersteen', role: 'Engineering manager', joined: '12 Jan 2021', hours: 1642.5 },
  { name: 'Ben Okafor', role: 'Designer', joined: '3 Mar 2023', hours: 918 },
  { name: 'Chloé Martin', role: 'Support', joined: '28 Oct 2024', hours: 86.25 },
];
const memberColumns: TableColumn[] = [
  { field: 'name', header: 'Name', width: '2fr' },
  { field: 'role', header: 'Role', width: '1.5fr' },
  { field: 'joined', header: 'Joined', width: 84 },
  { field: 'hours', header: 'Hours', align: 'end', width: 64, format: value => grouped(value, 1) },
];

/** The same table held in columns, as a wide screen shows it. */
export function MembersColumns() {
  return <Table label="Members" columns={memberColumns} rows={members} layout="columns" data-testid="members-columns" />;
}
```

## Props

| Prop | Type | Default | Description |
| --- | --- | --- | --- |
| `label` (required) | `string` |  | The table's accessible name: "Invoices". |
| `columns` (required) | `readonly TableColumn[]` |  |  |
| `rows` (required) | `readonly Readonly<Record<string, TableValue>>[]` |  |  |
| `keyField` | `string` |  | The field that identifies a row across updates, in `value` and in `onChange`. By default the first column's. |
| `layout` | `TableLayout` | `'auto'` |  |
| `density` | `TableDensity` | `'comfortable'` |  |
| `maxHeight` | `number` |  | Caps the rows' height in points: the rows scroll under a heading that stays. |
| `isHeaderSticky` | `boolean` | `false` | Holds the heading at the top of the scroll the table sits in while its rows pass under it. The table must be inside a `<scroll>`; without `maxHeight`. |
| `onChange` | `((key: string) => void)` |  | Makes each row one button; runs with its key. |
| `value` | `string \| null` |  | The row drawn and read as selected. |
| `emptyLabel` | `string` |  | Shown in place of the rows when there are none: "No invoices yet". |
| `className` | `string` |  |  |
| `data-testid` | `string` |  | On the table; a row is `<id>-<key>`. |

## TableColumn props

| Prop | Type | Default | Description |
| --- | --- | --- | --- |
| `field` (required) | `string` |  | The row field this column shows. |
| `header` (required) | `string` |  | The column's heading, and what each of its cells is read with: "Amount". |
| `align` | `TableAlign` | `'start'` | `end` for numbers, so their digits line up. |
| `width` | `number \| `${number}fr`` | `'1fr'` | The column's width in points, or its share of what is left: `'2fr'`. |
| `format` | `((value: TableValue, row: Readonly<Record<string, TableValue>>) => string)` |  | Turns the field's value into the text shown and read. By default the value as written. |
| `cell` | `((row: Readonly<Record<string, TableValue>>) => Child)` |  | Draws the cell in place of its text: a Badge, an Avatar. The text is still what is read. |

## Anatomy

| Part | Element | Required | Description |
| --- | --- | --- | --- |
| root | `<view>` | yes | The bordered table, a grid named by `label`. |
| header | `<view>` | yes | The row of headings; it stays above scrolling rows. |
| heading | `<text>` | yes | One column's heading, in muted footnote type; cut short when its column is narrower. |
| body | `<view>` | no | The rows, divided by hairlines; a scroll when `maxHeight` is set. |
| row | `<view>` | no | One record: a grid of the same tracks as the headings. A `button` with `onChange`. |
| cell | `<view>` | no | One field of one record; it wraps rather than truncating, except numbers. |
| empty | `<text>` | no | `emptyLabel`, in place of the rows. |
| block | `<view>` | no | Stacked: one record, in place of a row of cells. |
| lead | `<view>` | no | Stacked: the first column's value in medium weight and, at the end edge, the last number column's, so numbers still line up down the table. |
| pairs | `<view>` | no | Stacked: the other fields, each a heading and its value in footnote type, wrapping by whole pairs. |

## What an app may override

- `className`: classes merged after the recipe's, so they win.
- The recipe, `tableRecipe` from `@liulidesign/core/Table/recipe`: the classes of each part, to style an element of your own to match. The component does not read a changed copy.
- Scale tokens it reads, set in the app's stylesheet under `@theme`: `--spacing-control-sm`, `--spacing-control-md`, `--text-footnote`, `--text-body`, `--radius-container`.
- Colour roles it paints with, set by the theme pack: `accent`, `accent-soft`, `surface`, `surface-overlay`, `text`, `text-muted`.

## Accessibility

- Pattern: none.
- Name: label.
- Description: none.
- States: selected (from value, on a pressable row).
- Tab: With `onChange`: moves through the rows.
- Enter or Space: With `onChange`: opens the row.
- Press target: sm 36 px, md 44 px.
- The table is a grid named by `label`. Each cell is read with its column's heading, "Amount: $420.50", so the row of headings is not read on its own.
- With `onChange` a row is one button read as all its cells in order, and reports selected when its key is `value`.
- A cell drawn by `cell` is still read as its text, so a badge's meaning must be in the field's value or `format`.
- Stacked or in columns, a record is read the same way: each field with its heading. Stacked, the heading row is not drawn, since every field is beside its own heading or leads the record.
- `auto` stacks when a text column would be narrower than 88 points, or narrower than its longest value where that is shorter; the value's width is an estimate from its length, since text cannot be measured before layout.
- A comfortable row is at least 44 px tall; a `compact` row is 36 px, for tables read with a mouse.
- The hairlines between rows are decoration with no contrast minimum.

| Contrast | Pair | Minimum | Light | Dark |
| --- | --- | --- | --- | --- |
| cell text | `text` on `surface` | 4.5:1 | 16.26:1 | 13.87:1 |
| headings and the empty line | `text-muted` on `surface` | 4.5:1 | 5.89:1 | 6.64:1 |
| cell text, selected row | `text` on `accent-soft` | 4.5:1 | 14.25:1 | 11.26:1 |

Ratios are computed from @lingxia/view/theme and rounded down.

## Guidance

- Do: For a dense screen set `density`, or wrap the screen in a DensityProvider with `compact`.
- Do: Show the few columns a person compares; put the rest on the page a row opens.
- Do: Align numbers to the end edge and format them alike, with the same number of decimals.
- Do: Give a narrow, fixed-content column (a date, a count) a width in points and let the text columns share the rest.
- Do: Put the field that names the record first and its most compared number last: stacked, those two share the first line.
- Do: Leave `layout` on `auto`: a table whose text columns fit stays in columns on a phone, and one whose do not stacks.
- Don't: Use a table for layout, or for a list whose rows have one field; that is an ItemList.
- Don't: Pass thousands of rows; page them, or filter first.
- Don't: Set `isHeaderSticky` on a table that is not inside a `<scroll>`; the View refuses a sticky box there.

## Not yet

| Missing | Needs | Meanwhile |
| --- | --- | --- |
| Columns as wide as their content. | text measurement before layout | A width in points or a share of the table; rows are separate grids, so a track sized by content would not line up. |
| Sorting by a column, and choosing several rows. | nothing from the View | The app sorts `rows`; one row can be selected. |
| Sideways scrolling with the first column held. | a sticky box on the horizontal axis | Columns share the table's width, and the table stacks its records where they would be too narrow. |
| Rows drawn only while in view. | grid rows inside a `<List>` | Every row is laid out; page a long set. |
| Column headings read as headers by a screen reader. | a column header role | Each cell is named with its heading. |

Related: Item, Card, Badge.

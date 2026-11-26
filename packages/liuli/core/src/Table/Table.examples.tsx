import { useLocal } from '@lingxia/view';
import { Badge } from '../Badge/Badge';
import { Table, type TableColumn, type TableRow, type TableProps, type TableSort } from './Table';

const invoices = [
  { id: 'INV-1042', customer: 'Northwind', due: '3 Nov', amount: 1280, status: 'Paid' },
  { id: 'INV-1043', customer: 'Blue Harbor', due: '9 Nov', amount: 420.5, status: 'Due' },
  { id: 'INV-1044', customer: 'Kestrel & Co', due: '12 Nov', amount: 96, status: 'Overdue' },
  { id: 'INV-1045', customer: 'Lumen Studio', due: '20 Nov', amount: 3150, status: 'Due' },
];
const grouped = (value: unknown, decimals: number) => Number(value).toFixed(decimals).replace(/\B(?=(\d{3})+(?!\d))/g, ',');
const money = (value: unknown) => `$${grouped(value, 2)}`;
const status = (row: TableRow) => (row.status === 'Paid' ? <Badge label="Paid" tone="success" size="sm" />
  : row.status === 'Overdue' ? <Badge label="Overdue" tone="danger" size="sm" /> : <Badge label={String(row.status)} size="sm" />);

const columns: TableColumn[] = [
  { field: 'customer', header: 'Customer', width: '2fr' },
  { field: 'due', header: 'Due' },
  { field: 'amount', header: 'Amount', align: 'end', format: money },
];

/** Records in rows, with the number column at the end edge so its digits line up. */
export function Invoices() {
  return <Table label="Invoices" columns={columns} rows={invoices} keyField="id" data-testid="invoices" />;
}

/** Each row is one button that opens its record; the open one is selected. A cell can draw a badge. */
export function Pressable() {
  const open = useLocal<string | null>('open', 'INV-1043');
  return (
    <Table
      label="Invoices"
      columns={[{ field: 'customer', header: 'Customer', width: '2fr' }, { field: 'status', header: 'Status', cell: status }, { field: 'amount', header: 'Amount', align: 'end', format: money }]}
      rows={invoices}
      keyField="id"
      bind={open}
      data-testid="pressable"
    />
  );
}

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

/** The same table held in columns, as a wide screen shows it. */
export function MembersColumns() {
  return <Table label="Members" columns={memberColumns} rows={members} layout="columns" data-testid="members-columns" />;
}

const cities = [
  ['Lagos', 'Nigeria', 15.4], ['Cairo', 'Egypt', 10.0], ['Kinshasa', 'DR Congo', 9.5], ['Johannesburg', 'South Africa', 5.6], ['Nairobi', 'Kenya', 4.4],
  ['Addis Ababa', 'Ethiopia', 3.9], ['Casablanca', 'Morocco', 3.7], ['Accra', 'Ghana', 2.6], ['Dakar', 'Senegal', 1.3], ['Kigali', 'Rwanda', 1.2],
].map(([city, country, people]) => ({ city, country, people }));

const months = ['Jan', 'Feb', 'Mar', 'Apr', 'May', 'Jun', 'Jul', 'Aug', 'Sep', 'Oct', 'Nov', 'Dec'];
const day = (value: unknown) => { const [, m, d] = String(value).split('-').map(Number); return `${d} ${months[m - 1]}`; };
const bills = [
  { id: 'INV-1042', customer: 'Northwind', dueOn: '2026-11-03', amount: 1280 },
  { id: 'INV-1043', customer: 'Blue Harbor', dueOn: '2026-11-09', amount: 420.5 },
  { id: 'INV-1044', customer: 'Kestrel & Co', dueOn: '2026-10-28', amount: 96 },
  { id: 'INV-1045', customer: 'Lumen Studio', dueOn: '2026-11-20', amount: 3150 },
  { id: 'INV-1046', customer: 'Atlas Freight', dueOn: '2026-11-14', amount: null },
];
const sortableColumns: TableColumn[] = [
  { field: 'customer', header: 'Customer', width: '2fr', isSortable: true },
  { field: 'dueOn', header: 'Due', format: day, isSortable: true },
  { field: 'amount', header: 'Amount', align: 'end', format: value => (value === null ? '—' : money(value)), isSortable: true },
];

/** Headings that order the rows: a press sorts by that column, a second press the other way. A row without an amount stays last. */
export function Sortable() {
  return <Table label="Invoices" columns={sortableColumns} rows={bills} keyField="id" data-testid="sortable" />;
}

/** The order held by the page, here starting with the latest due date, so it can be kept or sent to a server. */
export function SortControlled() {
  const sort = useLocal<TableSort>('sort', { field: 'dueOn', direction: 'descending' });
  return <Table label="Invoices" columns={sortableColumns} rows={bills} keyField="id" sort={sort.value} onSortChange={sort.set} data-testid="sort-controlled" />;
}

/** On a phone the records stack, and the order is set from one menu above them. */
export function SortStacked() {
  return <Table label="Invoices" columns={sortableColumns} rows={bills} keyField="id" layout="stacked" data-testid="sort-stacked" />;
}

/** While the rows load: placeholder bars under the headings, the table already its final shape. */
export function Loading() {
  return <Table label="Invoices" columns={sortableColumns} rows={[]} keyField="id" isLoading emptyLabel="No invoices yet" data-testid="loading" />;
}

/** Loading stacked records, each checkbox already in its place. */
export function LoadingStacked() {
  const picked = useLocal<string[]>('picked', []);
  return <Table label="Invoices" columns={sortableColumns} rows={[]} keyField="id" layout="stacked" isLoading checkedRows={picked.value} onCheckedRowsChange={picked.set} data-testid="loading-stacked" />;
}

/** A checkbox on each row and one for all; the rows checked are the page's, here counted above the table. */
export function Selectable() {
  const picked = useLocal<string[]>('picked', ['INV-1043']);
  return (
    <view className="self-stretch gap-2">
      <text role="status" className="text-footnote text-muted">{picked.value.length === 0 ? 'None selected' : `${picked.value.length} selected`}</text>
      <Table label="Invoices" columns={[sortableColumns[0], sortableColumns[2]]} rows={bills} keyField="id" checkedRows={picked.value} onCheckedRowsChange={picked.set} data-testid="selectable" />
    </view>
  );
}

/** Stacked, the checkbox for all sits beside the sort menu, and each record's at its start. */
export function SelectableStacked() {
  const picked = useLocal<string[]>('picked', []);
  return <Table label="Invoices" columns={sortableColumns} rows={bills} keyField="id" layout="stacked" checkedRows={picked.value} onCheckedRowsChange={picked.set} data-testid="selectable-stacked" />;
}

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

export function StateEmpty() {
  return <Table label="Invoices" columns={columns} rows={[]} emptyLabel="No invoices yet" data-testid="empty" />;
}

/** The home tile. */
export function Specimen() {
  // Short values: a tile is too narrow for a name beside a price, and a table wraps its text rather than cutting it.
  return <Table label="Cities by population" columns={[{ field: 'city', header: 'City' }, { field: 'people', header: 'Millions', align: 'end', format: value => Number(value).toFixed(1) }]} rows={cities.slice(0, 1)} density="compact" className="w-full" />;
}

/** The playground's props that are not plain values. */
export function playground() {
  return { value: null, columns, rows: invoices } satisfies Partial<TableProps>;
}

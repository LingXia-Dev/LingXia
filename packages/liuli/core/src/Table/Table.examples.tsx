import { useLocal } from '@lingxia/view';
import { Badge } from '../Badge/Badge';
import { Table, type TableColumn, type TableRow } from './Table';

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
      selectedKey={open.value}
      onRowClick={open.set}
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
  return <Table label="Invoices" columns={[{ field: 'customer', header: 'Customer' }, { field: 'amount', header: 'Amount', align: 'end', format: money }]} rows={invoices.slice(0, 2)} density="compact" />;
}

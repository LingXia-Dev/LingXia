// Type test, checked by `tsc`: a chart's field props name fields of its own rows, so a misspelt
// column fails to compile instead of drawing nothing.
import { BarChart } from '../BarChart/BarChart';
import type { ChartSeries } from '../Chart/Chart';
import { LineChart } from '../LineChart/LineChart';
import { PieChart } from '../PieChart/PieChart';

interface Sale { month: string; online: number; store: number }
const sales: readonly Sale[] = [{ month: 'Jan', online: 42, store: 31 }];
const channels: readonly ChartSeries<Sale>[] = [{ field: 'online', label: 'Online' }, { field: 'store', label: 'In store' }];
const spend = [{ category: 'Rent', amount: 1200 }];
// Fields a chart does not name may hold anything.
interface Order { day: string; total: number; paid: boolean; placed: Date; tags: string[]; customer: { name: string } }
const orders: readonly Order[] = [];

export const named = [
  <BarChart rows={sales} x="month" y="online" keyField="month" label="Sales" />,
  <BarChart rows={sales} x="month" y={channels} label="Sales by channel" />,
  <LineChart rows={sales} x="month" y={[{ field: 'store', label: 'In store' }]} label="Store sales" />,
  <PieChart rows={spend} nameField="category" valueField="amount" label="Spending" />,
  <BarChart rows={orders} x="day" y="total" label="Orders" />,
  <LineChart rows={orders} x="day" y={[{ field: 'total', label: 'Total' }]} label="Orders" />,
  <PieChart rows={orders} nameField="day" valueField="total" label="Orders" />,
];

export const misspelt = [
  // @ts-expect-error: no field "mnth"
  <BarChart rows={sales} x="mnth" y="online" label="Sales" />,
  // @ts-expect-error: no field "web"
  <LineChart rows={sales} x="month" y={[{ field: 'web', label: 'Web' }]} label="Web sales" />,
  // @ts-expect-error: no field "ammount"
  <PieChart rows={spend} nameField="category" valueField="ammount" label="Spending" />,
  // @ts-expect-error: no field "id"
  <BarChart rows={sales} x="month" y="store" keyField="id" label="Sales" />,
  // @ts-expect-error: "placed" holds a date, which a chart cannot read
  <BarChart rows={orders} x="placed" y="total" label="Orders" />,
];

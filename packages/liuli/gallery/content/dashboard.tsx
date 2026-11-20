import { useBox, useLocal, useView } from '@lingxia/view';
import { BarChart } from '@liulidesign/charts/BarChart';
import { LineChart } from '@liulidesign/charts/LineChart';
import { Sparkline } from '@liulidesign/charts/Sparkline';
import { Stat } from '@liulidesign/charts/Stat';
import { summarize } from '@liulidesign/charts/summary/en';
import { Badge } from '@liulidesign/core/Badge';
import { Card } from '@liulidesign/core/Card';
import { SegmentedControl } from '@liulidesign/core/SegmentedControl';
import { Table, type TableRow } from '@liulidesign/core/Table';

type Period = 'week' | 'month' | 'year';

const grouped = (value: number, decimals = 0) => value.toFixed(decimals).replace(/\B(?=(\d{3})+(?!\d))/g, ',');
const euros = (value: number) => `€${grouped(value)}`;

const weekdays = ['Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat', 'Sun'];
const months = ['Jan', 'Feb', 'Mar', 'Apr', 'May', 'Jun', 'Jul', 'Aug', 'Sep', 'Oct', 'Nov', 'Dec'];

// A shop's sales, by period. Every figure on the page is derived from these rows.
// A day's or month's refund rate, in percent, wavering around the period's level.
const wobble = (level: number, index: number) => Math.round((level + ((index * 7) % 5 - 2) * 0.15) * 10) / 10;

const series: Record<Period, { label: string; revenue: number; orders: number; refunds: number }[]> = {
  week: [4120, 3980, 4460, 4710, 5240, 6130, 5380].map((revenue, index) => ({ label: weekdays[index], revenue, orders: [58, 55, 63, 66, 74, 91, 79][index], refunds: wobble(1.9, index) })),
  month: [3650, 3820, 4010, 3760, 4240, 4980, 4420, 3890, 4050, 4310, 4120, 4690, 5520, 4870, 4020, 4180, 4460, 4290, 4830, 5710, 5060,
    4120, 3980, 4460, 4710, 5240, 6130, 5380, 4350, 4520].map((revenue, index) => ({ label: `${index + 1} Nov`, revenue, orders: Math.round(revenue / 68), refunds: wobble(2.3, index) })),
  year: [98400, 91200, 104800, 109300, 118700, 112400, 121900, 117600, 126300, 134800, 141200, 0].map((revenue, index) => ({ label: months[index], revenue, orders: Math.round(revenue / 67), refunds: wobble(2.2, index) }))
    .slice(0, 11),
};
const before: Record<Period, { revenue: number; orders: number; refunds: number }> = {
  week: { revenue: 31280, orders: 462, refunds: 2.4 },
  month: { revenue: 124600, orders: 1851, refunds: 2.1 },
  year: { revenue: 1038000, orders: 15820, refunds: 2.6 },
};
const versus: Record<Period, string> = { week: 'vs last week', month: 'vs October', year: 'vs 2025' };

// How a period's orders split by channel; the bars are these shares of its total.
const channelShare = [{ channel: 'Web', share: 0.47 }, { channel: 'App', share: 0.31 }, { channel: 'Shops', share: 0.16 }, { channel: 'Partners', share: 0.06 }];
const span: Record<Period, string> = { week: 'Last 7 days', month: 'Last 30 days', year: '2026 so far' };

const products: TableRow[] = [
  { product: 'Linen shirt', stock: 'In stock', sold: 412, revenue: 24308 },
  { product: 'Canvas tote', stock: 'In stock', sold: 386, revenue: 9264 },
  { product: 'Wool beanie', stock: 'Low', sold: 301, revenue: 8127 },
  { product: 'Leather belt', stock: 'In stock', sold: 174, revenue: 9396 },
  { product: 'Rain jacket', stock: 'Sold out', sold: 139, revenue: 16541 },
];
const stock = (row: TableRow) => (row.stock === 'In stock' ? <Badge label="In stock" tone="success" size="sm" />
  : row.stock === 'Low' ? <Badge label="Low" tone="warning" size="sm" /> : <Badge label="Sold out" tone="danger" size="sm" />);

/** A change between two numbers, as the three things a Stat asks for. */
function change(now: number, was: number, unit = '%') {
  const moved = unit === '%' ? ((now - was) / was) * 100 : now - was;
  const text = `${Math.abs(moved).toFixed(1)}${unit}`;
  return { delta: text, deltaDirection: moved > 0.05 ? 'up' as const : moved < -0.05 ? 'down' as const : 'flat' as const, moved, text };
}

export function Content() {
  const { env } = useView();
  const frame = useBox('frame');
  const figure = useBox('figure');
  const period = useLocal<Period>('period', 'week');
  const day = useLocal<string | null>('day', null);
  const rows = series[period.value];
  const revenue = rows.reduce((sum, row) => sum + row.revenue, 0);
  const orders = rows.reduce((sum, row) => sum + row.orders, 0);
  const refundRate = rows.reduce((sum, row) => sum + row.refunds, 0) / rows.length;
  const channels = channelShare.map(each => ({ channel: each.channel, orders: Math.round(orders * each.share) }));
  const was = before[period.value];
  const [byRevenue, byOrders, byBasket, byRefunds] = [
    change(revenue, was.revenue), change(orders, was.orders), change(revenue / orders, was.revenue / was.orders), change(refundRate, was.refunds, ' pt'),
  ];
  const words = (name: string, by: ReturnType<typeof change>) => `${name} ${by.deltaDirection === 'flat' ? 'unchanged' : `${by.deltaDirection} ${by.text}`} ${versus[period.value]}`;
  const width = frame.value?.width ?? (env.sizeClass === 'regular' ? 900 : 328);
  const isWide = width >= 640;
  // A tile holds its figure whole: the widest, measured in the Stat's type, plus the card's padding and border.
  const figures = [euros(revenue), grouped(orders), `€${grouped(revenue / orders, 2)}`, `${refundRate.toFixed(1)}%`];
  const tile = (figure.value?.width ?? 0) + 26;
  const perRow = isWide && width >= tile * 4 + 36 ? 4 : width >= tile * 2 + 12 ? 2 : 1;
  // Each tile shows its trend, so its sparkline fits its own range: an area would be measured from zero.
  const spark = (values: readonly number[], label: string) => <Sparkline values={values} isSmooth hasEndDot label={label} />;

  return (
    <view box={frame} className="gap-5">
      <view className="gap-3">
        <view className="flex-row items-end justify-between gap-3">
          <view className="gap-0.5">
            <text role="heading" headingLevel={1} className="text-title font-semibold tracking-tight text-text">Sales</text>
            <text className="text-footnote text-muted">Linden Street store · all channels</text>
          </view>
        </view>
        <SegmentedControl
          label="Period"
          value={period.value}
          onChange={next => { period.set(next as Period); day.set(null); }}
          items={[{ value: 'week', label: 'Week' }, { value: 'month', label: 'Month' }, { value: 'year', label: 'Year' }]}
          className={isWide ? 'w-80' : undefined}
          data-testid="period"
        />
      </view>

      <view className={perRow === 4 ? 'grid grid-cols-4 gap-3' : perRow === 2 ? 'grid grid-cols-2 gap-3' : 'gap-3'} data-testid="stats">
        <Card padding="sm"><Stat label="Revenue" value={euros(revenue)} delta={byRevenue.delta} deltaDirection={byRevenue.deltaDirection} deltaLabel={words('Revenue', byRevenue)} description={versus[period.value]} chart={spark(rows.map(row => row.revenue), 'Revenue over the period')} size="sm" data-testid="stat-revenue" /></Card>
        <Card padding="sm"><Stat label="Orders" value={grouped(orders)} delta={byOrders.delta} deltaDirection={byOrders.deltaDirection} deltaLabel={words('Orders', byOrders)} description={versus[period.value]} chart={spark(rows.map(row => row.orders), 'Orders over the period')} size="sm" data-testid="stat-orders" /></Card>
        <Card padding="sm"><Stat label="Average order" value={`€${grouped(revenue / orders, 2)}`} delta={byBasket.delta} deltaDirection={byBasket.deltaDirection} deltaLabel={words('Average order', byBasket)} description={versus[period.value]} chart={spark(rows.map(row => row.revenue / row.orders), 'Average order over the period')} size="sm" data-testid="stat-basket" /></Card>
        <Card padding="sm"><Stat label="Refund rate" value={`${refundRate.toFixed(1)}%`} delta={byRefunds.delta} deltaDirection={byRefunds.deltaDirection} deltaTone={byRefunds.moved > 0 ? 'negative' : 'positive'} deltaLabel={words('Refund rate', byRefunds)} description={versus[period.value]} chart={spark(rows.map(row => row.refunds), 'Refund rate over the period')} size="sm" data-testid="stat-refunds" /></Card>
      </view>

      <view className={isWide ? 'grid grid-cols-[2fr_1fr] gap-3' : 'gap-3'}>
        <Card>
          <LineChart
            rows={rows}
            x="label"
            y="revenue"
            label="Revenue by day"
            title="Revenue"
            description={period.value === 'year' ? 'By month' : 'By day'}
            formatValue={euros}
            summarize={summarize}
            isSmooth={period.value !== 'week'}
            value={day.value}
            onChange={day.set}
            data-testid="revenue-chart"
          />
        </Card>
        <Card>
          <BarChart rows={channels} x="channel" y="orders" label="Orders by channel" title="Orders by channel" description={span[period.value]} formatValue={value => grouped(value)} summarize={summarize} size={isWide ? 'md' : 'sm'} data-testid="channels" />
        </Card>
      </view>

      {/* Titled inside its card, as the charts are; the table gives up its own frame to the card's. */}
      <Card padding="none" className="overflow-hidden">
        <view className="gap-0.5 px-4 pt-4 pb-3">
          <text role="heading" headingLevel={2} className="text-body font-semibold text-text">Top products</text>
          <text className="text-footnote text-muted">{span[period.value]}</text>
        </view>
        <Table
          label="Top products"
          keyField="product"
          columns={[
            { field: 'product', header: 'Product', width: '2fr' },
            { field: 'stock', header: 'Stock', cell: stock },
            { field: 'sold', header: 'Sold', align: 'end', width: 64, format: value => grouped(Number(value)) },
            { field: 'revenue', header: 'Revenue', align: 'end', width: 88, format: value => euros(Number(value)) },
          ]}
          rows={products}
          className="rounded-none border-x-0 border-b-0"
          data-testid="products"
        />
      </Card>
      <view aria-hidden className="absolute size-0 overflow-hidden opacity-0 flex-row">
        <text box={figure} className="shrink-0 whitespace-nowrap text-title font-semibold tracking-tight tabular-nums">{figures.reduce((most, each) => (each.length > most.length ? each : most))}</text>
      </view>
    </view>
  );
}

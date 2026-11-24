// The data model every chart shares: rows of fields, an encoding that names fields, and the
// facts a summary is written from.

export type ChartDatum = string | number | null | undefined;
/** One record: a day, a product, a region. Charts read the fields their encoding names. */
export type ChartRow = Readonly<Record<string, ChartDatum>>;
/** A row type whose every field is a datum: an app's own record type, interfaces included. */
export type ChartRecord<Row> = { readonly [K in keyof Row]: ChartDatum };
/** A field of `Row` by name, so a misspelt column fails to compile. */
export type ChartField<Row> = keyof Row & string;
/** A series colour: the theme's `chart-1` to `chart-8`. */
export type ChartColor = 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8;

export interface ChartSeries<Row = ChartRow> {
  /** The row field that holds this series' number. */
  field: ChartField<Row>;
  /** Its name in the legend and in readings: "Online". */
  label: string;
  /** Its colour; by default series take the eight in order. */
  color?: ChartColor;
}

export interface ChartPointFact {
  /** The row's category or slice name. */
  x: string;
  value: number;
  /** `value` as the chart formats it. */
  text: string;
  /** A pie slice's part of the whole, 0 to 1. */
  share?: number;
}

export interface ChartSeriesFacts {
  /** Empty for a chart with one unnamed series. */
  label: string;
  /** Rows with a finite value. */
  count: number;
  first: ChartPointFact;
  last: ChartPointFact;
  min: ChartPointFact;
  max: ChartPointFact;
  total: number;
  totalText: string;
  /** The last value against the first. */
  trend: 'up' | 'down' | 'flat';
}

/** What a chart knows about its data, for a `summarize` function to put into words. */
export interface ChartFacts {
  kind: 'bar' | 'line' | 'pie';
  /** Rows drawn. */
  count: number;
  /** Series with at least one finite value. */
  series: readonly ChartSeriesFacts[];
}

export type ChartSummarize = (facts: ChartFacts) => string;

export const colorCount = 8;
export const colorOf = (index: number, color?: ChartColor): ChartColor => color ?? (((index % colorCount) + 1) as ChartColor);
const roles = { 1: 'chart-1', 2: 'chart-2', 3: 'chart-3', 4: 'chart-4', 5: 'chart-5', 6: 'chart-6', 7: 'chart-7', 8: 'chart-8' } as const satisfies Record<ChartColor, string>;
export const roleOf = (color: ChartColor) => roles[color];

/** The field as a number; anything that is not a finite number is a gap. */
export function numberOf(row: ChartRow, field: string): number {
  const value = row[field];
  return typeof value === 'number' && Number.isFinite(value) ? value : NaN;
}

export const textOf = (row: ChartRow, field: string): string => String(row[field] ?? '');

/** A row's identity across updates: its key field, or its position when the field is empty. */
export const keyOf = (row: ChartRow, field: string, index: number): string => String(row[field] ?? index);

// A series of any row type, its field a plain name; `ChartSeries<ChartRow>` would refuse one typed to a narrower row.
type AnySeries = Omit<ChartSeries, 'field'> & { field: string };
export const seriesOf = (y: string | readonly AnySeries[]): readonly ChartSeries[] => (typeof y === 'string' ? [{ field: y, label: '' }] : y);

export const plain = (value: number) => String(value);

export function seriesFacts(
  rows: readonly ChartRow[], x: string, series: ChartSeries, format: (value: number) => string, hasShares = false,
): ChartSeriesFacts | undefined {
  const points = rows.map(row => ({ x: textOf(row, x), value: numberOf(row, series.field) })).filter(point => Number.isFinite(point.value));
  if (!points.length) return undefined;
  const total = points.reduce((sum, point) => sum + point.value, 0);
  const fact = (point: { x: string; value: number }): ChartPointFact => ({
    ...point, text: format(point.value), ...(hasShares && total > 0 ? { share: point.value / total } : {}),
  });
  const [first, last] = [points[0], points[points.length - 1]];
  const by = (pick: (a: number, b: number) => boolean) => points.reduce((best, point) => (pick(point.value, best.value) ? point : best));
  return {
    label: series.label,
    count: points.length,
    first: fact(first),
    last: fact(last),
    min: fact(by((a, b) => a < b)),
    max: fact(by((a, b) => a > b)),
    total,
    totalText: format(total),
    trend: last.value > first.value ? 'up' : last.value < first.value ? 'down' : 'flat',
  };
}

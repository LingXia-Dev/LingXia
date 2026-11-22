/** A chart the stream page's assistant attaches to a reply. */
export interface ChartData {
  kind: 'bar' | 'line' | 'pie';
  title: string;
  series: { label: string; value: number }[];
}

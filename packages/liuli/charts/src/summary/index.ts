import { liuliText } from '@liulidesign/core/internal/text';
import type { ChartFacts, ChartSeriesFacts } from '../internal/data';

const percent = (share: number) => `${Math.round(share * 100)}%`;
const then = (before: string, after: string) => liuliText('@liuli.chart.summary.then', { before, after });

// Bars are categories, not a run in time: only a line has a trend, a first and a latest.
function series(facts: ChartSeriesFacts, isOrdered: boolean): string {
  let text = facts.count === 1
    ? liuliText('@liuli.chart.summary.single', { x: facts.first.x, value: facts.first.text })
    : liuliText('@liuli.chart.summary.range', {
      count: String(facts.count), first: facts.first.x, last: facts.last.x,
      min: facts.min.text, minAt: facts.min.x, max: facts.max.text, maxAt: facts.max.x,
    });
  if (isOrdered && facts.count > 1)
    text = then(text, liuliText('@liuli.chart.summary.trend', { trend: facts.trend, first: facts.first.text, last: facts.last.text }));
  return facts.label ? liuliText('@liuli.chart.summary.named', { label: facts.label, text }) : text;
}

/** A chart's facts as sentences in the page's language: what a chart reads to a screen reader by default. */
export function summarize(facts: ChartFacts): string {
  if (!facts.series.length) return liuliText('@liuli.chart.summary.none');
  if (facts.kind !== 'pie') return facts.series.map(each => series(each, facts.kind === 'line')).reduce(then);
  const [parts] = facts.series;
  const share = (point: { share?: number }) => (point.share === undefined ? '' : liuliText('@liuli.chart.summary.share', { share: percent(point.share) }));
  if (parts.count === 1) return liuliText('@liuli.chart.summary.onePart', { x: parts.first.x, value: parts.first.text });
  return liuliText('@liuli.chart.summary.parts', {
    count: String(parts.count), total: parts.totalText,
    max: parts.max.text, maxAt: parts.max.x, maxShare: share(parts.max),
    min: parts.min.text, minAt: parts.min.x, minShare: share(parts.min),
  });
}

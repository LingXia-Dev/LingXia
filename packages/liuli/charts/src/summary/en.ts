import type { ChartFacts, ChartSeriesFacts } from '../internal/data';

const trend = { up: 'Rising', down: 'Falling', flat: 'Level' } as const;
const percent = (share: number) => `${Math.round(share * 100)}%`;

// Bars are categories, not a run in time: only a line has a trend, a first and a latest.
function series(facts: ChartSeriesFacts, isOrdered: boolean): string {
  const name = facts.label ? `${facts.label}: ` : '';
  if (facts.count === 1) return `${name}${facts.first.x}, ${facts.first.text}.`;
  return `${name}${facts.count} values from ${facts.first.x} to ${facts.last.x}. ` +
    `Lowest ${facts.min.text} at ${facts.min.x}, highest ${facts.max.text} at ${facts.max.x}.` +
    (isOrdered ? ` ${trend[facts.trend]}: ${facts.first.text} first, ${facts.last.text} last.` : '');
}

/** A chart's facts as English sentences, for `summarize`. */
export function summarize(facts: ChartFacts): string {
  if (!facts.series.length) return 'No data.';
  if (facts.kind !== 'pie') return facts.series.map(each => series(each, facts.kind === 'line')).join(' ');
  const [parts] = facts.series;
  const share = (point: { share?: number }) => (point.share === undefined ? '' : `, ${percent(point.share)}`);
  if (parts.count === 1) return `One part: ${parts.first.x}, ${parts.first.text}.`;
  return `${parts.count} parts, ${parts.totalText} in all. ` +
    `Largest: ${parts.max.x}, ${parts.max.text}${share(parts.max)}. Smallest: ${parts.min.x}, ${parts.min.text}${share(parts.min)}.`;
}

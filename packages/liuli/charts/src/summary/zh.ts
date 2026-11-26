import type { ChartFacts, ChartSeriesFacts } from '../internal/data';

const trend = { up: '整体上升', down: '整体下降', flat: '首尾持平' } as const;
const percent = (share: number) => `${Math.round(share * 100)}%`;

// Bars are categories, not a run in time: only a line has a trend, a first and a latest.
function series(facts: ChartSeriesFacts, isOrdered: boolean): string {
  const name = facts.label ? `${facts.label}：` : '';
  if (facts.count === 1) return `${name}${facts.first.x}，${facts.first.text}。`;
  return `${name}共 ${facts.count} 个数值，从${facts.first.x}到${facts.last.x}。` +
    `最低 ${facts.min.text}（${facts.min.x}），最高 ${facts.max.text}（${facts.max.x}）。` +
    (isOrdered ? `${trend[facts.trend]}：起始 ${facts.first.text}，最新 ${facts.last.text}。` : '');
}

/** 把图表的事实写成中文句子，传给 `summarize`。 */
export function summarize(facts: ChartFacts): string {
  if (!facts.series.length) return '暂无数据。';
  if (facts.kind !== 'pie') return facts.series.map(each => series(each, facts.kind === 'line')).join('');
  const [parts] = facts.series;
  const share = (point: { share?: number }) => (point.share === undefined ? '' : `，占 ${percent(point.share)}`);
  if (parts.count === 1) return `仅一项：${parts.first.x}，${parts.first.text}。`;
  return `共 ${parts.count} 项，合计 ${parts.totalText}。` +
    `最大：${parts.max.x}，${parts.max.text}${share(parts.max)}。最小：${parts.min.x}，${parts.min.text}${share(parts.min)}。`;
}

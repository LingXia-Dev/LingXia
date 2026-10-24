// ECharts' default emphasis lifts RGB channels by 10%, with integer truncation.
export function chartEmphasisColor(color: string): string {
  return '#' + [1, 3, 5].map(offset =>
    Math.min(255, Math.floor(parseInt(color.slice(offset, offset + 2), 16) * 1.1))
      .toString(16).padStart(2, '0'),
  ).join('');
}

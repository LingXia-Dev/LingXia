// Distribute hundredths by largest remainder so visible slices total 100%.
export function chartPercentages(values: readonly number[]): number[] {
  const total = values.reduce((sum, value) => sum + value, 0);
  if (!total) return values.map(() => 0);
  const exact = values.map(value => value / total * 100 * 100);
  const hundredths = exact.map(Math.floor);
  const remaining = 10000 - hundredths.reduce((sum, value) => sum + value, 0);
  const order = values.map((_, index) => index).sort((a, b) =>
    (exact[b] - hundredths[b]) - (exact[a] - hundredths[a]) || a - b);
  for (let index = 0; index < remaining; index++) hundredths[order[index]]++;
  return hundredths.map(value => value / 100);
}

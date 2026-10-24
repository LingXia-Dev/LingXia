export function chartScale(values: readonly number[]) {
  const low = Math.min(0, ...values), high = Math.max(0, ...values);
  if (low === high) return { min: 0, max: 1, ticks: [1, 0.8, 0.6, 0.4, 0.2, 0] };
  const raw = (high - low) / 5;
  const power = Math.max(Number.MIN_VALUE, 10 ** Math.floor(Math.log10(raw || Number.MIN_VALUE)));
  const normalized = raw / power;
  const step = (normalized < 1.5 ? 1 : normalized < 2.5 ? 2 : normalized < 4 ? 3 : normalized < 7 ? 5 : 10) * power;
  const min = Math.max(-1e9, Math.min(low, Number((Math.floor(low / step) * step).toPrecision(12))));
  const max = Math.min(1e9, Math.max(high, Number((Math.ceil(high / step) * step).toPrecision(12))));
  const ticks = [max];
  for (let index = Math.floor(max / step); index * step > min && ticks.length < 16; index--) {
    const value = Number((index * step).toPrecision(12));
    if (value < max && value > min && max - value > step * 1e-8) ticks.push(value);
  }
  ticks.push(min);
  return { min, max, ticks };
}

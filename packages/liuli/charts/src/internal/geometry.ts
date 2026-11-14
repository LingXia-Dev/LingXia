// Chart geometry in a 100 × 100 box that the vector stretches to its own size.

export const box = 100;

/** A longer series is reduced to this many points: a small chart has no room for more. */
export const maxPoints = 240;

export type Point = readonly [number, number];

const round = (value: number) => String(Math.round(value * 100) / 100);

const mean = (values: readonly number[]) => {
  const finite = values.filter(Number.isFinite);
  return finite.length ? finite.reduce((sum, value) => sum + value, 0) / finite.length : NaN;
};

/**
 * `values` reduced to at most `maxPoints` by largest-triangle-three-buckets: each bucket keeps the
 * reading that stands out most from its neighbours, so peaks and dips survive where an average
 * would flatten them. A bucket with no finite reading stays a gap.
 */
export function thin(values: readonly number[]): readonly number[] {
  const count = values.length;
  if (count <= maxPoints) return values;
  const size = (count - 2) / (maxPoints - 2);
  const edge = (bucket: number) => Math.min(count - 1, Math.floor(bucket * size) + 1);
  const out = [values[0]];
  let kept = 0;
  for (let bucket = 0; bucket < maxPoints - 2; bucket += 1) {
    const [from, to] = [edge(bucket), edge(bucket + 1)];
    const next = bucket === maxPoints - 3 ? values[count - 1] : mean(values.slice(to, edge(bucket + 2)));
    const nextAt = bucket === maxPoints - 3 ? count - 1 : (to + edge(bucket + 2) - 1) / 2;
    let [best, area] = [-1, -1];
    for (let index = from; index < to; index += 1) {
      if (!Number.isFinite(values[index])) continue;
      const spread = Number.isFinite(values[kept]) && Number.isFinite(next)
        ? Math.abs((kept - nextAt) * (values[index] - values[kept]) - (kept - index) * (next - values[kept]))
        : 0;
      if (spread > area) [best, area] = [index, spread];
    }
    out.push(best < 0 ? NaN : values[best]);
    if (best >= 0) kept = best;
  }
  out.push(values[count - 1]);
  return out;
}

/** The value range drawn: the data's own, widened to hold zero when asked and to any bound given. */
export function domain(values: readonly number[], hasZero: boolean, min?: number, max?: number): readonly [number, number] {
  const finite = values.filter(Number.isFinite);
  let low = finite.length ? Math.min(...finite) : 0;
  let high = finite.length ? Math.max(...finite) : 0;
  if (hasZero) { low = Math.min(low, 0); high = Math.max(high, 0); }
  if (min !== undefined && Number.isFinite(min)) low = Math.min(low, min);
  if (max !== undefined && Number.isFinite(max)) high = Math.max(high, max);
  return [low, high];
}

/** A value's height in the box; a flat range sits on the middle line. */
export function scaleY(value: number, [low, high]: readonly [number, number]): number {
  return high > low ? ((high - value) / (high - low)) * box : box / 2;
}

/** Each run of consecutive finite values as points spread edge to edge; a gap ends a run. */
export function runs(values: readonly number[], range: readonly [number, number]): Point[][] {
  const step = values.length > 1 ? box / (values.length - 1) : 0;
  const out: Point[][] = [];
  let run: Point[] = [];
  values.forEach((value, index) => {
    if (Number.isFinite(value)) run.push([values.length > 1 ? index * step : box / 2, scaleY(value, range)]);
    else if (run.length) { out.push(run); run = []; }
  });
  if (run.length) out.push(run);
  return out;
}

const at = ([x, y]: Point) => `${round(x)} ${round(y)}`;

/** A point as a zero-length stroke, which a round cap draws as a dot of the stroke's width. */
export const dot = (point: Point) => `M${at(point)}h0.01`;

/** Straight segments through the points. */
export function polyline(points: readonly Point[]): string {
  return points.map((point, index) => `${index ? 'L' : 'M'}${at(point)}`).join('');
}

/**
 * A monotone cubic through the points (Fritsch–Carlson): between two points the curve stays
 * within their heights, so it never rises above or dips below the data it joins.
 */
export function monotone(points: readonly Point[]): string {
  if (points.length < 3) return polyline(points);
  const last = points.length - 1;
  const width = (index: number) => points[index + 1][0] - points[index][0];
  const slope = (index: number) => (points[index + 1][1] - points[index][1]) / width(index);
  // A turning point is flat; elsewhere the tangent is the mean of the slopes on either side.
  const tangent = points.map((_, index) => {
    if (index === 0) return slope(0);
    if (index === last) return slope(last - 1);
    const [before, after] = [slope(index - 1), slope(index)];
    return before * after <= 0 ? 0 : (before + after) / 2;
  });
  // Tangents too steep for their segment are pulled back onto the circle of radius 3, the
  // bound inside which a cubic cannot overshoot.
  for (let index = 0; index < last; index += 1) {
    const rise = slope(index);
    if (rise === 0) { tangent[index] = 0; tangent[index + 1] = 0; continue; }
    const [a, b] = [tangent[index] / rise, tangent[index + 1] / rise];
    const reach = Math.hypot(a, b);
    if (reach > 3) { tangent[index] = (3 / reach) * a * rise; tangent[index + 1] = (3 / reach) * b * rise; }
  }
  let d = `M${at(points[0])}`;
  for (let index = 0; index < last; index += 1) {
    const [x0, y0] = points[index];
    const [x1, y1] = points[index + 1];
    const third = (x1 - x0) / 3;
    d += `C${at([x0 + third, y0 + tangent[index] * third])} ${at([x1 - third, y1 - tangent[index + 1] * third])} ${at([x1, y1])}`;
  }
  return d;
}

/** A line closed down to `floor`, for the fill under it. */
export function closed(line: string, points: readonly Point[], floor: number): string {
  return `${line}L${round(points[points.length - 1][0])} ${round(floor)}L${round(points[0][0])} ${round(floor)}Z`;
}

/** One bar per finite value, from the zero line, each `share` of its slot wide. */
export function bars(values: readonly number[], range: readonly [number, number], share: number): string {
  const slot = box / values.length;
  const zero = scaleY(0, range);
  return values.map((value, index) => {
    if (!Number.isFinite(value)) return '';
    const y = scaleY(value, range);
    const x = slot * index + (slot * (1 - share)) / 2;
    return `M${round(x)} ${round(Math.min(y, zero))}h${round(slot * share)}v${round(Math.abs(y - zero))}h${round(-slot * share)}Z`;
  }).join('');
}

const centre = box / 2;
const polar = (radius: number, angle: number): Point => [centre + radius * Math.cos(angle), centre + radius * Math.sin(angle)];

/** A full ring or disc: two half arcs, since one arc cannot close on itself. */
export function circle(radius: number): string {
  const r = round(radius);
  return `M${round(centre - radius)} ${centre}a${r} ${r} 0 1 0 ${round(2 * radius)} 0a${r} ${r} 0 1 0 ${round(-2 * radius)} 0Z`;
}

/**
 * A slice from angle `from` to `to` (radians, clockwise from the top) between two radii. `gap` is
 * the distance kept clear between neighbours, the same at the rim and near the centre, so each
 * straight edge is moved in by half of it. A slice too thin for the gap is drawn without one.
 */
export function sector(inner: number, outer: number, from: number, to: number, gap: number): string {
  const top = -Math.PI / 2;
  const half = gap / 2;
  const sweep = to - from;
  const outerTrim = Math.asin(Math.min(1, half / outer));
  const trim = sweep > 2 * outerTrim + 0.02 ? half : 0;
  const edge = (radius: number) => (trim ? Math.asin(Math.min(1, trim / radius)) : 0);
  const [a0, a1] = [top + from + edge(outer), top + to - edge(outer)];
  const large = a1 - a0 > Math.PI ? 1 : 0;
  const rim = `M${at(polar(outer, a0))}A${round(outer)} ${round(outer)} 0 ${large} 1 ${at(polar(outer, a1))}`;
  const innerSweep = inner > trim ? sweep - 2 * edge(inner) : 0;
  if (innerSweep > 0) {
    const [b0, b1] = [top + from + edge(inner), top + to - edge(inner)];
    return `${rim}L${at(polar(inner, b1))}A${round(inner)} ${round(inner)} 0 ${b1 - b0 > Math.PI ? 1 : 0} 0 ${at(polar(inner, b0))}Z`;
  }
  // The two moved edges meet on the slice's middle line, short of the centre.
  const reach = trim && sweep < Math.PI ? Math.min(outer, trim / Math.sin(sweep / 2)) : Math.max(inner, 0);
  return `${rim}L${at(polar(reach, top + from + sweep / 2))}Z`;
}

const exact = (value: number) => Number(value.toPrecision(12));
const leading = (value: number, steps: readonly number[]) => {
  const scale = 10 ** Math.floor(Math.log10(value));
  return exact((steps.find(step => step >= value / scale - 1e-9) ?? 10) * scale);
};

/** The next round number at or above a positive value: 187 gives 200, 67 gives 80. */
export const roundUp = (value: number) => (value > 0 ? leading(value, [1, 1.2, 1.5, 2, 2.5, 3, 4, 5, 6, 8, 10]) : 0);

/** The range widened to round numbers about a quarter of its span apart: 70.5 to 72.4 gives 70.5 to 72.5. */
export function roundRange([low, high]: readonly [number, number]): readonly [number, number] {
  const span = high - low || Math.abs(high) || 1;
  const step = leading(span / 4, [1, 2, 2.5, 5, 10]);
  return [exact(Math.floor(low / step + 1e-9) * step), exact(Math.ceil(high / step - 1e-9) * step)];
}

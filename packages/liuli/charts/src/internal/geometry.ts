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
 * Steps through the points: each value holds level across its own point, and the line rises or
 * falls straight halfway to the next, so the first and the last value are as plain as the rest.
 */
export function stepped(points: readonly Point[]): string {
  if (points.length < 2) return polyline(points);
  const middles = points.slice(1).map((point, index) => round((points[index][0] + point[0]) / 2));
  return `M${at(points[0])}${points.slice(1).map((point, index) => `H${middles[index]}V${round(point[1])}`).join('')}H${round(points[points.length - 1][0])}`;
}

/** How a line goes from one point to the next. */
export type ChartCurve = 'straight' | 'smooth' | 'step';
/** The path through the points along `curve`. */
export function traced(points: readonly Point[], curve: ChartCurve): string {
  return curve === 'smooth' ? monotone(points) : curve === 'step' ? stepped(points) : polyline(points);
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


/**
 * Which of `count` evenly spaced labels to write when neighbours are `pitch` apart and a label
 * needs `need`: every one, or every nth counted from the `anchor` end, so a series in time keeps
 * its latest label. A step that reaches both ends is preferred when it is under twice the
 * smallest that fits.
 */
export function fitTicks(count: number, pitch: number, need: number, anchor: 'start' | 'end' = 'start'): number[] {
  if (count <= 0) return [];
  const least = Math.max(1, Math.ceil(need / Math.max(pitch, 1e-6) - 1e-9));
  const from = anchor === 'end' ? count - 1 : 0;
  if (least > count - 1) return [from];
  let step = least;
  for (let each = least; each <= Math.min(least * 2 - 1, count - 1); each++) if ((count - 1) % each === 0) { step = each; break; }
  const out: number[] = [];
  for (let index = from % step; index < count; index += step) out.push(index);
  return out;
}

/** The datum under a pointer `x` points into a row of `count` data `width` wide: points run edge to edge inside `inset`, slots share the width. */
export function indexAt(x: number, width: number, count: number, layout: 'points' | 'slots', inset = 0): number {
  const span = Math.max(1, width - inset * 2);
  const index = layout === 'points' ? Math.round(((x - inset) / span) * (count - 1)) : Math.floor((x / Math.max(1, width)) * count);
  return Math.min(count - 1, Math.max(0, index));
}

/**
 * The viewBox of a drawing whose data fills `box` and whose element is `width` by `height` points:
 * it reaches `inset` points past the data at the sides and top, and `foot` points below. A
 * drawing is cut at its viewBox, so this is the room a marker on an edge value needs to be whole.
 */
export function reach(width: number, height: number, inset: number, foot: number): [number, number, number, number] {
  const across = (inset * box) / Math.max(1, width - inset * 2);
  const tall = Math.max(1, height - inset - foot);
  const [up, down] = [(inset * box) / tall, (foot * box) / tall];
  return [-across, -up, box + across * 2, box + up + down].map(value => Math.round(value * 1000) / 1000) as [number, number, number, number];
}

/**
 * A scale for the values between `low` and `high`: ticks at round numbers about `target` apart
 * in count, inside a range that reaches a quarter of a step past the data at most, so the
 * plot is not left half empty under a round top.
 *
 * Range and ticks are counted in quarter steps, as whole numbers, so every tick is inside the
 * range by construction. `gaps` is the plot's height from the top down, in those quarter steps:
 * above the top tick, between each two, under the bottom one. They are small whole numbers
 * whatever the data's size, which is what a layout takes.
 */
export function niceTicks([low, high]: readonly [number, number], target: number): { ticks: number[]; range: readonly [number, number]; gaps: number[] } {
  if (!Number.isFinite(low) || !Number.isFinite(high)) [low, high] = [0, 0];
  if (high < low) [low, high] = [high, low];
  const span = high - low || Math.abs(high) || 1;
  const step = leading(span / Math.max(1, target), [1, 2, 2.5, 5, 10]);
  const fine = step / 4;
  const from = Math.floor(low / fine + 1e-9);
  const to = Math.max(from, Math.ceil(high / fine - 1e-9));
  const range = [exact(from * fine), exact(to * fine)] as const;
  // One value: one tick, on the middle line where a flat range is drawn.
  if (to === from) return { ticks: [range[0]], range, gaps: [1, 1] };
  const [first, last] = [Math.ceil(from / 4), Math.floor(to / 4)];
  // Too few round numbers fall inside a narrow range: its two ends are the scale.
  const marks = last > first ? Array.from({ length: last - first + 1 }, (_, index) => (first + index) * 4) : [from, to];
  const gaps = [to - marks[marks.length - 1], ...marks.slice(1).map((mark, index) => mark - marks[index]).reverse(), marks[0] - from];
  return { ticks: marks.map(mark => exact(mark * fine)), range, gaps };
}

/** A box in points, across from the plot's start edge and down from its top. */
export interface Extent { start: number; end: number; top: number; bottom: number }

/**
 * Where a line at height `y` breaks: one gap per slot, in order, so the line keeps one shape while
 * values change. A slot whose label the line's 1 pt stroke touches gets that label's span, `pad` wider
 * on both sides; any other slot an empty gap at its `middle`. Gaps never overlap.
 */
export function knockOuts(y: number, labels: ReadonlyArray<Extent | undefined>, middles: readonly number[], pad: number): Array<[number, number]> {
  let reached = -Infinity;
  return middles.map((middle, index) => {
    const label = labels[index];
    const isCrossed = label !== undefined && label.top - 0.5 < y && y < label.bottom + 0.5;
    const [start, end] = isCrossed ? [label.start - pad, label.end + pad] : [middle, middle];
    const gap: [number, number] = [Math.max(start, reached), Math.max(end, reached)];
    reached = gap[1];
    return gap;
  });
}

/**
 * Where a reference's label sits: above or below its line, `inset` points in from the end edge, with its
 * name or its value alone.
 */
export interface ReferencePlace { side: 'above' | 'below'; inset: number; isShort: boolean }

const overlaps = (a: Extent, b: Extent) => a.start < b.end && b.start < a.end && a.top < b.bottom && b.top < a.bottom;

/**
 * Each reference label, in order, where it covers no other label, nothing in `taken` and no edge of the
 * plot: at the end edge above its line, else below it; else slid along its line toward the start, just
 * clear of what is in the way; only then its value alone, tried the same way.
 * `lines` are the lines' heights in points; widths and the line height are the labels' measured sizes.
 */
export function placeReferences(
  lines: readonly number[], plot: { width: number; height: number }, label: { full: readonly number[]; short: readonly number[]; height: number },
  taken: readonly Extent[], gap: number,
): ReferencePlace[] {
  const placed: Extent[] = [...taken];
  return lines.map((y, index) => {
    const extent = (place: ReferencePlace): Extent => {
      const width = (place.isShort ? label.short : label.full)[index] ?? 0;
      const top = place.side === 'above' ? y - gap - label.height : y + gap;
      return { start: plot.width - place.inset - width, end: plot.width - place.inset, top, bottom: top + label.height };
    };
    const fits = (place: ReferencePlace) => {
      const box = extent(place);
      return box.start >= 0 && box.top >= 0 && box.bottom <= plot.height && !placed.some(other => overlaps(box, other));
    };
    // The insets that put the label's end just before something in its band, nearest the end edge first.
    const slides = (side: ReferencePlace['side'], isShort: boolean) => {
      const band = extent({ side, inset: 0, isShort });
      return placed.filter(other => other.top < band.bottom && band.top < other.bottom)
        .map(other => plot.width - other.start + gap).filter(inset => inset > 0).sort((a, b) => a - b)
        .map(inset => ({ side, inset, isShort }));
    };
    const tries = (isShort: boolean): ReferencePlace[] => [
      { side: 'above', inset: 0, isShort }, { side: 'below', inset: 0, isShort }, ...slides('above', isShort), ...slides('below', isShort),
    ];
    // Nothing fits: the value alone at the end edge, on the side the plot has room for.
    const place = [...tries(false), ...tries(true)].find(fits)
      ?? { side: extent({ side: 'above', inset: 0, isShort: true }).top >= 0 ? 'above' : 'below', inset: 0, isShort: true };
    placed.push(extent(place));
    return place;
  });
}

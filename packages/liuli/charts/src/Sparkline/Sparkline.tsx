import { cx, type VectorPath } from '@lingxia/view';
import { bars, box, closed, domain, dot, monotone, polyline, runs, thin, type Point } from '../internal/geometry';
import { sparklineRecipe as r } from './Sparkline.recipe';

export type SparklineVariant = 'line' | 'area' | 'bar';
export type SparklineSize = 'sm' | 'md' | 'lg';
export type SparklineTone = 'series' | 'neutral' | 'positive' | 'negative';

export interface SparklineProps {
  /** The numbers, oldest first. A value that is not finite (`NaN`) is a gap. Past 240 values it keeps the 240 that hold the shape, peaks and dips included. */
  values: readonly number[];
  /** What the chart shows, in words; it is all a screen reader gets: "Revenue, last 7 days: up from 12k to 24k". */
  label: string;
  /** `area` fills under the line; `bar` draws one bar per value. @default 'line' */
  variant?: SparklineVariant;
  /** Rounds the line between points without passing above or below them. Lines and areas only. @default false */
  isSmooth?: boolean;
  /** Marks the latest value with a dot. Lines and areas only. @default false */
  hasEndDot?: boolean;
  /** `series` is the first chart colour; `positive` and `negative` say the trend is good or bad. @default 'series' */
  tone?: SparklineTone;
  /** The lowest value drawn. A line fits its data; an area or bars start from zero. */
  domainMin?: number;
  /** The highest value drawn. */
  domainMax?: number;
  /** The height: 24, 32 or 48. In a column the width fills the container; in a row give it one with `className` (`w-20`, `grow`). @default 'md' */
  size?: SparklineSize;
  className?: string;
  'data-testid'?: string;
}

const toneRole = { series: 'chart-1', neutral: 'text-muted', positive: 'success', negative: 'danger' } as const satisfies Record<SparklineTone, string>;

const strokeWidth = 1.5;
const dotWidth = 5;
// Bars fill this share of their slot, the rest is the gap between them.
const barShare = 0.72;

/** A small chart without axes that shows the shape of a series beside a figure or in a row. */
export function Sparkline({
  values,
  label,
  variant = 'line',
  isSmooth = false,
  hasEndDot = false,
  tone = 'series',
  domainMin,
  domainMax,
  size = 'md',
  className,
  'data-testid': testId,
}: SparklineProps) {
  const role = toneRole[tone];
  const drawn = thin(values);
  const range = domain(drawn, variant !== 'line', domainMin, domainMax);
  const paths: VectorPath[] = [];
  if (variant === 'bar') {
    const d = bars(drawn, range, barShare);
    if (d) paths.push({ d, fill: role });
  } else {
    const stroke = { fill: 'none', stroke: role, strokeLinecap: 'round', strokeLinejoin: 'round', vectorEffect: 'non-scaling-stroke' } as const;
    const lines = runs(drawn, range);
    const trace = (points: readonly Point[]) => (isSmooth ? monotone(points) : polyline(points));
    const joined = lines.filter(points => points.length > 1);
    if (variant === 'area' && joined.length)
      paths.push({ d: joined.map(points => closed(trace(points), points, box)).join(''), fill: `${role}/16` });
    if (joined.length) paths.push({ d: joined.map(trace).join(''), strokeWidth, ...stroke });
    // A value with a gap on both sides has no segment, so it is drawn as a dot of the line's width.
    const alone = lines.filter(points => points.length === 1);
    if (alone.length) paths.push({ d: alone.map(points => dot(points[0])).join(''), strokeWidth: strokeWidth * 2, ...stroke });
    const latest = lines[lines.length - 1];
    if (hasEndDot && latest) paths.push({ d: dot(latest[latest.length - 1]), strokeWidth: dotWidth, ...stroke });
  }
  // A vector needs a path; with nothing to draw, the box keeps its place and its name.
  return (
    <view role="img" aria-label={label} data-testid={testId} className={cx(r.root.base, r.root.size[size], className)}>
      {/* A vector needs a path; with nothing to draw, the box keeps its place and its name. */}
      {paths.length > 0 && <vector aria-hidden paths={paths} viewBox={[0, 0, box, box]} className={cx(r.plot.base, r.plot.variant[variant])} />}
    </view>
  );
}

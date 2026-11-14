import { cx, type Child, type VectorPath } from '@lingxia/view';
import { Skeleton } from '@liulidesign/core/Skeleton';
import { statRecipe as r } from './Stat.recipe';

export type StatSize = 'sm' | 'md';
export type StatDirection = 'up' | 'down' | 'flat';
export type StatTone = 'neutral' | 'positive' | 'negative';

export interface StatProps {
  /** What the figure measures: "Revenue". Also names the group for assistive tech. */
  label: string;
  /** The figure as the app formats it: "$48,250", "3.2 s". Liuli formats no numbers. */
  value: string;
  /** The change since the last period, as the app formats it: "12.4%". Drawn after an arrow. */
  delta?: string;
  /** Which way the figure moved; picks the arrow. @default 'flat' */
  deltaDirection?: StatDirection;
  /** Whether the change is good news. By default up is positive, down negative and flat neutral; set it when up is bad (errors, cost). */
  deltaTone?: StatTone;
  /** The change in words, read in place of the arrow and `delta`: "Up 12.4% from last month". */
  deltaLabel?: string;
  /** A short note after the change: "vs last month". */
  description?: string;
  /** A small chart under the figure, usually a Sparkline. */
  chart?: Child;
  /** The figure's size: title or display type. @default 'md' */
  size?: StatSize;
  /** Stands a skeleton in for the figure and leaves out the change and the chart; the group is reported busy. @default false */
  isLoading?: boolean;
  className?: string;
  /** On the group. */
  'data-testid'?: string;
}

const arrow = { fill: 'none', stroke: 'currentColor', strokeWidth: 1.5, strokeLinecap: 'round', strokeLinejoin: 'round' } as const;
const glyphs = {
  up: [{ d: 'M6 9.75v-7.5M2.75 5.5 6 2.25 9.25 5.5', ...arrow }],
  down: [{ d: 'M6 2.25v7.5M2.75 6.5 6 9.75 9.25 6.5', ...arrow }],
  flat: [{ d: 'M2.5 6h7', ...arrow }],
} satisfies Record<StatDirection, VectorPath[]>;
const glyphBox = [0, 0, 12, 12] as const;
const toneOf = { up: 'positive', down: 'negative', flat: 'neutral' } as const satisfies Record<StatDirection, StatTone>;

/** One figure with its label, and optionally how it changed and a small chart of its history. */
export function Stat({
  label,
  value,
  delta,
  deltaDirection = 'flat',
  deltaTone,
  deltaLabel,
  description,
  chart,
  size = 'md',
  isLoading = false,
  className,
  'data-testid': testId,
}: StatProps) {
  const hasDelta = delta !== undefined && !isLoading;
  return (
    <view role="group" aria-label={label} aria-busy={isLoading || undefined} data-testid={testId} className={cx(r.root.base, className)}>
      {/* The group carries the label as its name, so it is not read a second time. */}
      <text aria-hidden className={r.label.base}>{label}</text>
      {isLoading ? <Skeleton shape="block" className={r.skeleton.size[size]} /> : <text className={cx(r.value.base, r.value.size[size])}>{value}</text>}
      {(hasDelta || description !== undefined) && (
        <view className={r.change.base}>
          {hasDelta && (
            <view className={cx(r.delta.base, r.delta.tone[deltaTone ?? toneOf[deltaDirection]])}>
              <vector aria-hidden paths={glyphs[deltaDirection]} viewBox={glyphBox} className={r.deltaGlyph.base} />
              <text aria-hidden={deltaLabel !== undefined || undefined} className={r.deltaText.base}>{delta}</text>
              {deltaLabel !== undefined && <text className={r.deltaSpoken.base}>{deltaLabel}</text>}
            </view>
          )}
          {description !== undefined && <text className={r.description.base}>{description}</text>}
        </view>
      )}
      {chart !== undefined && !isLoading && <view className={r.chart.base}>{chart}</view>}
    </view>
  );
}

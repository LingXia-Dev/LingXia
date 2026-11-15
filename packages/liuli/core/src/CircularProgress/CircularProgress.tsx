import { cx, type VectorPath } from '@lingxia/view';
import type { ProgressBarTone } from '../ProgressBar/ProgressBar';
import { circularProgressRecipe as r } from './CircularProgress.recipe';

export type CircularProgressSize = 'sm' | 'md' | 'lg';

export interface CircularProgressProps {
  /** The accessible name: what is in progress ("Uploading", "Daily goal"). Not drawn. */
  label: string;
  /** The amount done, from 0 to `max`. Leave it out while the amount is unknown. */
  value?: number;
  /** @default 100 */
  max?: number;
  /** The amount as the app words it ("72%", "3 of 8"): read in place of a percentage, and drawn in the middle at `md` and `lg`. */
  valueLabel?: string;
  /** 24, 48 or 80 px. `sm` has no room for words. @default 'md' */
  size?: CircularProgressSize;
  /** The arc's colour, for a ring that is a meter. @default 'accent' */
  tone?: ProgressBarTone;
  className?: string;
  'data-testid'?: string;
}

const box = [0, 0, 24, 24] as const;
// A thinner stroke on a larger ring keeps the weight even across sizes.
const weight = { sm: 3, md: 2.5, lg: 2 } as const;
const radius = 10;

const stroke = (d: string, width: number): VectorPath[] => [{ d, fill: 'none', stroke: 'currentColor', strokeWidth: width, strokeLinecap: 'round' }];
const circle = `M12 ${12 - radius}a${radius} ${radius} 0 1 1 0 ${2 * radius}a${radius} ${radius} 0 1 1 0 ${-2 * radius}`;

/** An arc from twelve o'clock, clockwise, over `part` of the ring. */
function arc(part: number): string {
  if (part >= 1) return circle;
  const angle = 2 * Math.PI * part;
  const x = 12 + radius * Math.sin(angle);
  const y = 12 - radius * Math.cos(angle);
  return `M12 ${12 - radius}A${radius} ${radius} 0 ${part > 0.5 ? 1 : 0} 1 ${x.toFixed(3)} ${y.toFixed(3)}`;
}

/** How much of a task is done, as a ring: for a tile, a row or a button where a bar has no room. */
export function CircularProgress({ label, value, max = 100, valueLabel, size = 'md', tone = 'accent', className, 'data-testid': testId }: CircularProgressProps) {
  const now = value === undefined ? undefined : Math.min(max, Math.max(0, value));
  return (
    <view
      role="progressbar"
      aria-label={label}
      aria-valuenow={now}
      aria-valuemin={now === undefined ? undefined : 0}
      aria-valuemax={now === undefined ? undefined : max}
      aria-valuetext={now === undefined ? undefined : valueLabel}
      data-testid={testId}
      className={cx(r.root.base, r.root.size[size], className)}
    >
      <vector aria-hidden paths={stroke(circle, weight[size])} viewBox={box} className={r.track.base} />
      {now === undefined ? (
        <vector aria-hidden paths={stroke(arc(0.25), weight[size])} viewBox={box} className={cx(r.fill.base, r.fill.tone[tone], r.fill.indeterminate)} />
      ) : (
        now > 0 && <vector aria-hidden paths={stroke(arc(now / max), weight[size])} viewBox={box} className={cx(r.fill.base, r.fill.tone[tone])} />
      )}
      {size !== 'sm' && now !== undefined && valueLabel !== undefined && <text aria-hidden className={cx(r.value.base, r.value.size[size])}>{valueLabel}</text>}
    </view>
  );
}

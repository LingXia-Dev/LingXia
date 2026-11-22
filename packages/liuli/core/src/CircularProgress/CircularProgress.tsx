import { childrenToArray, createContext, cx, useBox, useContext, type Box, type Child, type VectorPath } from '@lingxia/view';
import type { ProgressAmount, ProgressBarTone } from '../ProgressBar/ProgressBar';
import { amount } from '../internal/amount';
import { circularProgressRecipe as r } from './CircularProgress.recipe';
import { dataOf } from '../internal/data';

export type CircularProgressSize = 'sm' | 'md' | 'lg';

export type CircularProgressProps = ProgressAmount & CircularProgressOwnProps;

interface CircularProgressOwnProps {
  /** The accessible name: what is in progress ("Uploading", "Daily goal"). Not drawn. */
  label: string;
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
const side = { sm: 24, md: 48, lg: 80 } as const;
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

/** A ring in a group measures its words with the group's box and follows the group's choice. */
interface Slot { words: Box; beside: boolean }
const SlotContext = createContext<Slot | null>(null);
// Each measured box's middle, written as a ring renders: the group reads it once the box has a size.
const holes = new WeakMap<Box, number>();
const fits = (words: Box) => words.value === null || !holes.has(words) || Math.hypot(words.value.width, words.value.height) <= holes.get(words)!;

/** How much of a task is done, as a ring: for a tile, a row or a button where a bar has no room. */
export function CircularProgress({ label, value, max: ownMax = 100, progress, valueLabel, size = 'md', tone = 'accent', className, 'data-testid': testId, ...attributes }: CircularProgressProps) {
  const { now, max } = amount(value, ownMax, progress);
  const slot = useContext(SlotContext);
  const own = useBox('words');
  const words = slot?.words ?? own;
  const hasWords = size !== 'sm' && now !== undefined && valueLabel !== undefined;
  // The words stay in the middle while their box fits inside the stroke; at large text they move beside the ring.
  const hole = (side[size] * (2 * radius - weight[size])) / 24;
  if (hasWords) holes.set(words, hole); else holes.delete(words);
  const inside = slot ? !slot.beside : fits(words);
  const text = hasWords && <text aria-hidden className={cx(r.value.base, r.value.size[size])}>{valueLabel}</text>;
  return (
    <view
      role="progressbar"
      aria-label={label}
      aria-valuenow={now}
      aria-valuemin={now === undefined ? undefined : 0}
      aria-valuemax={now === undefined ? undefined : max}
      aria-valuetext={now === undefined ? undefined : valueLabel}
      data-testid={testId} {...dataOf(attributes)}
      className={cx(r.root.base, className)}
    >
      <view className={cx(r.ring.base, r.ring.size[size])}>
        <vector aria-hidden paths={stroke(circle, weight[size])} viewBox={box} className={r.track.base} />
        {now === undefined ? (
          <vector aria-hidden paths={stroke(arc(0.25), weight[size])} viewBox={box} className={cx(r.fill.base, r.fill.tone[tone], r.fill.indeterminate)} />
        ) : (
          now > 0 && <vector aria-hidden paths={stroke(arc(now / max), weight[size])} viewBox={box} className={cx(r.fill.base, r.fill.tone[tone])} />
        )}
        {hasWords && <text aria-hidden box={words} className={cx(r.value.base, r.value.size[size], r.value.ghost)}>{valueLabel}</text>}
        {inside && text}
      </view>
      {!inside && text}
    </view>
  );
}

export interface CircularProgressGroupProps {
  /** The rings, each a direct child. */
  children: Child;
  /** Lays the rings out: `flex-row flex-wrap gap-5` for a row. */
  className?: string;
  'data-testid'?: string;
}

/** Rings shown together: when one ring's words outgrow its middle, every ring in the group sets its words beside, so a row never mixes the two. */
export function CircularProgressGroup({ children, className, 'data-testid': testId, ...attributes }: CircularProgressGroupProps) {
  const rings = childrenToArray(children).filter(ring => typeof ring === 'object');
  const boxes = rings.map((_, i) => useBox(`words-${i}`));
  const beside = boxes.some(words => !fits(words));
  return (
    <view data-testid={testId} {...dataOf(attributes)} className={className}>
      {rings.map((ring, i) => <SlotContext.Provider key={String(i)} value={{ words: boxes[i], beside }}>{ring}</SlotContext.Provider>)}
    </view>
  );
}

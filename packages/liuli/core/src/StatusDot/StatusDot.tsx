import { cx } from '@lingxia/view';
import type { Hue } from '../hue';
import { statusDotRecipe as r } from './StatusDot.recipe';
import { dataOf } from '../internal/data';

export type StatusDotTone = 'success' | 'warning' | 'danger' | 'info' | 'accent' | 'neutral';
export type StatusDotSize = 'sm' | 'md';

export interface StatusDotProps {
  /** The status in words: "Online", "Degraded", "Paused". Shown beside the dot, and what is read. */
  label: string;
  /** Hides the words, keeping them as the dot\'s name: only where the same words are already beside it. @default false */
  isLabelHidden?: boolean;
  /** @default 'neutral' */
  tone?: StatusDotTone;
  /** A category's colour in place of a tone, one of the theme's eight hues: which calendar, which project. Not for a status. */
  hue?: Hue;
  /** @default 'md' */
  size?: StatusDotSize;
  /** A halo that keeps ringing out from the dot, for a state that is live right now: recording, on a call. Still when motion is reduced. @default false */
  isPulsing?: boolean;
  className?: string;
  'data-testid'?: string;
}

/** How something stands, as a small coloured dot and a word: a service, a device, a record. */
export function StatusDot({ label, isLabelHidden = false, tone = 'neutral', hue, size = 'md', isPulsing = false, className, 'data-testid': testId, ...attributes }: StatusDotProps) {
  const colour = hue !== undefined ? r.dot.hue[hue] : r.dot.tone[tone];
  const halo = isPulsing && <view aria-hidden className={cx(r.halo.base, colour)} />;
  const dot = <view aria-hidden={!isLabelHidden || undefined} className={cx(r.dot.base, r.dot.size[size], colour)}>{halo}</view>;
  if (isLabelHidden) {
    return <view role="img" aria-label={label} data-testid={testId} {...dataOf(attributes)} className={cx(r.dot.base, r.dot.size[size], colour, className)}>{halo}</view>;
  }
  return (
    <view data-testid={testId} {...dataOf(attributes)} className={cx(r.root.base, r.root.size[size], className)}>
      {dot}
      <text className={cx(r.label.base, r.label.size[size])}>{label}</text>
    </view>
  );
}

import { cx } from '@lingxia/view';
import { statusDotRecipe as r } from './StatusDot.recipe';

export type StatusDotTone = 'success' | 'warning' | 'danger' | 'accent' | 'neutral';
export type StatusDotSize = 'sm' | 'md';

export interface StatusDotProps {
  /** The status in words: "Online", "Degraded", "Paused". Shown beside the dot, and what is read. */
  label: string;
  /** Hides the words, keeping them as the dot\'s name: only where the same words are already beside it. @default false */
  isLabelHidden?: boolean;
  /** @default 'neutral' */
  tone?: StatusDotTone;
  /** @default 'md' */
  size?: StatusDotSize;
  className?: string;
  'data-testid'?: string;
}

/** How something stands, as a small coloured dot and a word: a service, a device, a record. */
export function StatusDot({ label, isLabelHidden = false, tone = 'neutral', size = 'md', className, 'data-testid': testId }: StatusDotProps) {
  const dot = <view aria-hidden={!isLabelHidden || undefined} className={cx(r.dot.base, r.dot.size[size], r.dot.tone[tone])} />;
  if (isLabelHidden) {
    return <view role="img" aria-label={label} data-testid={testId} className={cx(r.dot.base, r.dot.size[size], r.dot.tone[tone], className)} />;
  }
  return (
    <view data-testid={testId} className={cx(r.root.base, r.root.size[size], className)}>
      {dot}
      <text className={cx(r.label.base, r.label.size[size])}>{label}</text>
    </view>
  );
}

import { cx, type Child } from '@lingxia/view';
import { badgeRecipe as r } from './Badge.recipe';

export type BadgeStatusTone = 'accent' | 'success' | 'warning' | 'danger';
export type BadgeTone = 'neutral' | BadgeStatusTone;
export type BadgeSize = 'sm' | 'md';

/** Neutral is soft only; the other tones also come solid. */
export type BadgeLook =
  | {
      /** What the badge marks; `neutral` comes soft only. @default 'neutral' */
      tone?: 'neutral';
      emphasis?: 'soft';
    }
  | {
      tone: BadgeStatusTone;
      /** `solid` for a badge that must stand out; at most one per view. @default 'soft' */
      emphasis?: 'soft' | 'solid';
    };

export type BadgeProps = BadgeLook & {
  /** The text, read as it is shown. Say the status in words: "Overdue", not a colour. */
  label: string;
  /** @default 'md' */
  size?: BadgeSize;
  /** Drawn before the label and hidden from assistive tech. */
  startIcon?: Child;
  className?: string;
  'data-testid'?: string;
};

/** A short status or category label. */
export function Badge(props: BadgeProps) {
  const { label, size = 'md', startIcon, className, 'data-testid': testId } = props;
  const look = props.tone === undefined || props.tone === 'neutral'
    ? r.root.soft.neutral
    : props.emphasis === 'solid' ? r.root.solid[props.tone] : r.root.soft[props.tone];
  return (
    <view data-testid={testId} className={cx(r.root.base, r.root.size[size], look, className)}>
      {startIcon !== undefined && <view aria-hidden className={cx(r.icon.base, r.icon.size[size])}>{startIcon}</view>}
      <text className={cx(r.label.base, r.label.size[size])}>{label}</text>
    </view>
  );
}

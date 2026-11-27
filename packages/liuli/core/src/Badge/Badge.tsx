import { cx, useEnv } from '@lingxia/view';
import type { Hue } from '../hue';
import type { IconRef } from '../Icon/Icon';
import { Glyph } from '../internal/Glyph';
import { badgeRecipe as r } from './Badge.recipe';
import { dataOf } from '../internal/data';

export type BadgeStatusTone = 'accent' | 'info' | 'success' | 'warning' | 'danger';
export type BadgeTone = 'neutral' | BadgeStatusTone;
export type BadgeSize = 'sm' | 'md';

/** Neutral is soft only; the other tones also come solid. A hue is a category, not a status, and takes no tone. */
export type BadgeLook =
  | {
      /** What the badge marks; `neutral` comes soft only. @default 'neutral' */
      tone?: 'neutral';
      emphasis?: 'soft';
      hue?: never;
    }
  | {
      tone: BadgeStatusTone;
      /** `solid` for a badge that must stand out; at most one per view. @default 'soft' */
      emphasis?: 'soft' | 'solid';
      hue?: never;
    }
  | {
      /** A category's colour, one of the theme's eight hues: a dot in the hue on its soft fill. For kinds of things (a project, a label), never a status. */
      hue: Hue;
      tone?: never;
      emphasis?: never;
    };

export type BadgeProps = BadgeLook & {
  /** The text, read as it is shown. Say the status in words: "Overdue", not a colour. */
  label: string;
  /** @default 'md' */
  size?: BadgeSize;
  /** An icon by name from the set, or a definition, before the label; hidden from assistive tech. */
  startIcon?: IconRef;
  className?: string;
  'data-testid'?: string;
};

/** A short status or category label. */
export function Badge(props: BadgeProps) {
  const { label, size = 'md', startIcon, className, 'data-testid': testId } = props;
  const hue = props.hue;
  const { highContrast } = useEnv();
  // Only a status tone comes solid, so `emphasis: 'solid'` alone says the badge is solid.
  const soft = hue !== undefined || props.emphasis !== 'solid';
  const look = hue !== undefined
    ? r.root.hue[hue]
    : props.tone === undefined || props.tone === 'neutral'
      ? r.root.soft.neutral
      : props.emphasis === 'solid' ? r.root.solid[props.tone] : r.root.soft[props.tone];
  return (
    <view data-testid={testId} {...dataOf(props)} className={cx(r.root.base, r.root.size[size], look, highContrast && soft && r.root.highContrast, className)}>
      {startIcon !== undefined && <Glyph name={startIcon} side={size === 'sm' ? 12 : 14} className={cx(r.icon.base, r.icon.size[size], hue !== undefined && r.icon.hue[hue])} />}
      {startIcon === undefined && hue !== undefined && <view aria-hidden className={cx(r.dot.base, r.dot.size[size], r.dot.hue[hue])} />}
      <text className={cx(r.label.base, r.label.size[size])}>{label}</text>
    </view>
  );
}

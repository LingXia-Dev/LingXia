import { cx } from '@lingxia/view';
import { dividerRecipe as r } from './Divider.recipe';
import { dataOf } from '../internal/data';

export type DividerOrientation = 'horizontal' | 'vertical';
export type DividerInset = 'none' | 'start' | 'both';

export interface DividerProps {
  /** `vertical` parts items in a row and takes their height. @default 'horizontal' */
  orientation?: DividerOrientation;
  /** Sets the line in from the edge: `start` to line up with text after an icon, `both` inside a padded card. Horizontal only. @default 'none' */
  inset?: DividerInset;
  /** A word in the middle of the line: "or". Horizontal only. */
  label?: string;
  className?: string;
  'data-testid'?: string;
}

/** A hairline between two things that belong apart. */
export function Divider({ orientation = 'horizontal', inset = 'none', label, className, 'data-testid': testId, ...attributes }: DividerProps) {
  if (orientation === 'vertical') return <view aria-hidden data-testid={testId} {...dataOf(attributes)} className={cx(r.line.vertical, className)} />;
  if (label === undefined) return <view aria-hidden data-testid={testId} {...dataOf(attributes)} className={cx(r.line.horizontal, r.inset[inset], className)} />;
  return (
    <view data-testid={testId} {...dataOf(attributes)} className={cx(r.labelled.base, r.inset[inset], className)}>
      <view aria-hidden className={r.line.side} />
      <text className={r.label.base}>{label}</text>
      <view aria-hidden className={r.line.side} />
    </view>
  );
}

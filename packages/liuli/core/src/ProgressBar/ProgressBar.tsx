import { cx } from '@lingxia/view';
import { spinner, spinnerBox } from '../internal/spinner';
import { progressBarRecipe as r } from './ProgressBar.recipe';

export type ProgressBarSize = 'sm' | 'md';
/** What the progress stands for: `success` for a goal met, `warning` for a limit near, `danger` for a limit passed. */
export type ProgressBarTone = 'accent' | 'success' | 'warning' | 'danger';
export type SpinnerSize = 'sm' | 'md' | 'lg';

export interface ProgressBarProps {
  /** The visible label and the accessible name: what is in progress ("Uploading photos"). */
  label: string;
  /** Hides the label row; `label` stays the name. @default false */
  isLabelHidden?: boolean;
  /** The amount done, from 0 to `max`. Leave it out while the amount is unknown. */
  value?: number;
  /** @default 100 */
  max?: number;
  /** The amount as the app words it ("3 of 8 files"), shown after the label and read in place of a percentage. */
  valueLabel?: string;
  /** The track's thickness. @default 'md' */
  size?: ProgressBarSize;
  /** The fill's colour, for a bar that is a meter: storage, a quota, a goal. Say the state in `valueLabel` too. @default 'accent' */
  tone?: ProgressBarTone;
  className?: string;
  /** On the progress bar. */
  'data-testid'?: string;
}

/** How much of a task is done, or that it is running when the amount is unknown. */
export function ProgressBar({
  label,
  isLabelHidden = false,
  value,
  max = 100,
  valueLabel,
  size = 'md',
  tone = 'accent',
  className,
  'data-testid': testId,
}: ProgressBarProps) {
  const now = value === undefined ? undefined : Math.min(max, Math.max(0, value));
  return (
    <view className={cx(r.root.base, className)}>
      {!isLabelHidden && (
        // The bar carries the label and the value, so the row is not read again.
        <view aria-hidden className={r.header.base}>
          <text className={r.label.base}>{label}</text>
          {valueLabel !== undefined && now !== undefined && <text className={r.value.base}>{valueLabel}</text>}
        </view>
      )}
      <view
        role="progressbar"
        aria-label={label}
        aria-valuenow={now}
        aria-valuemin={now === undefined ? undefined : 0}
        aria-valuemax={now === undefined ? undefined : max}
        aria-valuetext={now === undefined ? undefined : valueLabel}
        data-testid={testId}
        className={cx(r.track.base, r.track.size[size])}
      >
        {now === undefined ? (
          <view className={cx(r.fill.base, r.fill.tone[tone], r.fill.indeterminate)} />
        ) : (
          <view className={cx(r.fill.base, r.fill.tone[tone], r.fill.determinate)} style={{ width: `${(now / max) * 100}%` }} />
        )}
      </view>
    </view>
  );
}

export interface SpinnerProps {
  /** The accessible name: what is loading. Not drawn. */
  label: string;
  /** @default 'md' */
  size?: SpinnerSize;
  className?: string;
  'data-testid'?: string;
}

/** A spinning arc for a wait whose length is unknown. */
export function Spinner({ label, size = 'md', className, 'data-testid': testId }: SpinnerProps) {
  return (
    <view role="progressbar" aria-label={label} data-testid={testId} className={className}>
      <vector aria-hidden paths={spinner} viewBox={spinnerBox} className={cx(r.spinner.base, r.spinner.size[size])} />
    </view>
  );
}

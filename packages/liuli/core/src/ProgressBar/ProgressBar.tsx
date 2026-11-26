import { cx, useBox, useView } from '@lingxia/view';
import { amount } from '../internal/amount';
import { SpinnerMark } from '../internal/spinner';
import { progressBarRecipe as r } from './ProgressBar.recipe';
import { dataOf } from '../internal/data';

export type ProgressBarSize = 'sm' | 'md';
/** What the progress stands for: `success` for a goal met, `warning` for a limit near, `danger` for a limit passed. */
export type ProgressBarTone = 'accent' | 'success' | 'warning' | 'danger';
export type SpinnerSize = 'sm' | 'md' | 'lg' | 'xl';
/** `muted` beside quiet text; `on-media` over a picture or a video. */
export type SpinnerTone = 'accent' | 'muted' | 'on-media';

/** A transfer's progress as Logic streams it: bytes `loaded` of `total`, and `fraction` from 0 to 1 while the total is known. */
export interface TransferProgress {
  readonly loaded: number;
  readonly total?: number;
  readonly fraction?: number;
}

/** The amount as numbers, or as a transfer's progress; not both. */
export type ProgressAmount =
  | {
      /** The amount done, from 0 to `max`. Leave it out while the amount is unknown. */
      value?: number;
      /** @default 100 */
      max?: number;
      progress?: never;
    }
  | {
      /**
       * A transfer's latest progress, such as `useStream(...).data`: determinate while it has a `fraction`, unknown while
       * it has none or is `null` before the first value. The accessible value is in bytes when the total is known.
       */
      progress: TransferProgress | null;
      value?: never;
      max?: never;
    };

export type ProgressBarProps = ProgressAmount & ProgressBarOwnProps;

interface ProgressBarOwnProps {
  /** The visible label and the accessible name: what is in progress ("Uploading photos"). */
  label: string;
  /** Hides the label row; `label` stays the name. @default false */
  isLabelHidden?: boolean;
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

// A dash is three track heights, up to 12 dashes; past that they lengthen with the track.
const dashLength = { sm: 12, md: 24 } as const satisfies Record<ProgressBarSize, number>;
const mostDashes = 12;
const keys = (count: number) => Array.from({ length: count }, (_, index) => `dash-${index}`);

/** Whole dashes, half a dash apart, that fill `width` exactly. */
function dashesFor(width: number, size: ProgressBarSize) {
  const count = Math.min(mostDashes, Math.max(1, Math.round((width + dashLength[size] / 2) / (1.5 * dashLength[size]))));
  return { count, dash: (2 * width) / (3 * count - 1) };
}

/** How much of a task is done, or that it is running when the amount is unknown. */

export function ProgressBar({
  label,
  isLabelHidden = false,
  value,
  max: ownMax = 100,
  progress,
  valueLabel,
  size = 'md',
  tone = 'accent',
  className,
  'data-testid': testId, ...attributes
}: ProgressBarProps) {
  const { now, max } = amount(value, ownMax, progress);
  const track = useBox('track');
  const direction = useView().env.direction === 'rtl' ? 'rtl' : 'ltr';
  // Nothing until the track is measured, so no other pattern shows first.
  const width = now === undefined ? track.value?.width ?? 0 : 0;
  const { count, dash } = dashesFor(width, size);
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
        data-testid={testId} {...dataOf(attributes)}
        box={track}
        className={cx(r.track.base, r.track.size[size])}
      >
        {now === undefined ? (
          width > 0 && (
            // One period wide, so the march moves one period whatever the dash length. The extra dash
            // enters as it marches; still, it lies past the clipped end.
            <view className={r.dashes.march[direction]} style={{ width: 1.5 * dash }}>
              <view className={r.dashes.row} style={{ width: width + 1.5 * dash, gap: dash / 2 }}>
                {keys(count + 1).map(key => <view key={key} className={cx(r.dashes.dash, r.fill.tone[tone])} style={{ width: dash }} />)}
              </view>
            </view>
          )
        ) : (
          <view className={cx(r.fill.base, r.fill.tone[tone], r.fill.determinate)} style={{ width: `${(now / max) * 100}%` }} />
        )}
      </view>
    </view>
  );
}

export interface SpinnerProps {
  /** The accessible name: what is loading, drawn under the ring unless `isLabelHidden`. */
  label: string;
  /** Draws the ring alone, the label read only: for a spinner inside a control or a row. Leave the label shown for a page, a panel or a list that has room to say what. @default false */
  isLabelHidden?: boolean;
  /** 16, 24, 32 or 48 px; `xl` for a whole page or a media frame that is loading. @default 'md' */
  size?: SpinnerSize;
  /** @default 'accent' */
  tone?: SpinnerTone;
  className?: string;
  'data-testid'?: string;
}

/** A spinning arc for a wait whose length is unknown. */
export function Spinner({ label, isLabelHidden = false, size = 'md', tone = 'accent', className, 'data-testid': testId, ...attributes }: SpinnerProps) {
  return (
    <view role="progressbar" aria-label={label} data-testid={testId} {...dataOf(attributes)} className={cx(!isLabelHidden && r.loading.base, className)}>
      <SpinnerMark ring={tone === 'on-media' ? 'on-media/30' : undefined} className={cx(r.spinner.base, r.spinner.tone[tone], r.spinner.size[size])} />
      {/* Read once, as the name; the drawn words are for the eye. */}
      {!isLabelHidden && <text aria-hidden className={cx(r.loading.label, r.loading.tone[tone])}>{label}</text>}
    </view>
  );
}

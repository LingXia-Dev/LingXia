import { cx, type Local } from '@lingxia/view';
import { segmentedControlRecipe as r } from './SegmentedControl.recipe';

export type SegmentedControlSize = 'sm' | 'md';

export interface SegmentedOption {
  /** What `value` and `onChange` carry. */
  value: string;
  /** The segment's text and accessible name. */
  label: string;
  /** @default false */
  isDisabled?: boolean;
}

export type SegmentedControlBinding =
  | {
      /** A View-local cell the control reads and writes; use it or `value` with `onChange`. */
      bind: Local<string>;
      value?: never;
      onChange?: never;
    }
  | {
      bind?: never;
      /** The chosen segment's value. */
      value: string;
      /** Runs with the segment chosen by a tap or by arrowing onto it. */
      onChange: (value: string) => void;
    };

export type SegmentedControlProps = SegmentedControlBinding & {
  /** Names the group for assistive tech: "Units", "Sort". */
  label: string;
  /** Two to five choices, in order; they share the row equally. */
  items: readonly SegmentedOption[];
  /** @default 'md' */
  size?: SegmentedControlSize;
  className?: string;
  /** On the group; each segment gets `<testid>-<value>`. */
  'data-testid'?: string;
};

/**
 * One value from a few, as a row of segments with a thumb on the chosen one. A radio group: one Tab
 * stop, arrows move and choose on arrival. It sets a value; to switch what a panel shows, use Tabs.
 */
export function SegmentedControl(props: SegmentedControlProps) {
  const { label, items, size = 'md', className, 'data-testid': testId } = props;
  const chosen = props.bind ? props.bind.value : props.value;
  const index = Math.max(0, items.findIndex(option => option.value === chosen));
  const choose = (value: string) => {
    if (value === chosen) return;
    if (props.bind) props.bind.set(value);
    else props.onChange(value);
  };
  return (
    <view role="radiogroup" aria-label={label} data-testid={testId} className={cx(r.root.base, className)}>
      {/* One box a segment wide, moved by its own width per segment, in a left-to-right box that
          right-to-left mirrors. */}
      <view aria-hidden className={r.track.base}>
        <view dir="ltr" className={r.track.inner}>
          <view className={r.thumb.base} style={{ width: `${100 / Math.max(1, items.length)}%`, translate: [`${index * 100}%`, 0] }} />
        </view>
      </view>
      {items.map(option => {
        const isChecked = option.value === items[index]?.value;
        return (
          <button
            key={option.value}
            role="radio"
            aria-checked={isChecked}
            aria-label={option.label}
            data-state={isChecked ? 'checked' : 'unchecked'}
            disabled={option.isDisabled}
            data-testid={testId && `${testId}-${option.value}`}
            onClick={() => choose(option.value)}
            className={cx(r.segment.base, r.segment.size[size])}
          >
            <text aria-hidden className={cx(r.label.base, r.label.size[size], isChecked ? r.label.chosen : r.label.other)}>{option.label}</text>
          </button>
        );
      })}
    </view>
  );
}

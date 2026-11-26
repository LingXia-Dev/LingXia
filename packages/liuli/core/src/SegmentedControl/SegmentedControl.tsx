import { cx, useBox, type Local } from '@lingxia/view';
import type { IconRef } from '../Icon/Icon';
import { Glyph } from '../internal/Glyph';
import { segmentedControlRecipe as r } from './SegmentedControl.recipe';
import { dataOf } from '../internal/data';

export type SegmentedControlSize = 'sm' | 'md';

export type SegmentedOption = {
  /** What `value` and `onChange` carry. */
  value: string;
  /** The segment's text and accessible name; with `isLabelHidden`, its name alone. */
  label: string;
  /** @default false */
  isDisabled?: boolean;
} & (
  | {
      /** An icon by name from the set, or a definition, before the label. */
      icon?: IconRef;
      isLabelHidden?: false;
    }
  | {
      icon: IconRef;
      /** Shows the icon alone, for a view switcher; the label still names the segment. @default false */
      isLabelHidden: true;
    }
);

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
  /** Two to five choices, in order; they share the row equally, and stack when it cannot hold them. */
  items: readonly SegmentedOption[];
  /** @default 'md' */
  size?: SegmentedControlSize;
  className?: string;
  /** On the group; each segment gets `<testid>-<value>`. */
  'data-testid'?: string;
};

// A segment's side padding, both sides, in points: what its measured content leaves out (`px-2`, `px-3`).
const iconSide: Record<SegmentedControlSize, number> = { sm: 16, md: 20 };
const segmentPadding: Record<SegmentedControlSize, number> = { sm: 16, md: 24 };

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
  const share = 100 / Math.max(1, items.length);
  // Labels keep their width in both layouts, so they decide; segments place the thumb in a row.
  const root = useBox('root');
  const labelBoxes = items.map((_, at) => useBox(`label-${at}`));
  const segmentBoxes = items.map((_, at) => useBox(`segment-${at}`));
  const labels = labelBoxes.map(box => box.value?.width);
  const extents = segmentBoxes.map(box => box.value?.width);
  const room = root.value?.width;
  const isStacked = room !== undefined && labels.every(each => each !== undefined)
    && labels.reduce<number>((sum, each) => sum + (each ?? 0) + segmentPadding[size], 0) > room + 0.5;
  const isUneven = !isStacked && extents.every(each => each !== undefined) && extents.some(each => Math.abs((each ?? 0) - (extents[0] ?? 0)) > 0.5);
  const thumb = isStacked
    ? { height: `${share}%` as const, translate: [0, `${index * 100}%`] as const }
    : isUneven
      ? { width: extents[index], translate: [extents.slice(0, index).reduce<number>((sum, each) => sum + (each ?? 0), 0), 0] as const }
      : { width: `${share}%` as const, translate: [`${index * 100}%`, 0] as const };
  const layout = isStacked ? 'stacked' : 'row';
  return (
    <view box={root} role="radiogroup" aria-label={label} data-testid={testId} {...dataOf(props)} className={cx(r.root.base, r.root[layout], className)}>
      {/* One box a segment wide, moved by its own width per segment, in a left-to-right box that
          right-to-left mirrors; stacked, one segment tall. */}
      {items.length > 0 && (
        <view aria-hidden className={r.track.base}>
          <view dir="ltr" className={r.track.inner}>
            <view className={cx(r.thumb.base, r.thumb[layout], items.some(option => option.isLabelHidden) && r.thumb.edged)} style={thumb} />
          </view>
        </view>
      )}
      {items.map((option, at) => {
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
            className={cx(r.segment.base, r.segment.size[size], r.segment[layout])}
          >
            <view box={labelBoxes[at]} aria-hidden className={cx(r.content.base, r.content[layout])}>
              {option.icon !== undefined && <Glyph name={option.icon} side={iconSide[size]} className={cx(r.icon.size[size], isChecked ? r.icon.chosen : r.icon.other)} />}
              {!option.isLabelHidden && <text className={cx(r.label.base, r.label[layout], r.label.size[size], isChecked ? r.label.chosen : r.label.other)}>{option.label}</text>}
            </view>
            <view aria-hidden box={segmentBoxes[at]} className={r.segment.extent} />
          </button>
        );
      })}
    </view>
  );
}

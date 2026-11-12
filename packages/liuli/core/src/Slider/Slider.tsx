import { cx, useId, type Local } from '@lingxia/view';
import { describedBy } from '../internal/describedBy';
import { sliderRecipe as r } from './Slider.recipe';

export type SliderVariant = 'standalone' | 'row';

export type SliderBinding =
  | {
      /** A View-local cell the slider reads and writes; use it or `value` with `onChange`. */
      bind: Local<number>;
      value?: never;
      onChange?: never;
    }
  | {
      bind?: never;
      /** The app's value; pass `onChange` with it. */
      value: number;
      /** Runs with each new value while dragging, snapped to `step`. */
      onChange: (value: number) => void;
    };

export type SliderProps = SliderBinding & {
  /** The visible label and the slider's accessible name. */
  label: string;
  /** Hides the visible label; the slider keeps it as its name. @default false */
  isLabelHidden?: boolean;
  /**
   * The value in words, drawn at the end of the label row and read after the name: "40%",
   * "1:24 of 3:10". Liuli formats no numbers.
   */
  valueLabel?: string;
  /** Help text under the slider, read as its description. */
  description?: string;
  /** @default 0 */
  min?: number;
  /** @default 100 */
  max?: number;
  /** The value snaps to it. @default 1 */
  step?: number;
  /**
   * `row` sits in a list group among `Item` rows: the row's padding, the Item label style, the
   * value at the end in muted. @default 'standalone'
   */
  variant?: SliderVariant;
  /** @default false */
  isDisabled?: boolean;
  className?: string;
  /** On the slider. */
  'data-testid'?: string;
};

/** A value picked along a track, with a label row that can show the value in words. */
export function Slider(props: SliderProps) {
  const {
    label, isLabelHidden = false, valueLabel, description, min = 0, max = 100, step = 1, variant = 'standalone', isDisabled = false,
    className, 'data-testid': testId,
  } = props;
  const id = useId();
  const valueId = `${id}-value`;
  const helpId = `${id}-help`;
  const semantics = {
    'aria-label': label,
    'aria-describedby': describedBy(valueLabel !== undefined && valueId, description !== undefined && helpId),
    min,
    max,
    step,
    disabled: isDisabled,
    'data-testid': testId,
    className: r.control.base,
  };
  return (
    <view className={cx(r.root.base, r.root.variant[variant], className)}>
      {(!isLabelHidden || valueLabel !== undefined) && (
        <view className={r.header.base}>
          {/* The slider carries the label as its name, so the visible one is not read again. */}
          <text aria-hidden className={cx(r.label.base, r.label.variant[variant], isDisabled && r.label.disabled, isLabelHidden && r.label.hidden)}>{label}</text>
          {valueLabel !== undefined && <text id={valueId} className={cx(r.value.base, r.value.variant[variant])}>{valueLabel}</text>}
        </view>
      )}
      {props.bind ? <slider bind={props.bind} {...semantics} /> : <slider value={props.value} onChange={props.onChange} {...semantics} />}
      {description !== undefined && <text id={helpId} className={r.description.base}>{description}</text>}
    </view>
  );
}

import { cx, useId, type Local } from '@lingxia/view';
import { sliderRecipe as r } from './Slider.recipe';
import { dataOf } from '../internal/data';

export type SliderVariant = 'standalone' | 'row';

export type SliderSize = 'sm' | 'md';

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

/** A named point on the track. */
export interface SliderMark {
  value: number;
  /** Drawn under the tick, as the app writes it: "Small", "0", "100%". */
  label?: string;
}

export type SliderProps = SliderBinding & {
  /** The visible label and the slider's accessible name. */
  label: string;
  /** Hides the visible label; the slider keeps it as its name. @default false */
  isLabelHidden?: boolean;
  /**
   * The value in words, drawn at the end of the label row and read as the value: "40%",
   * "1:24 of 3:10". Liuli formats no numbers.
   */
  valueLabel?: string;
  /** A second, quieter range from `min`, such as how much of a recording has loaded. */
  secondaryValue?: number;
  /** Help text under the slider, read as its description. */
  description?: string;
  /** @default 0 */
  min?: number;
  /** @default 100 */
  max?: number;
  /** The value snaps to it. @default 1 */
  step?: number;
  /** Ticks under the track, with optional labels, in ascending order: the ends of the scale, or each step of a short one. */
  marks?: readonly SliderMark[];
  /** Runs once when a drag ends, with the value it ended on: for work too heavy to do at every step, such as saving. */
  onChangeEnd?: (value: number) => void;
  /** `sm` for dense rows and media controls. @default 'md' */
  size?: SliderSize;
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
    label, isLabelHidden = false, valueLabel, secondaryValue, description, min = 0, max = 100, step = 1, size = 'md',
    variant = 'standalone', marks, onChangeEnd, isDisabled = false, className, 'data-testid': testId,
  } = props;
  const id = useId();
  const helpId = `${id}-help`;
  const semantics = {
    'aria-label': label,
    'aria-valuetext': valueLabel,
    'aria-describedby': description !== undefined ? helpId : undefined,
    min,
    max,
    step,
    secondaryValue,
    // The primitive reports the finger leaving; the value it ended on is the one in hand.
    onPressChange: onChangeEnd && ((pressed: boolean) => { if (!pressed) onChangeEnd(props.bind ? props.bind.value : props.value); }),
    disabled: isDisabled,
    'data-testid': testId, ...dataOf(props),
    className: cx(r.control.base, r.control.size[size]),
  };
  return (
    <view className={cx(r.root.base, r.root.variant[variant], className)}>
      {(!isLabelHidden || valueLabel !== undefined) && (
        <view className={r.header.base}>
          {/* The slider carries the label as its name and the words as its value, so neither is read twice. */}
          <text aria-hidden className={cx(r.label.base, r.label.variant[variant], isDisabled && r.label.disabled, isLabelHidden && r.label.hidden)}>{label}</text>
          {valueLabel !== undefined && <text aria-hidden className={cx(r.value.base, r.value.variant[variant])}>{valueLabel}</text>}
        </view>
      )}
      {props.bind ? <slider bind={props.bind} {...semantics} /> : <slider value={props.value} onChange={props.onChange} {...semantics} />}
      {marks !== undefined && marks.length > 0 && (
        <view aria-hidden className={cx(r.marks.base, r.marks.size[size])}>
          {marks.map((mark, index) => {
            const at = ((mark.value - min) / (max - min)) * 100;
            const before = index === 0 ? 0 : ((marks[index - 1].value - min) / (max - min)) * 100;
            const edge = mark.value <= min ? 'start' : mark.value >= max ? 'end' : 'middle';
            return [
              <view key={`gap-${mark.value}`} style={{ width: `${at - before}%` }} />,
              <view key={`mark-${mark.value}`} className={cx(r.mark.base, r.mark.edge[edge])}>
                <view className={r.tick.base} />
                {mark.label !== undefined && <text className={r.markLabel.base}>{mark.label}</text>}
              </view>,
            ];
          })}
        </view>
      )}
      {description !== undefined && <text id={helpId} className={r.description.base}>{description}</text>}
    </view>
  );
}

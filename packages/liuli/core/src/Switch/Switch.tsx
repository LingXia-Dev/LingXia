import { cx, useId, type Local } from '@lingxia/view';
import { spinner, spinnerBox } from '../internal/spinner';
import { switchRecipe as r } from './Switch.recipe';
import { dataOf } from '../internal/data';

export type SwitchVariant = 'standalone' | 'row';

export type SwitchSize = 'sm' | 'md';

export type SwitchBinding =
  | {
      /** A View-local cell the switch reads and writes; use it or `value` with `onChange`. */
      bind: Local<boolean>;
      value?: never;
      onChange?: never;
    }
  | {
      bind?: never;
      /** The app's value; pass `onChange` with it. */
      value: boolean;
      /** Runs with the new value; the app passes it back as `value`. */
      onChange: (value: boolean) => void;
    };

export type SwitchProps = SwitchBinding & {
  /** The visible label and the switch's name. Say what is on, not the action: "Wi-Fi", "Sync over mobile data". */
  label: string;
  /** Help text under the label, read as the switch's description. */
  description?: string;
  /** @default false */
  isDisabled?: boolean;
  /** A spinner before the track while the app applies the change; the switch reports busy and takes no press until it settles. @default false */
  isLoading?: boolean;
  /** `sm` for dense rows. @default 'md' */
  size?: SwitchSize;
  /** `row` sits in a list group among `Item` rows, with the row's padding and height. @default 'standalone' */
  variant?: SwitchVariant;
  className?: string;
  /** On the switch. */
  'data-testid'?: string;
};

/** A setting that takes effect at once: on or off, label at the start, track at the end. */
export function Switch(props: SwitchProps) {
  const { label, description, isDisabled = false, isLoading = false, size = 'md', variant = 'standalone', className, 'data-testid': testId } = props;
  const helpId = `${useId()}-help`;
  const semantics = {
    'aria-label': label,
    'aria-describedby': description !== undefined ? helpId : undefined,
    disabled: isDisabled || isLoading,
    'aria-busy': isLoading || undefined,
    'data-testid': testId, ...dataOf(props),
    'data-state': (props.bind ? props.bind.value : props.value) ? 'on' : 'off',
    className: cx(r.control.base, r.control.size[size], r.control.variant[variant], isLoading && r.control.loading),
  };
  const words = (
    <view className={r.text.base}>
      <text className={cx(r.label.base, isDisabled && r.label.disabled)}>{label}</text>
      {description !== undefined && <text id={helpId} className={r.description.base}>{description}</text>}
    </view>
  );
  const content = isLoading ? (
    <view className={r.loading.base}>
      <view className={r.loading.words}>{words}</view>
      <vector aria-hidden paths={spinner} viewBox={spinnerBox} className={r.spinner.base} />
    </view>
  ) : words;
  return (
    <view className={cx(r.root.base, r.root.variant[variant], className)}>
      {props.bind ? (
        <switch bind={props.bind} {...semantics}>{content}</switch>
      ) : (
        <switch value={props.value} onChange={next => props.onChange(next)} {...semantics}>{content}</switch>
      )}
    </view>
  );
}

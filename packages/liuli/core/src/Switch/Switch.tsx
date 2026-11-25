import { cx, useId, type Local } from '@lingxia/view';
import { describedBy } from '../internal/describedBy';
import { StatusRegion } from '../internal/StatusRegion';
import { spinner, spinnerBox } from '../internal/spinner';
import { switchRecipe as r } from './Switch.recipe';
import { dataOf } from '../internal/data';
import { useChangeAction } from '../internal/changeAction';

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
  /** Keeps `label` as the name and draws only the track, for a switch whose row already says what it sets. Help text is not drawn either. @default false */
  isLabelHidden?: boolean;
  /** @default false */
  isDisabled?: boolean;
  /** A spinner before the track while the app applies the change; the switch reports busy and takes no press until it settles. @default false */
  isLoading?: boolean;
  /**
   * An async change, such as a Logic action. The switch flips at once and reports busy to assistive tech while it runs, with no spinner;
   * if it fails, the switch flips back and says so under its label. A rejection is logged with the label.
   */
  changeAction?: (value: boolean) => Promise<unknown>;
  /** Said under the label when `changeAction` fails. Defaults to a short "try again", in the page's language. */
  failedMessage?: string;
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
  const { label, description, isLabelHidden = false, isDisabled = false, isLoading = false, changeAction, failedMessage, size = 'md', variant = 'standalone', className, 'data-testid': testId } = props;
  const id = useId();
  const helpId = `${id}-help`;
  const failedId = `${id}-failed`;
  const value = props.bind ? props.bind.value : props.value;
  const apply = (next: boolean) => (props.bind ? props.bind.set(next) : props.onChange(next));
  const { isPending, failure, change } = useChangeAction('Switch', label, apply, changeAction, failedMessage);
  const semantics = {
    'aria-label': label,
    'aria-describedby': describedBy(description !== undefined && !isLabelHidden && helpId, failure !== undefined && !isLabelHidden && failedId),
    disabled: isDisabled || isLoading,
    // An action's wait is silent, like the platform switches: busy to assistive tech, no spinner, no reflow.
    'aria-busy': isLoading || isPending || undefined,
    'data-testid': testId, ...dataOf(props),
    'data-state': value ? 'on' : 'off',
    className: cx(r.control.base, r.control.size[size], r.control.variant[variant], isLoading && r.control.loading, isLabelHidden && r.control.bare),
  };
  const words = (
    <view className={r.text.base}>
      <text className={cx(r.label.base, isDisabled && r.label.disabled)}>{label}</text>
      {description !== undefined && <text id={helpId} className={r.description.base}>{description}</text>}
      {changeAction && <StatusRegion id={failedId} status={failure === undefined ? undefined : { type: 'error', message: failure }} className={r.failed.base} />}
    </view>
  );
  const spinnerOnly = <vector aria-hidden paths={spinner} viewBox={spinnerBox} className={r.spinner.base} />;
  const content = isLabelHidden ? (isLoading ? spinnerOnly : undefined) : isLoading ? (
    <view className={r.loading.base}>
      <view className={r.loading.words}>{words}</view>
      <vector aria-hidden paths={spinner} viewBox={spinnerBox} className={r.spinner.base} />
    </view>
  ) : words;
  return (
    <view className={cx(r.root.base, r.root.variant[variant], className)}>
      {/* With an action the switch goes through `change`, which holds off a second flip while one runs. */}
      {props.bind && !changeAction ? (
        <switch bind={props.bind} {...semantics}>{content}</switch>
      ) : (
        <switch value={value} onChange={next => void change(next)} {...semantics}>{content}</switch>
      )}
    </view>
  );
}

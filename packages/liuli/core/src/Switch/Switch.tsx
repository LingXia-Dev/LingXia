import { cx, useContext, useId, type Local } from '@lingxia/view';
import { describedBy } from '../internal/describedBy';
import { StatusRegion } from '../internal/StatusRegion';
import { Glyph } from '../internal/Glyph';
import { ItemStatusContext } from '../internal/itemStatus';
import { SpinnerMark } from '../internal/spinner';
import { switchRecipe as r } from './Switch.recipe';
import { dataOf } from '../internal/data';
import { useChangeAction } from '../internal/changeAction';
import { registerControl } from '../internal/trigger';
import { useControlSize } from '../density';

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
  /** Why it is disabled: shown in place of `description` while disabled, and read as the description. */
  disabledMessage?: string;
  /** A spinner before the track while the app applies the change; the switch reports busy and takes no press until it settles. @default false */
  isLoading?: boolean;
  /**
   * An async change, such as a Logic action. The switch flips at once and reports busy to assistive tech while it runs, with no spinner;
   * if it fails, the switch flips back and says so under its label. A rejection is logged with the label.
   */
  changeAction?: (value: boolean) => Promise<unknown>;
  /** Said under the label when `changeAction` fails. Defaults to a short "try again", in the page's language. */
  failedMessage?: string;
  /** `sm` for dense rows. By default `sm` inside a compact `DensityProvider`, else `md`. */
  size?: SwitchSize;
  /** `row` sits in a list group among `Item` rows, with the row's padding and height. @default 'standalone' */
  variant?: SwitchVariant;
  className?: string;
  /** On the switch. */
  'data-testid'?: string;
};

/** A setting that takes effect at once: on or off, label at the start, track at the end. */
export function Switch(props: SwitchProps) {
  const { label, description: own, isLabelHidden = false, isDisabled = false, disabledMessage, isLoading = false, changeAction, failedMessage, size: ownSize, variant = 'standalone', className, 'data-testid': testId } = props;
  const size = useControlSize(ownSize);
  const description = isDisabled && disabledMessage !== undefined ? disabledMessage : own;
  const id = useId();
  const helpId = `${id}-help`;
  const failedId = `${id}-failed`;
  const value = props.bind ? props.bind.value : props.value;
  const apply = (next: boolean) => (props.bind ? props.bind.set(next) : props.onChange(next));
  // Label-hidden inside an Item, the row draws the failure under its own words.
  const toRow = useContext(ItemStatusContext);
  const inRow = isLabelHidden && toRow !== undefined;
  const { isPending, failure, change } = useChangeAction('Switch', label, apply, changeAction, failedMessage, inRow ? toRow : undefined);
  const semantics = {
    'aria-label': label,
    'aria-describedby': describedBy(description !== undefined && !isLabelHidden && helpId, failure !== undefined && failedId),
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
  const spinnerOnly = <SpinnerMark className={r.spinner.base} />;
  const content = isLabelHidden ? (isLoading ? spinnerOnly : undefined) : isLoading ? (
    <view className={r.loading.base}>
      <view className={r.loading.words}>{words}</view>
      <SpinnerMark className={r.spinner.base} />
    </view>
  ) : words;
  const control = props.bind && !changeAction ? (
    <switch key="control" bind={props.bind} {...semantics}>{content}</switch>
  ) : (
    <switch key="control" value={value} onChange={next => void change(next)} {...semantics}>{content}</switch>
  );
  if (!isLabelHidden || !changeAction) {
    // With an action the switch goes through `change`, which holds off a second flip while one runs.
    return <view className={cx(r.root.base, r.root.variant[variant], className)}>{control}</view>;
  }
  // No words to put a failure under: it is announced, describes the switch, and is marked beside it (or in the Item's row).
  return (
    <view className={cx(r.root.base, r.root.variant[variant], r.root.hidden, className)}>
      {failure !== undefined && !inRow && (
        <view key="mark" role="img" aria-label={failure} data-testid={testId && `${testId}-failed`} className={r.failedMark.base}>
          <Glyph name="error" side={18} className={r.failedMark.icon} />
        </view>
      )}
      {control}
      <view key="alert" id={failedId} role="alert" className={r.alert.base}>{failure !== undefined && <text>{failure}</text>}</view>
    </view>
  );
}
registerControl(Switch);

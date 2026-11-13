import { cx, useId, type Local } from '@lingxia/view';
import { switchRecipe as r } from './Switch.recipe';

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
  const { label, description, isDisabled = false, size = 'md', variant = 'standalone', className, 'data-testid': testId } = props;
  const helpId = `${useId()}-help`;
  const semantics = {
    'aria-label': label,
    'aria-describedby': description !== undefined ? helpId : undefined,
    disabled: isDisabled,
    'data-testid': testId,
    'data-state': (props.bind ? props.bind.value : props.value) ? 'on' : 'off',
    className: cx(r.control.base, r.control.size[size], r.control.variant[variant]),
  };
  return (
    <view className={cx(r.root.base, r.root.variant[variant], className)}>
      {props.bind ? (
        <switch bind={props.bind} {...semantics}>{label}</switch>
      ) : (
        <switch value={props.value} onChange={next => props.onChange(next)} {...semantics}>{label}</switch>
      )}
      {description !== undefined && <text id={helpId} className={cx(r.description.base, r.description.size[size])}>{description}</text>}
    </view>
  );
}

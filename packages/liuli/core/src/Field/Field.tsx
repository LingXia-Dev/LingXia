import { cx, useId, type Child } from '@lingxia/view';
import { describedBy } from '../internal/describedBy';
import { hasMessage, StatusRegion } from '../internal/StatusRegion';
import { fieldRecipe as r } from './Field.recipe';

export type FieldSize = 'sm' | 'md' | 'lg';
export type FieldStatusType = 'error' | 'warning' | 'success';
/** A validation result. `message` is the app's own copy. */
export interface FieldStatus {
  type: FieldStatusType;
  message?: string;
}

/** The label, help and status props every Liuli form control shares. */
export interface FieldOwnProps {
  /** The visible label and the control's accessible name. */
  label: string;
  /** Hides the visible label; the control keeps it as its name. @default false */
  isLabelHidden?: boolean;
  /** Help text under the control, read as the control's description. */
  description?: string;
  /** An `error` status marks the control invalid; its message is read as the control's description. */
  status?: FieldStatus;
  /** Marks the label with an asterisk and the control as required. Validation is the app's. @default false */
  isRequired?: boolean;
  /** Draws the asterisk on a required label; turn it off on a form where every field is required. @default true */
  hasRequiredMark?: boolean;
  /** @default false */
  isDisabled?: boolean;
  /** Shown in place of `description` while disabled. */
  disabledMessage?: string;
  /** @default 'md' */
  size?: FieldSize;
}

/** What Field hands its control: the name, the description ids and the states. */
export interface FieldControl {
  'aria-label': string;
  'aria-describedby'?: string;
  'aria-invalid'?: boolean;
  'aria-required'?: boolean;
  disabled: boolean;
}

export interface FieldProps extends FieldOwnProps {
  /** Renders the control from the props Field gives it; pass each one on to the control element. */
  children: (control: FieldControl) => Child;
  className?: string;
  'data-testid'?: string;
}

/**
 * Label, control, help text and status in one column. The control is named by the label and
 * described by the help text and status. TextInput and the other form controls use it inside.
 */
export function Field({
  label,
  isLabelHidden = false,
  description,
  status,
  isRequired = false,
  hasRequiredMark = true,
  isDisabled = false,
  disabledMessage,
  size = 'md',
  children,
  className,
  'data-testid': testId,
}: FieldProps) {
  const id = useId();
  const helpId = `${id}-help`;
  const statusId = `${id}-status`;
  const help = isDisabled && disabledMessage ? disabledMessage : description;
  const control: FieldControl = {
    'aria-label': label,
    'aria-describedby': describedBy(help !== undefined && helpId, hasMessage(status) && statusId),
    'aria-invalid': status?.type === 'error' || undefined,
    'aria-required': isRequired || undefined,
    disabled: isDisabled,
  };
  return (
    <view data-testid={testId} className={cx(r.root.base, className)}>
      {!isLabelHidden && (
        // The control carries the same text as its name, so the visible label is not read again.
        <text aria-hidden className={cx(r.label.base, r.label.size[size], isDisabled && r.label.disabled)}>
          {label}
          {isRequired && hasRequiredMark && <span className={r.required.base}>{' *'}</span>}
        </text>
      )}
      {children(control)}
      {help !== undefined && <text id={helpId} className={r.description.base}>{help}</text>}
      <StatusRegion id={statusId} status={status} />
    </view>
  );
}

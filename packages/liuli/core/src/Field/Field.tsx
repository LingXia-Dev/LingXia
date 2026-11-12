import { cx, type Child } from '@lingxia/view';
import { StatusRegion } from '../internal/StatusRegion';
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
  /** Help text under the control. */
  description?: string;
  status?: FieldStatus;
  /** Marks the label with an asterisk. Validation is the app's. @default false */
  isRequired?: boolean;
  /** @default false */
  isDisabled?: boolean;
  /** Shown in place of `description` while disabled. */
  disabledMessage?: string;
  /** @default 'md' */
  size?: FieldSize;
}

export interface FieldProps extends FieldOwnProps {
  /**
   * The control. It must carry `aria-label={label}` itself: the visible label is hidden from
   * assistive tech so it is not read twice.
   */
  children: Child;
  className?: string;
  'data-testid'?: string;
}

/**
 * Label, control, help text and status in one column. Wrap a control Liuli has no component for
 * yet; TextInput and the other form controls use it inside.
 */
export function Field({
  label,
  isLabelHidden = false,
  description,
  status,
  isRequired = false,
  isDisabled = false,
  disabledMessage,
  size = 'md',
  children,
  className,
  'data-testid': testId,
}: FieldProps) {
  const help = isDisabled && disabledMessage ? disabledMessage : description;
  return (
    <view data-testid={testId} className={cx(r.root.base, className)}>
      {!isLabelHidden && (
        <text aria-hidden className={cx(r.label.base, r.label.size[size], isDisabled && r.label.disabled)}>
          {label}
          {isRequired && <span className={r.required.base}>{' *'}</span>}
        </text>
      )}
      {children}
      {help !== undefined && <text className={r.description.base}>{help}</text>}
      <StatusRegion status={status} />
    </view>
  );
}

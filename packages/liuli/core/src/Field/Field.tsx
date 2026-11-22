import { cx, useId, type Child } from '@lingxia/view';
import { useControlSize } from '../density';
import { describedBy } from '../internal/describedBy';
import { hasMessage, StatusRegion } from '../internal/StatusRegion';
import { InfoTip } from '../InfoTip/InfoTip';
import { fieldRecipe as r } from './Field.recipe';
import { dataOf } from '../internal/data';

export type FieldSize = 'sm' | 'md' | 'lg';
export type FieldStatusType = 'error' | 'warning' | 'success';
/** A validation result. `message` is the app's own copy. */
export interface FieldStatus {
  type: FieldStatusType;
  message?: string;
}

/** A note behind an info button beside the label. */
export interface FieldInfo {
  /** The info button's accessible name: "About the tax number". */
  label: string;
  /** The note: a sentence or two that would crowd the help text. */
  content: string;
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
  /** A word after the label for a field that may be left empty, as the app writes it: "optional". Read before the help text. */
  optionalLabel?: string;
  /** An info button after the label that opens a note. For what most people need, use `description`: it is always in view. */
  info?: FieldInfo;
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
  optionalLabel,
  info,
  isDisabled = false,
  disabledMessage,
  size: ownSize,
  children,
  className,
  'data-testid': testId, ...attributes
}: FieldProps) {
  const size = useControlSize(ownSize);
  const id = useId();
  const helpId = `${id}-help`;
  const statusId = `${id}-status`;
  const optionalId = `${id}-optional`;
  const hasOptional = optionalLabel !== undefined && !isRequired && !isLabelHidden;
  const hasInfo = info !== undefined && !isLabelHidden;
  const help = isDisabled && disabledMessage ? disabledMessage : description;
  const control: FieldControl = {
    'aria-label': label,
    'aria-describedby': describedBy(hasOptional && optionalId, help !== undefined && helpId, hasMessage(status) && statusId),
    'aria-invalid': status?.type === 'error' || undefined,
    'aria-required': isRequired || undefined,
    disabled: isDisabled,
  };
  // The control carries the same text as its name, so the visible label is not read again.
  const visibleLabel = (
    <text aria-hidden className={cx(r.label.base, r.label.size[size], isDisabled && r.label.disabled)}>
      {label}
      {isRequired && hasRequiredMark && <span className={r.required.base}>{' *'}</span>}
    </text>
  );
  return (
    <view data-testid={testId} {...dataOf(attributes)} className={cx(r.root.base, className)}>
      {!isLabelHidden && !hasOptional && !hasInfo && visibleLabel}
      {(hasOptional || hasInfo) && (
        <view className={r.labelRow.base}>
          {visibleLabel}
          {hasOptional && <text id={optionalId} className={cx(r.optional.base, r.optional.size[size])}>{optionalLabel}</text>}
          {hasInfo && <InfoTip label={info.label} content={info.content} size="sm" className={cx(r.info.base, r.info.size[size])} />}
        </view>
      )}
      {children(control)}
      {help !== undefined && <text id={helpId} className={r.description.base}>{help}</text>}
      <StatusRegion id={statusId} status={status} />
    </view>
  );
}

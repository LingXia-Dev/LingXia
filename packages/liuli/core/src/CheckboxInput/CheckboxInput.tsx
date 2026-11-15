import { cx, useId, type Local } from '@lingxia/view';
import type { FieldOwnProps } from '../Field/Field';
import { fieldRecipe } from '../Field/Field.recipe';
import { describedBy } from '../internal/describedBy';
import { Glyph } from '../internal/Glyph';
import { hasMessage, StatusRegion } from '../internal/StatusRegion';
import { checkboxInputRecipe as r } from './CheckboxInput.recipe';

export type CheckboxInputBinding =
  | {
      /** A View-local cell the control reads and writes; use it or `value` with `onChange`. */
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

export type CheckboxInputProps = FieldOwnProps & CheckboxInputBinding & {
  /** Stands for a set that is partly chosen: draws a dash and is read as mixed. A press asks for all; the app clears it when the set is all or none. @default false */
  isIndeterminate?: boolean;
  className?: string;
  /** On the checkbox. */
  'data-testid'?: string;
};

/**
 * One on/off choice with a label and optional help text. The box, label and help text are one
 * control: a tap anywhere on them toggles it. It is named by the label and described by the help
 * text and status.
 */
export function CheckboxInput(props: CheckboxInputProps) {
  const {
    label, isLabelHidden = false, description, status, isRequired = false, isDisabled = false, disabledMessage, size = 'md',
    isIndeterminate = false, className, 'data-testid': testId,
  } = props;
  const id = useId();
  const helpId = `${id}-help`;
  const statusId = `${id}-status`;
  const checked = props.bind ? props.bind.value : props.value;
  const isMixed = isIndeterminate && !checked;
  const state = checked ? 'checked' : isMixed ? 'mixed' : 'unchecked';
  const help = isDisabled && disabledMessage ? disabledMessage : description;
  const semantics = {
    'aria-label': label,
    'aria-describedby': describedBy(help !== undefined && helpId, hasMessage(status) && statusId),
    'aria-invalid': status?.type === 'error' || undefined,
    'aria-required': isRequired || undefined,
  };
  const content = (
    <view className={r.row.base}>
      <view aria-hidden className={cx(r.box.base, r.box.size[size], r.box.state[checked || isMixed ? 'on' : 'off'], !checked && !isMixed && status?.type === 'error' && r.box.invalid)}>
        {checked && <Glyph name="check" weight={2.5} className={r.mark.size[size]} />}
        {isMixed && <view className={cx(r.dash.base, r.dash.size[size])} />}
      </view>
      <view className={r.text.base}>
        <text className={cx(r.label.base, r.label.size[size], isLabelHidden && r.label.hidden)}>
          {label}
          {isRequired && <span className={fieldRecipe.required.base}>{' *'}</span>}
        </text>
        {help !== undefined && <text id={helpId} className={r.description.base}>{help}</text>}
      </view>
    </view>
  );
  return (
    <view className={cx(r.root.base, className)}>
      {props.bind ? (
        <checkbox
          bind={props.bind}
          aria-label={semantics['aria-label']}
          aria-describedby={semantics['aria-describedby']}
          aria-invalid={semantics['aria-invalid']}
          aria-required={semantics['aria-required']}
          indeterminate={isMixed || undefined}
          disabled={isDisabled}
          data-testid={testId}
          data-state={state}
          className={r.control.base}
        >
          {content}
        </checkbox>
      ) : (
        <checkbox
          value={props.value}
          onChange={props.onChange}
          aria-label={semantics['aria-label']}
          aria-describedby={semantics['aria-describedby']}
          aria-invalid={semantics['aria-invalid']}
          aria-required={semantics['aria-required']}
          indeterminate={isMixed || undefined}
          disabled={isDisabled}
          data-testid={testId}
          data-state={state}
          className={r.control.base}
        >
          {content}
        </checkbox>
      )}
      <StatusRegion id={statusId} status={status} className={r.status.size[size]} />
    </view>
  );
}

import { cx, type Local } from '@lingxia/view';
import { Field, type FieldOwnProps } from '../Field/Field';
import { selectRecipe as r } from './Select.recipe';

export interface SelectOption {
  /** What `value` and `onChange` carry. */
  value: string;
  /** The text in the box and in the list. */
  label: string;
  /** @default false */
  isDisabled?: boolean;
}

export type SelectBinding =
  | {
      /** A View-local cell the select reads and writes; use it or `value` with `onChange`. */
      bind: Local<string>;
      value?: never;
      onChange?: never;
    }
  | {
      bind?: never;
      /** The chosen option's value; `''` shows the placeholder. */
      value: string;
      /** Runs with the chosen option's value. */
      onChange: (value: string) => void;
    };

export type SelectProps = FieldOwnProps & SelectBinding & {
  /** The choices, in order; the platform shows them as a list or a menu. */
  options: readonly SelectOption[];
  /** Shown in the box while nothing is chosen. */
  placeholder?: string;
  className?: string;
  /** On the select. */
  'data-testid'?: string;
};

/** One choice from a list that opens on demand, with Field's label, help text and status. */
export function Select(props: SelectProps) {
  const {
    label, isLabelHidden, description, status, isRequired, hasRequiredMark, isDisabled = false, disabledMessage, size = 'md',
    options, placeholder, className, 'data-testid': testId,
  } = props;
  const choices = options.map(option => ({ value: option.value, label: option.label, disabled: option.isDisabled }));
  const controlClass = cx(r.control.base, r.control.size[size], status && r.control.status[status.type]);
  return (
    <Field
      label={label}
      isLabelHidden={isLabelHidden}
      description={description}
      status={status}
      isRequired={isRequired}
      hasRequiredMark={hasRequiredMark}
      isDisabled={isDisabled}
      disabledMessage={disabledMessage}
      size={size}
      className={className}
    >
      {control =>
        props.bind ? (
          <select
            bind={props.bind}
            options={choices}
            placeholder={placeholder}
            aria-label={control['aria-label']}
            aria-describedby={control['aria-describedby']}
            aria-invalid={control['aria-invalid']}
            aria-required={control['aria-required']}
            disabled={control.disabled}
            data-testid={testId}
            className={controlClass}
          />
        ) : (
          <select
            value={props.value}
            onChange={props.onChange}
            options={choices}
            placeholder={placeholder}
            aria-label={control['aria-label']}
            aria-describedby={control['aria-describedby']}
            aria-invalid={control['aria-invalid']}
            aria-required={control['aria-required']}
            disabled={control.disabled}
            data-testid={testId}
            className={controlClass}
          />
        )
      }
    </Field>
  );
}

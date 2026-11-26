import { cx, type Local } from '@lingxia/view';
import { useControlSize } from '../density';
import { Field, type FieldOwnProps } from '../Field/Field';
import { selectRecipe as r } from './Select.recipe';
import { joinedCorners, joinedPlace, useJoined } from '../internal/joined';
import { dataOf } from '../internal/data';

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
  items: readonly SelectOption[];
  /** Shown in the box while nothing is chosen. */
  placeholder?: string;
  className?: string;
  /** On the select. */
  'data-testid'?: string;
};

/** One choice from a list that opens on demand, with Field's label, help text and status. */
export function Select(props: SelectProps) {
  const {
    label, isLabelHidden, description, status, isRequired, hasRequiredMark, optionalLabel, info, isDisabled = false, disabledMessage, size: ownSize,
    items, placeholder, className, 'data-testid': testId,
  } = props;
  const group = useJoined();
  const joined = group?.edge ?? null;
  const size = useControlSize(ownSize ?? group?.size);
  const choices = items.map(option => ({ value: option.value, label: option.label, disabled: option.isDisabled }));
  const controlClass = cx(r.control.base, r.control.size[size], joined && joinedCorners[joined], status && r.control.status[status.type]);
  return (
    <Field
      label={label}
      isLabelHidden={isLabelHidden || (joined !== null && joined !== 'only')}
      description={description}
      status={status}
      isRequired={isRequired}
      hasRequiredMark={hasRequiredMark}
      optionalLabel={optionalLabel}
      info={info}
      isDisabled={isDisabled}
      disabledMessage={disabledMessage}
      size={size}
      className={cx(joined && joinedPlace[joined], className)}
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
            data-testid={testId} {...dataOf(props)}
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
            data-testid={testId} {...dataOf(props)}
            className={controlClass}
          />
        )
      }
    </Field>
  );
}

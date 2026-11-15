import { cx, useId, type Local } from '@lingxia/view';
import { Field, type FieldOwnProps } from '../Field/Field';
import { radioGroupRecipe as r } from './RadioGroup.recipe';

export type RadioGroupVariant = 'list' | 'card';

export interface RadioOption {
  /** What `value` and `onChange` carry. */
  value: string;
  /** The option's text and accessible name. */
  label: string;
  /** A second line, read as the option's description. */
  description?: string;
  /** @default false */
  isDisabled?: boolean;
}

export type RadioGroupBinding =
  | {
      /** A View-local cell the group reads and writes; use it or `value` with `onChange`. */
      bind: Local<string | null>;
      value?: never;
      onChange?: never;
    }
  | {
      bind?: never;
      /** The chosen option's value; `null` while none is chosen. */
      value: string | null;
      /** Runs with the option chosen by a tap or by arrowing onto it. */
      onChange: (value: string) => void;
    };

export type RadioGroupOrientation = 'vertical' | 'horizontal';

export type RadioGroupProps = FieldOwnProps & RadioGroupBinding & {
  /** The choices, in order. */
  options: readonly RadioOption[];
  /** `list` stacks plain rows; `card` makes each option a bordered card, for plans and other choices with a second line. @default 'list' */
  variant?: RadioGroupVariant;
  /** `horizontal` sets two or three short options side by side; they wrap when the line is full. @default 'vertical' */
  orientation?: RadioGroupOrientation;
  className?: string;
  /** On the group; each option gets `<testid>-<value>`. */
  'data-testid'?: string;
};

/**
 * One choice from a short list, all in view. The Field label names the group; each option is a
 * radio named by its label. One Tab stop: arrows move between options and choose on arrival.
 */
export function RadioGroup(props: RadioGroupProps) {
  const { options, variant = 'list', size = 'md', isDisabled = false, status, className, orientation = 'vertical', 'data-testid': testId } = props;
  const id = useId();
  const chosen = props.bind ? props.bind.value : props.value;
  const choose = (value: string) => {
    if (value === chosen) return;
    if (props.bind) props.bind.set(value);
    else props.onChange(value);
  };
  return (
    <Field {...props} className={className} data-testid={undefined}>
      {control => (
        <view
          role="radiogroup"
          aria-label={control['aria-label']}
          aria-describedby={control['aria-describedby']}
          aria-invalid={control['aria-invalid']}
          aria-required={control['aria-required']}
          data-testid={testId}
          className={cx(r.group.base, orientation === 'horizontal' ? r.group.horizontal[variant] : r.group.variant[variant])}
        >
          {options.map(option => {
            const isChecked = option.value === chosen;
            const helpId = `${id}-${option.value}-help`;
            return (
              <button
                key={option.value}
                role="radio"
                aria-checked={isChecked}
                data-state={isChecked ? 'checked' : 'unchecked'}
                aria-label={option.label}
                aria-describedby={option.description !== undefined ? helpId : undefined}
                disabled={isDisabled || option.isDisabled}
                data-testid={testId && `${testId}-${option.value}`}
                onClick={() => choose(option.value)}
                className={cx(r.option.base, r.option.variant[variant], variant === 'card' && r.option.card[isChecked ? 'on' : 'off'])}
              >
                <view aria-hidden className={cx(r.circle.base, r.circle.size[size], r.circle.state[isChecked ? 'on' : 'off'], !isChecked && status?.type === 'error' && r.circle.invalid)}>
                  {isChecked && <view className={cx(r.dot.base, r.dot.size[size])} />}
                </view>
                <view className={r.text.base}>
                  <text aria-hidden className={cx(r.label.base, r.label.variant[variant], r.label.size[size])}>{option.label}</text>
                  {option.description !== undefined && <text id={helpId} className={r.description.base}>{option.description}</text>}
                </view>
              </button>
            );
          })}
        </view>
      )}
    </Field>
  );
}

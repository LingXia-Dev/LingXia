import { cx, useId, useLocal, type Local } from '@lingxia/view';
import { Field, type FieldOwnProps } from '../Field/Field';
import { textInputRecipe as t } from '../TextInput/TextInput.recipe';
import { describedBy } from '../internal/describedBy';
import { Glyph } from '../internal/Glyph';
import { numberInputRecipe as r } from './NumberInput.recipe';

export type NumberInputBinding =
  | {
      /** A View-local cell the control reads and writes; `null` while the field is empty. Use it or `value` with `onChange`. */
      bind: Local<number | null>;
      value?: never;
      onChange?: never;
    }
  | {
      bind?: never;
      /** The number, or `null` while the field is empty. */
      value: number | null;
      /** Runs with the new number, or `null` when the field is emptied. */
      onChange: (value: number | null) => void;
    };

export type NumberInputProps = FieldOwnProps & NumberInputBinding & {
  /** The lowest number the steppers reach and the field keeps. */
  min?: number;
  /** The highest number the steppers reach and the field keeps. */
  max?: number;
  /** How much a stepper adds or takes away; its decimals are the number's. @default 1 */
  step?: number;
  /** The minus stepper's accessible name: "Fewer guests". */
  decrementLabel: string;
  /** The plus stepper's accessible name: "More guests". */
  incrementLabel: string;
  /** Leaves the steppers out, for a number people type and never nudge. @default true */
  hasSteppers?: boolean;
  /** Fixed text before the number: a currency sign. Read with the description. */
  prefix?: string;
  /** Fixed text after the number: a unit. Read with the description. */
  suffix?: string;
  placeholder?: string;
  className?: string;
  /** On the editor; the steppers are `<id>-decrement` and `<id>-increment`. */
  'data-testid'?: string;
};

/** What was typed, as a number; `null` for nothing, `undefined` while it is not a number yet ("-", "1e"). */
function parse(text: string): number | null | undefined {
  const trimmed = text.trim();
  if (trimmed === '') return null;
  const number = Number(trimmed);
  return Number.isFinite(number) ? number : undefined;
}

const decimalsOf = (value: number) => (String(value).split('.')[1] ?? '').length;

/** A number typed or nudged by steps, kept between a minimum and a maximum. */
export function NumberInput(props: NumberInputProps) {
  const {
    label, isLabelHidden, description, status, isRequired, hasRequiredMark, optionalLabel, isDisabled = false, disabledMessage, size = 'md',
    min, max, step = 1, decrementLabel, incrementLabel, hasSteppers = true, prefix, suffix, placeholder, className, 'data-testid': testId,
  } = props;
  const current = props.bind ? props.bind.value : props.value;
  const set = (value: number | null) => (props.bind ? props.bind.set(value) : props.onChange(value));
  // The text as typed, and the value it stood for. While they still match, the field shows the
  // text ("1." on the way to "1.5"); once the value changes from outside, it shows the value.
  const typed = useLocal('typed', '');
  const typedFor = useLocal<number | null>('typed-for', null);
  const shown = typedFor.value === current && typed.value !== '' ? typed.value : current === null ? '' : String(current);
  const id = useId();
  const [prefixId, suffixId] = [`${id}-prefix`, `${id}-suffix`];
  const clamp = (value: number) => Math.min(max ?? value, Math.max(min ?? value, value));
  const input = (text: string) => {
    const parsed = parse(text);
    typed.set(text);
    typedFor.set(parsed === undefined ? current : parsed);
    if (parsed !== undefined && parsed !== current) set(parsed);
  };
  const nudge = (direction: 1 | -1) => {
    // From an empty field the first press lands on the minimum, or on zero.
    // Rounded to the decimals in play, so a tenth added three times is 0.3 and a typed 4.5 keeps its half.
    const next = current === null ? clamp(min ?? 0) : clamp(Number((current + direction * step).toFixed(Math.max(decimalsOf(step), decimalsOf(current)))));
    typed.set('');
    set(next);
  };
  return (
    <Field
      label={label}
      isLabelHidden={isLabelHidden}
      description={description}
      status={status}
      isRequired={isRequired}
      hasRequiredMark={hasRequiredMark}
      optionalLabel={optionalLabel}
      isDisabled={isDisabled}
      disabledMessage={disabledMessage}
      size={size}
      className={className}
    >
      {control => (
        <view className={cx(t.box.base, t.box.size[size], status && t.box.status[status.type], isDisabled && t.box.disabled, hasSteppers && r.box.base)}>
          {prefix !== undefined && <text id={prefixId} className={cx(t.adornment.base, t.input.size[size])}>{prefix}</text>}
          <input
            value={shown}
            onInput={input}
            type="number"
            min={min}
            max={max}
            step={step}
            aria-label={control['aria-label']}
            aria-describedby={describedBy(prefix !== undefined && prefixId, suffix !== undefined && suffixId, control['aria-describedby'])}
            aria-invalid={control['aria-invalid']}
            aria-required={control['aria-required']}
            placeholder={placeholder}
            disabled={control.disabled}
            data-testid={testId}
            className={cx(t.input.base, t.input.size[size], r.input.base)}
          />
          {suffix !== undefined && <text id={suffixId} className={cx(t.adornment.base, t.input.size[size])}>{suffix}</text>}
          {hasSteppers && (
            <view className={r.steppers.base}>
              <button
                aria-label={decrementLabel}
                disabled={isDisabled || (current !== null && min !== undefined && current <= min)}
                onClick={() => nudge(-1)}
                data-testid={testId && `${testId}-decrement`}
                className={cx(r.stepper.base, r.stepper.size[size])}
              >
                <Glyph name="remove" className={r.stepperIcon.base} />
              </button>
              <button
                aria-label={incrementLabel}
                disabled={isDisabled || (current !== null && max !== undefined && current >= max)}
                onClick={() => nudge(1)}
                data-testid={testId && `${testId}-increment`}
                className={cx(r.stepper.base, r.stepper.size[size], r.stepper.second)}
              >
                <Glyph name="add" className={r.stepperIcon.base} />
              </button>
            </view>
          )}
        </view>
      )}
    </Field>
  );
}

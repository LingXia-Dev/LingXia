import { cx, type Local, type VectorPath } from '@lingxia/view';
import { Field, type FieldOwnProps, type FieldSize, type FieldStatusType } from '../Field/Field';
import { glyphBox } from './glyphs';

/** Controlled by the app (`value` + `onChange`), or bound to a View-local cell (`bind`); ISO text, `''` while empty. */
export type PickerBinding =
  | {
      /** A View-local cell the field reads and writes; use it or `value` with `onChange`. */
      bind: Local<string>;
      value?: never;
      onChange?: never;
    }
  | {
      bind?: never;
      /** The app's value; pass `onChange` with it. */
      value: string;
      /** Runs with the value picked; the app passes it back as `value`. */
      onChange: (value: string) => void;
    };

interface PickerRecipe {
  box: { base: string; size: Record<FieldSize, string>; status: Record<FieldStatusType, string>; disabled: string };
  input: { base: string; size: Record<FieldSize, string> };
  icon: { base: string };
}

export type PickerFieldProps = FieldOwnProps & PickerBinding & {
  type: 'date' | 'time';
  glyph: readonly VectorPath[];
  recipe: PickerRecipe;
  placeholder?: string;
  min?: string;
  max?: string;
  step?: number;
  onSubmit?: (value: string) => void;
  className?: string;
  controlClassName?: string;
  'data-testid'?: string;
};

/** A Field around the View's date or time input, which opens the View's own picker. */
export function PickerField(props: PickerFieldProps) {
  const {
    label, isLabelHidden, description, status, isRequired, hasRequiredMark, isDisabled = false, disabledMessage, size = 'md',
    type, glyph, recipe: r, placeholder, min, max, step, onSubmit, className, controlClassName, 'data-testid': testId,
  } = props;
  const inputClass = cx(r.input.base, r.input.size[size]);
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
      {control => {
        const shared = {
          type,
          'aria-label': control['aria-label'],
          'aria-describedby': control['aria-describedby'],
          'aria-invalid': control['aria-invalid'],
          'aria-required': control['aria-required'],
          'data-testid': testId,
          placeholder,
          min,
          max,
          step,
          disabled: control.disabled,
          onSubmit,
          className: inputClass,
        };
        return (
          <view className={cx(r.box.base, r.box.size[size], status && r.box.status[status.type], isDisabled && r.box.disabled, controlClassName)}>
            {props.bind ? <input bind={props.bind} {...shared} /> : <input value={props.value} onInput={props.onChange} {...shared} />}
            <vector aria-hidden paths={glyph} viewBox={glyphBox} className={r.icon.base} />
          </view>
        );
      }}
    </Field>
  );
}

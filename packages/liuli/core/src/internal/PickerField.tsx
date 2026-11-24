import { cx, type Local } from '@lingxia/view';
import { useControlSize } from '../density';
import { Field, type FieldOwnProps, type FieldSize, type FieldStatusType } from '../Field/Field';
import type { IconRef } from '../Icon/Icon';
import { Glyph } from './Glyph';
import { dataOf } from './data';

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
  icon: IconRef;
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

/** The picker's mark, in points at the standard text size; it grows with the text as the value does. */
export const pickerMark = 16;

/** A Field around the View's date or time input, which opens the View's own picker. */
export function PickerField(props: PickerFieldProps) {
  const {
    label, isLabelHidden, description, status, isRequired, hasRequiredMark, optionalLabel, info, isDisabled = false, disabledMessage, size: ownSize,
    type, icon, recipe: r, placeholder, min, max, step, onSubmit, className, controlClassName, 'data-testid': testId,
  } = props;
  const size = useControlSize(ownSize);
  const inputClass = cx(r.input.base, r.input.size[size]);
  return (
    <Field
      label={label}
      isLabelHidden={isLabelHidden}
      description={description}
      status={status}
      isRequired={isRequired}
      hasRequiredMark={hasRequiredMark}
      optionalLabel={optionalLabel}
      info={info}
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
          'data-testid': testId, ...dataOf(props),
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
            <Glyph name={icon} side={pickerMark} className={r.icon.base} />
          </view>
        );
      }}
    </Field>
  );
}

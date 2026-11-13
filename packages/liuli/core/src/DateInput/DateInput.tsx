import type { FieldOwnProps } from '../Field/Field';
import { glyphs } from '../internal/glyphs';
import { PickerField, type PickerBinding } from '../internal/PickerField';
import { dateInputRecipe } from './DateInput.recipe';

export type DateInputProps = FieldOwnProps & PickerBinding & {
  /** Shown while empty, such as "Choose a date"; never a substitute for `label`. */
  placeholder?: string;
  /** The earliest day offered, `YYYY-MM-DD`; earlier days are greyed in the picker. */
  min?: string;
  /** The latest day offered, `YYYY-MM-DD`. */
  max?: string;
  /** Runs with the day picked, after `onChange`. */
  onSubmit?: (value: string) => void;
  /** Classes for the field column. */
  className?: string;
  /** Classes for the bordered box. */
  controlClassName?: string;
  /** On the editor. */
  'data-testid'?: string;
};

/**
 * A labelled date field. A tap opens the View's month picker: a popover with a pointer, a sheet
 * on touch. The value is `YYYY-MM-DD`, or `''` while empty.
 */
export function DateInput(props: DateInputProps) {
  return <PickerField {...props} type="date" glyph={glyphs.calendar} recipe={dateInputRecipe} />;
}

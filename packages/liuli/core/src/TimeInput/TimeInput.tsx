import type { FieldOwnProps } from '../Field/Field';
import { glyphs } from '../internal/glyphs';
import { PickerField, type PickerBinding } from '../internal/PickerField';
import { timeInputRecipe } from './TimeInput.recipe';

export type TimeInputProps = FieldOwnProps & PickerBinding & {
  /** Shown while empty, such as "Choose a time"; never a substitute for `label`. */
  placeholder?: string;
  /** The earliest time offered, `HH:MM`. */
  min?: string;
  /** The latest time offered, `HH:MM`. */
  max?: string;
  /** Seconds between the times offered: 300 offers every 5 minutes. @default 60 */
  step?: number;
  /** Runs with the time picked, after `onChange`. */
  onSubmit?: (value: string) => void;
  /** Classes for the field column. */
  className?: string;
  /** Classes for the bordered box. */
  controlClassName?: string;
  /** On the editor. */
  'data-testid'?: string;
};

/**
 * A labelled time field. A tap opens the View's time picker, in the user's 12- or 24-hour clock.
 * The value is `HH:MM` (or `HH:MM:SS`), or `''` while empty.
 */
export function TimeInput(props: TimeInputProps) {
  return <PickerField {...props} type="time" glyph={glyphs.clock} recipe={timeInputRecipe} />;
}

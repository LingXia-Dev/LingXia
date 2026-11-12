import { cx, type AutoComplete, type Child, type Local } from '@lingxia/view';
import { Field, type FieldOwnProps } from '../Field/Field';
import { glyphBox, glyphs } from '../internal/glyphs';
import { textInputRecipe as r } from './TextInput.recipe';

/** Controlled by the app (`value` + `onChange`), or bound to a View-local cell (`bind`). */
export type TextInputBinding =
  | {
      /** A View-local cell the control reads and writes; use it or `value` with `onChange`. */
      bind: Local<string>;
      value?: never;
      onChange?: never;
    }
  | {
      bind?: never;
      /** The app's value; pass `onChange` with it. */
      value: string;
      /** Runs with the new value; the app passes it back as `value`. */
      onChange: (value: string) => void;
    };

/** The clear button needs its accessible name from the app, so turning it on asks for one. */
export type TextInputClear =
  | { hasClear?: false; clearLabel?: never }
  | {
      /** A button that empties the text, shown while there is text. @default false */
      hasClear: true;
      /** The clear button's accessible name; required with `hasClear`. */
      clearLabel: string;
    };

export type TextInputProps = FieldOwnProps & TextInputBinding & TextInputClear & {
  /** A hint shown while empty; never a substitute for `label`. */
  placeholder?: string;
  /** @default 'text' */
  type?: 'text' | 'password' | 'email' | 'number' | 'tel' | 'search';
  inputMode?: 'text' | 'numeric' | 'decimal' | 'tel' | 'email';
  autoComplete?: AutoComplete;
  /** The label of the keyboard's action key; `onSubmit` runs on it. */
  enterKeyHint?: 'enter' | 'done' | 'go' | 'next' | 'previous' | 'search' | 'send';
  autoCapitalize?: 'none' | 'sentences' | 'words' | 'characters';
  /** Unset follows the platform. */
  hasSpellCheck?: boolean;
  /** Focus it when it first appears. @default false */
  hasAutoFocus?: boolean;
  maxLength?: number;
  /** An icon inside the box, before the text. */
  startIcon?: Child;
  /** The keyboard's action key, or Enter. */
  onSubmit?: (value: string) => void;
  /** Classes for the field column. */
  className?: string;
  /** Classes for the bordered box. */
  controlClassName?: string;
  /** On the editor. */
  'data-testid'?: string;
};

/** A labelled one-line text editor. */
export function TextInput(props: TextInputProps) {
  const {
    label, isLabelHidden, description, status, isRequired, isDisabled = false, disabledMessage, size = 'md',
    placeholder, type, inputMode, autoComplete, enterKeyHint, autoCapitalize, hasSpellCheck, hasAutoFocus = false, maxLength,
    startIcon, onSubmit, className, controlClassName, 'data-testid': testId,
  } = props;
  const text = props.bind ? props.bind.value : props.value;
  const clear = () => (props.bind ? props.bind.set('') : props.onChange(''));
  const inputClass = cx(r.input.base, r.input.size[size]);
  return (
    <Field
      label={label}
      isLabelHidden={isLabelHidden}
      description={description}
      status={status}
      isRequired={isRequired}
      isDisabled={isDisabled}
      disabledMessage={disabledMessage}
      size={size}
      className={className}
    >
      {control => (
        <view className={cx(r.box.base, r.box.size[size], status && r.box.status[status.type], isDisabled && r.box.disabled, controlClassName)}>
          {startIcon !== undefined && <view aria-hidden className={r.icon.base}>{startIcon}</view>}
          {props.bind ? (
            <input
              bind={props.bind}
              aria-label={control['aria-label']}
              aria-describedby={control['aria-describedby']}
              aria-invalid={control['aria-invalid']}
              aria-required={control['aria-required']}
              data-testid={testId}
              placeholder={placeholder}
              type={type}
              inputMode={inputMode}
              autoComplete={autoComplete}
              enterKeyHint={enterKeyHint}
              autoCapitalize={autoCapitalize}
              spellCheck={hasSpellCheck}
              autoFocus={hasAutoFocus}
              maxLength={maxLength}
              disabled={control.disabled}
              onSubmit={onSubmit}
              className={inputClass}
            />
          ) : (
            <input
              value={props.value}
              onInput={props.onChange}
              aria-label={control['aria-label']}
              aria-describedby={control['aria-describedby']}
              aria-invalid={control['aria-invalid']}
              aria-required={control['aria-required']}
              data-testid={testId}
              placeholder={placeholder}
              type={type}
              inputMode={inputMode}
              autoComplete={autoComplete}
              enterKeyHint={enterKeyHint}
              autoCapitalize={autoCapitalize}
              spellCheck={hasSpellCheck}
              autoFocus={hasAutoFocus}
              maxLength={maxLength}
              disabled={control.disabled}
              onSubmit={onSubmit}
              className={inputClass}
            />
          )}
          {props.hasClear && text !== '' && !isDisabled && (
            <button aria-label={props.clearLabel} onClick={clear} className={r.clear.base}>
              <vector aria-hidden paths={glyphs.close} viewBox={glyphBox} className={r.clearIcon.base} />
            </button>
          )}
        </view>
      )}
    </Field>
  );
}

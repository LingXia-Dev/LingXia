import { cx, useId, type AutoComplete, type Child, type Local } from '@lingxia/view';
import { Field, type FieldOwnProps } from '../Field/Field';
import { describedBy } from '../internal/describedBy';
import { Glyph } from '../internal/Glyph';
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
  /** `search` draws Liuli's magnifier, not the platform's, and defaults `enterKeyHint` to `search`. @default 'text' */
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
  /** An icon inside the box, before the text; drawn 16 px in the muted colour. Replaces the search magnifier. */
  startIcon?: Child;
  /** Fixed text inside the box before the value, as the app writes it: "$", "https://". Read with the description. */
  prefix?: string;
  /** Fixed text inside the box after the value: a unit such as "kg" or "%". Read with the description. */
  suffix?: string;
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
    label, isLabelHidden, description, status, isRequired, hasRequiredMark, optionalLabel, isDisabled = false, disabledMessage, size = 'md',
    placeholder, type, inputMode, autoComplete, enterKeyHint, autoCapitalize, hasSpellCheck, hasAutoFocus = false, maxLength,
    startIcon, prefix, suffix, onSubmit, className, controlClassName, 'data-testid': testId,
  } = props;
  const text = props.bind ? props.bind.value : props.value;
  // The platform search field draws its own magnifier at its own size and colour, so search is a
  // plain editor with Liuli's icon.
  const isSearch = type === 'search';
  const editorType = isSearch ? undefined : type;
  const keyHint = enterKeyHint ?? (isSearch ? 'search' : undefined);
  const leading = startIcon ?? (isSearch ? <Glyph name="search" className={r.searchIcon.base} /> : undefined);
  const clear = () => (props.bind ? props.bind.set('') : props.onChange(''));
  const inputClass = cx(r.input.base, r.input.size[size]);
  const id = useId();
  const [prefixId, suffixId] = [`${id}-prefix`, `${id}-suffix`];
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
        <view className={cx(r.box.base, r.box.size[size], status && r.box.status[status.type], isDisabled && r.box.disabled, controlClassName)}>
          {leading !== undefined && <view aria-hidden className={r.icon.base}>{leading}</view>}
          {prefix !== undefined && <text id={prefixId} className={cx(r.adornment.base, r.input.size[size])}>{prefix}</text>}
          {props.bind ? (
            <input
              bind={props.bind}
              aria-label={control['aria-label']}
              aria-describedby={describedBy(prefix !== undefined && prefixId, suffix !== undefined && suffixId, control['aria-describedby'])}
              aria-invalid={control['aria-invalid']}
              aria-required={control['aria-required']}
              data-testid={testId}
              placeholder={placeholder}
              type={editorType}
              inputMode={inputMode}
              autoComplete={autoComplete}
              enterKeyHint={keyHint}
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
              aria-describedby={describedBy(prefix !== undefined && prefixId, suffix !== undefined && suffixId, control['aria-describedby'])}
              aria-invalid={control['aria-invalid']}
              aria-required={control['aria-required']}
              data-testid={testId}
              placeholder={placeholder}
              type={editorType}
              inputMode={inputMode}
              autoComplete={autoComplete}
              enterKeyHint={keyHint}
              autoCapitalize={autoCapitalize}
              spellCheck={hasSpellCheck}
              autoFocus={hasAutoFocus}
              maxLength={maxLength}
              disabled={control.disabled}
              onSubmit={onSubmit}
              className={inputClass}
            />
          )}
          {suffix !== undefined && <text id={suffixId} className={cx(r.adornment.base, r.input.size[size])}>{suffix}</text>}
          {props.hasClear && text !== '' && !isDisabled && (
            <button aria-label={props.clearLabel} onClick={clear} className={r.clear.base}>
              <Glyph name="close" className={r.clearIcon.base} />
            </button>
          )}
        </view>
      )}
    </Field>
  );
}

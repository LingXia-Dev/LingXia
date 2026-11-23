import { cx, useId, useLocal, type AutoComplete, type Child, type Local } from '@lingxia/view';
import { liuliText } from '../i18n/text';
import { useControlSize } from '../density';
import { Field, type FieldOwnProps } from '../Field/Field';
import type { IconRef } from '../Icon/Icon';
import { describedBy } from '../internal/describedBy';
import { Glyph } from '../internal/Glyph';
import { spinner, spinnerBox } from '../internal/spinner';
import { textInputRecipe as r } from './TextInput.recipe';
import { dataOf } from '../internal/data';

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

/** A clear button, named "Clear" in the page's language unless the app names it. */
export type TextInputClear =
  | { hasClear?: false; clearLabel?: never }
  | {
      /** A button that empties the text, shown while there is text. @default false */
      hasClear: true;
      /** The clear button's accessible name. Defaults to "Clear" in the page's language. */
      clearLabel?: string;
    };

// Values that are always written left to right: addresses, numbers, phone numbers.
const leftToRightTypes = new Set(['email', 'number', 'tel', 'numeric', 'decimal']);

export type TextInputProps = FieldOwnProps & TextInputBinding & TextInputClear & {
  /** A hint shown while empty; never a substitute for `label`. */
  placeholder?: string;
  /** `search` draws Liuli's magnifier, not the platform's, and defaults `enterKeyHint` to `search`. `password` hides the text and adds a button that shows it. @default 'text' */
  type?: 'text' | 'password' | 'email' | 'number' | 'tel' | 'search';
  inputMode?: 'text' | 'numeric' | 'decimal' | 'tel' | 'email';
  /**
   * The value's direction; the label and the box follow the page. `auto` takes the direction of the value's first letter,
   * so an address or a name reads in its own order on a right-to-left page.
   * @default 'ltr' for `email`, `number` and `tel`, or a numeric, tel or email `inputMode`; otherwise 'auto'
   */
  dir?: 'ltr' | 'rtl' | 'auto';
  autoComplete?: AutoComplete;
  /** The label of the keyboard's action key; `onSubmit` runs on it. */
  enterKeyHint?: 'enter' | 'done' | 'go' | 'next' | 'previous' | 'search' | 'send';
  autoCapitalize?: 'none' | 'sentences' | 'words' | 'characters';
  /** Unset follows the platform. */
  hasSpellCheck?: boolean;
  /** Focus it when it first appears. @default false */
  hasAutoFocus?: boolean;
  maxLength?: number;
  /** Shows the value without letting it be changed; it can still be selected, copied and focused. For a value that does not apply, use `isDisabled`. @default false */
  isReadOnly?: boolean;
  /** A spinner at the end of the box while the app checks or fetches the value; the field reports busy and stays editable. @default false */
  isLoading?: boolean;
  /** An icon by name from the set, or a definition, inside the box, before the text, 16 px in the muted colour. Replaces the search magnifier. */
  startIcon?: IconRef;
  /** A node in the icon's place, such as a spinner; wins over `startIcon`. */
  startContent?: Child;
  /** Fixed text inside the box before the value, as the app writes it: "$", "https://". Read with the description. */
  prefix?: string;
  /** Fixed text inside the box after the value: a unit such as "kg" or "%". Read with the description. */
  suffix?: string;
  /** A node inside the box after the value and any suffix, such as a small IconButton. The clear and show-password buttons come after it. */
  endContent?: Child;
  /** The keyboard's action key, or Enter. */
  onSubmit?: (value: string) => void;
  /** Runs when the editor takes focus. */
  onFocus?: () => void;
  /** Runs when the editor loses focus: the moment to check a value the person has finished typing. */
  onBlur?: () => void;
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
    label, isLabelHidden, description, status, isRequired, hasRequiredMark, optionalLabel, info, isDisabled = false, disabledMessage, size: ownSize,
    placeholder, type, inputMode, autoComplete, enterKeyHint, autoCapitalize, hasSpellCheck, hasAutoFocus = false, maxLength, isReadOnly = false, isLoading = false,
    startIcon, startContent, prefix, suffix, endContent, onSubmit, onFocus, onBlur, className, controlClassName, 'data-testid': testId,
  } = props;
  const size = useControlSize(ownSize);
  const text = props.bind ? props.bind.value : props.value;
  // The platform search field draws its own magnifier at its own size and colour, so search is a
  // plain editor with Liuli's icon.
  const isSearch = type === 'search';
  const dir = props.dir ?? (leftToRightTypes.has(type ?? 'text') || leftToRightTypes.has(inputMode ?? 'text') ? 'ltr' : 'auto');
  const isPassword = type === 'password';
  const revealed = useLocal('revealed', false);
  const editorType = isSearch || (isPassword && revealed.value) ? undefined : type;
  const keyHint = enterKeyHint ?? (isSearch ? 'search' : undefined);
  const leading = startContent ?? (startIcon !== undefined || isSearch ? <Glyph name={startIcon ?? 'search'} className={r.searchIcon.base} /> : undefined);
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
      info={info}
      isDisabled={isDisabled}
      disabledMessage={disabledMessage}
      size={size}
      className={className}
    >
      {control => (
        <view className={cx(r.box.base, r.box.size[size], status && r.box.status[status.type], isReadOnly && r.box.readOnly, isDisabled && r.box.disabled, controlClassName)}>
          {leading !== undefined && <view aria-hidden className={r.icon.base}>{leading}</view>}
          {prefix !== undefined && <text id={prefixId} className={cx(r.adornment.base, r.input.size[size])}>{prefix}</text>}
          {props.bind ? (
            <input
              bind={props.bind}
              aria-label={control['aria-label']}
              aria-describedby={describedBy(prefix !== undefined && prefixId, suffix !== undefined && suffixId, control['aria-describedby'])}
              aria-invalid={control['aria-invalid']}
              aria-required={control['aria-required']}
              data-testid={testId} {...dataOf(props)}
              placeholder={placeholder}
              dir={dir}
              type={editorType}
              inputMode={inputMode}
              autoComplete={autoComplete}
              enterKeyHint={keyHint}
              autoCapitalize={autoCapitalize}
              spellCheck={hasSpellCheck}
              autoFocus={hasAutoFocus}
              maxLength={maxLength}
              readOnly={isReadOnly || undefined}
              aria-busy={isLoading || undefined}
              disabled={control.disabled}
              onSubmit={onSubmit}
              onFocus={onFocus}
              onBlur={onBlur}
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
              data-testid={testId} {...dataOf(props)}
              placeholder={placeholder}
              dir={dir}
              type={editorType}
              inputMode={inputMode}
              autoComplete={autoComplete}
              enterKeyHint={keyHint}
              autoCapitalize={autoCapitalize}
              spellCheck={hasSpellCheck}
              autoFocus={hasAutoFocus}
              maxLength={maxLength}
              readOnly={isReadOnly || undefined}
              aria-busy={isLoading || undefined}
              disabled={control.disabled}
              onSubmit={onSubmit}
              onFocus={onFocus}
              onBlur={onBlur}
              className={inputClass}
            />
          )}
          {suffix !== undefined && <text id={suffixId} className={cx(r.adornment.base, r.input.size[size])}>{suffix}</text>}
          {endContent !== undefined && <view className={r.end.base}>{endContent}</view>}
          {isLoading && <vector aria-hidden paths={spinner} viewBox={spinnerBox} className={r.spinner.base} />}
          {props.hasClear && text !== '' && !isDisabled && !isReadOnly && (
            <button aria-label={props.clearLabel ?? liuliText('@liuli.clear')} onClick={clear} className={r.clear.base}>
              <Glyph name="close" className={r.clearIcon.base} />
            </button>
          )}
          {isPassword && (
            <button
              aria-label={liuliText(revealed.value ? '@liuli.hidePassword' : '@liuli.showPassword')}
              onClick={() => revealed.set(!revealed.value)}
              disabled={isDisabled}
              data-testid={testId !== undefined ? `${testId}-reveal` : undefined}
              className={r.clear.base}
            >
              <Glyph name={revealed.value ? 'hidden' : 'visible'} className={r.clearIcon.base} />
            </button>
          )}
        </view>
      )}
    </Field>
  );
}

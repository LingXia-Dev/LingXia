import { cx, type AutoComplete } from '@lingxia/view';
import { useControlSize } from '../density';
import { Field, type FieldOwnProps } from '../Field/Field';
import type { TextInputBinding } from '../TextInput/TextInput';
import { textAreaRecipe as r } from './TextArea.recipe';

export type TextAreaProps = FieldOwnProps & TextInputBinding & {
  /** A hint shown while empty; never a substitute for `label`. */
  placeholder?: string;
  /** Lines shown while empty or short. @default 3 */
  rows?: number;
  /** Lines it grows to as the text gets longer; then it scrolls. Never below `rows`. @default 8 */
  maxRows?: number;
  inputMode?: 'text' | 'numeric' | 'decimal' | 'tel' | 'email';
  autoComplete?: AutoComplete;
  /** The keyboard's action key. `send`, `go`, `done` and `search` make Return submit and Shift+Return add a line. */
  enterKeyHint?: 'enter' | 'done' | 'go' | 'next' | 'previous' | 'search' | 'send';
  autoCapitalize?: 'none' | 'sentences' | 'words' | 'characters';
  /** Unset follows the platform. */
  hasSpellCheck?: boolean;
  /** Focus it when it first appears. @default false */
  hasAutoFocus?: boolean;
  maxLength?: number;
  /** Shows how much of `maxLength` is used, at the bottom end of the box: "118 / 280". Needs `maxLength`. @default false */
  hasCount?: boolean;
  /** The keyboard's action key, or Return when `enterKeyHint` makes it submit. */
  onSubmit?: (value: string) => void;
  /** Classes for the field column. */
  className?: string;
  /** Classes for the bordered box. */
  controlClassName?: string;
  /** On the editor. */
  'data-testid'?: string;
};

/** A labelled editor for several lines, growing with its text up to `maxRows`. */
export function TextArea(props: TextAreaProps) {
  const {
    label, isLabelHidden, description, status, isRequired, hasRequiredMark, optionalLabel, info, isDisabled = false, disabledMessage, size: ownSize,
    placeholder, rows = 3, inputMode, autoComplete, enterKeyHint, autoCapitalize, hasSpellCheck,
    hasAutoFocus = false, maxLength, hasCount = false, onSubmit, className, controlClassName, 'data-testid': testId,
  } = props;
  const size = useControlSize(ownSize);
  // The View refuses maxRows below rows, so a tall `rows` lifts the default.
  const maxRows = Math.max(rows, props.maxRows ?? 8);
  // Counted as the editor counts it for `maxLength`.
  const length = (props.bind ? props.bind.value : props.value).length;
  const editorClass = cx(r.editor.base, r.editor.size[size]);
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
        <view className={cx(r.box.base, status && r.box.status[status.type], isDisabled && r.box.disabled, controlClassName)}>
          {props.bind ? (
            <textarea
              bind={props.bind}
              rows={rows}
              maxRows={maxRows}
              aria-label={control['aria-label']}
              aria-describedby={control['aria-describedby']}
              aria-invalid={control['aria-invalid']}
              aria-required={control['aria-required']}
              data-testid={testId}
              placeholder={placeholder}
              inputMode={inputMode}
              autoComplete={autoComplete}
              enterKeyHint={enterKeyHint}
              autoCapitalize={autoCapitalize}
              spellCheck={hasSpellCheck}
              autoFocus={hasAutoFocus}
              maxLength={maxLength}
              disabled={control.disabled}
              onSubmit={onSubmit}
              className={editorClass}
            />
          ) : (
            <textarea
              value={props.value}
              onInput={props.onChange}
              rows={rows}
              maxRows={maxRows}
              aria-label={control['aria-label']}
              aria-describedby={control['aria-describedby']}
              aria-invalid={control['aria-invalid']}
              aria-required={control['aria-required']}
              data-testid={testId}
              placeholder={placeholder}
              inputMode={inputMode}
              autoComplete={autoComplete}
              enterKeyHint={enterKeyHint}
              autoCapitalize={autoCapitalize}
              spellCheck={hasSpellCheck}
              autoFocus={hasAutoFocus}
              maxLength={maxLength}
              disabled={control.disabled}
              onSubmit={onSubmit}
              className={editorClass}
            />
          )}
          {hasCount && maxLength !== undefined && (
            <text aria-hidden className={cx(r.count.base, length >= maxLength && r.count.full)}>{`${length} / ${maxLength}`}</text>
          )}
        </view>
      )}
    </Field>
  );
}

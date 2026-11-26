import { cx, useLocal, type Local } from '@lingxia/view';
import { liuliText } from '../i18n/text';
import { useControlSize } from '../density';
import { Field, type FieldOwnProps } from '../Field/Field';
import { Token } from '../Token/Token';
import { tokenizerRecipe as r } from './Tokenizer.recipe';
import { dataOf } from '../internal/data';

export type TokenizerBinding =
  | {
      /** A View-local cell holding the entries; use it or `value` with `onChange`. */
      bind: Local<readonly string[]>;
      value?: never;
      onChange?: never;
    }
  | {
      bind?: never;
      /** The entries, in the order they were typed. */
      value: readonly string[];
      /** Runs with the entries after one is added or removed. */
      onChange: (value: readonly string[]) => void;
    };

export type TokenizerProps = FieldOwnProps & TokenizerBinding & {
  /** Shown in the editor while nothing is typed and nothing is entered. */
  placeholder?: string;
  /**
   * Checks an entry before it becomes a token: return the reason it is refused, shown as the field's
   * error, and the text stays in the editor to be corrected: `text => text.includes('@') ? undefined : 'Not an email address'`.
   */
  validate?: (text: string) => string | undefined;
  /** Each token's remove button, named with what it removes. Defaults to "Remove <entry>" in the page's language. */
  removeLabel?: (entry: string) => string;
  className?: string;
  /** On the editor; the box gets `<testid>-box`, each token `<testid>-token-<index>`. */
  'data-testid'?: string;
};

// A comma, a semicolon or a line break ends an entry, so a pasted list splits into tokens.
const separator = /[,;\n]/;

/** Free text turned into tokens as it is typed or pasted: addresses, tags, keywords. */
export function Tokenizer(props: TokenizerProps) {
  const {
    label, isLabelHidden, description, status, isRequired, hasRequiredMark, optionalLabel, info, isDisabled = false, disabledMessage, size: ownSize,
    placeholder, validate, removeLabel = (entry: string) => liuliText('@liuli.removeNamed', { name: entry }), className, 'data-testid': testId,
  } = props;
  const size = useControlSize(ownSize);
  const entries = props.bind ? props.bind.value : props.value;
  const draft = useLocal('draft', '');
  const refused = useLocal<string | null>('refused', null);
  const set = (next: readonly string[]) => (props.bind ? props.bind.set(next) : props.onChange(next));

  // Adds each finished piece; a refused or unfinished piece stays in the editor.
  const commit = (text: string, isFinished: boolean) => {
    const pieces = text.split(separator);
    const tail = isFinished ? '' : pieces.pop() ?? '';
    const added: string[] = [];
    const kept: string[] = [];
    let reason: string | undefined;
    for (const piece of pieces.map(each => each.trim()).filter(each => each !== '')) {
      if (entries.includes(piece) || added.includes(piece)) continue;
      const why = validate?.(piece);
      if (why === undefined) added.push(piece);
      else {
        kept.push(piece);
        reason ??= why;
      }
    }
    if (added.length > 0) set([...entries, ...added]);
    draft.set([...kept, tail.trimStart()].filter(each => each !== '').join(', '));
    refused.set(reason ?? null);
  };

  const shown = refused.value !== null ? { type: 'error' as const, message: refused.value } : status;
  return (
    <Field
      label={label}
      isLabelHidden={isLabelHidden}
      description={description}
      status={shown}
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
        <view
          role="group"
          aria-label={control['aria-label']}
          data-testid={testId && `${testId}-box`}
          className={cx(r.box.base, r.box.size[size], shown && r.box.status[shown.type], isDisabled && r.box.disabled)}
        >
          {entries.map((entry, index) => (
            <Token
              key={entry}
              label={entry}
              size="sm"
              isDisabled={isDisabled}
              removeLabel={removeLabel(entry)}
              onRemove={() => set(entries.filter(each => each !== entry))}
              data-testid={testId && `${testId}-token-${index}`}
            />
          ))}
          <input
            aria-label={control['aria-label']}
            aria-describedby={control['aria-describedby']}
            aria-invalid={control['aria-invalid']}
            aria-required={control['aria-required']}
            value={draft.value}
            placeholder={entries.length === 0 ? placeholder : undefined}
            onInput={text => {
              if (refused.value !== null) refused.set(null);
              if (separator.test(text)) commit(text, false);
              else draft.set(text);
            }}
            onKeyDown={{ Enter: () => { commit(draft.value, true); return true; } }}
            onBlur={() => { if (draft.value.trim() !== '') commit(draft.value, true); }}
            disabled={control.disabled}
            data-testid={testId} {...dataOf(props)}
            className={cx(r.editor.base, r.editor.size[size])}
          />
        </view>
      )}
    </Field>
  );
}

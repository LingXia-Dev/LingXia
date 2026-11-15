import { cx, useId, useLocal, useView, type Local } from '@lingxia/view';
import { Field, type FieldOwnProps, type FieldSize } from '../Field/Field';
import { TextInput } from '../TextInput/TextInput';
import { Glyph } from '../internal/Glyph';
import { LayerContext } from '../internal/layer';
import { spinner, spinnerBox } from '../internal/spinner';
import { comboboxRecipe as r } from './Combobox.recipe';

export interface ComboboxOption {
  /** What `value` and `onChange` carry. */
  value: string;
  /** The text in the box and in the list, and what the search matches. */
  label: string;
  /** A second, quieter line in the list. */
  description?: string;
  /** @default false */
  isDisabled?: boolean;
}

export type ComboboxBinding =
  | {
      /** A View-local cell the combobox reads and writes; use it or `value` with `onChange`. */
      bind: Local<string>;
      value?: never;
      onChange?: never;
    }
  | {
      bind?: never;
      /** The chosen option's value; `''` shows the placeholder. */
      value: string;
      /** Runs with the chosen option's value. */
      onChange: (value: string) => void;
    };

export type ComboboxProps = FieldOwnProps & ComboboxBinding & {
  /** The choices. By default the list shows those whose label contains what was typed, whatever its case. */
  options: readonly ComboboxOption[];
  /** Shown in the box while nothing is chosen. */
  placeholder?: string;
  /** The search field's accessible name: "Search countries". */
  searchLabel: string;
  /** Shown in the search field while it is empty. */
  searchPlaceholder?: string;
  /** Shown in place of the list when no option matches: "No country matches". */
  emptyLabel: string;
  /** Shown under the list when more than fifty options match: "Keep typing to see more". */
  moreLabel?: string;
  /** Names a close button in the sheet a finger gets; without it the sheet closes by a tap outside, a drag or Back. */
  closeLabel?: string;
  /**
   * Runs with each change of the search text. With it the list shows `options` as given, so the
   * app does the matching: by its own rule, or by asking a server.
   */
  onQueryChange?: (query: string) => void;
  /** Shows a spinner in place of the list while the app fetches options. @default false */
  isLoading?: boolean;
  className?: string;
  /** On the trigger; the search field is `<id>-search`, an option `<id>-<value>`. */
  'data-testid'?: string;
};

// A list a person scans is short; past this the search narrows it.
const maxShown = 50;

interface PanelProps {
  label: string;
  options: readonly ComboboxOption[];
  current: string;
  size: FieldSize;
  searchLabel: string;
  searchPlaceholder?: string;
  emptyLabel: string;
  moreLabel?: string;
  onQueryChange?: (query: string) => void;
  isLoading: boolean;
  hasAutoFocus: boolean;
  listClass: string;
  testId?: string;
  onChoose: (value: string) => void;
}

/** The search field and the list. It lives inside the popover or sheet, so what was typed is forgotten when that closes. */
function ComboboxPanel({
  label, options, current, size, searchLabel, searchPlaceholder, emptyLabel, moreLabel, onQueryChange, isLoading, hasAutoFocus, listClass, testId, onChoose,
}: PanelProps) {
  const query = useLocal('query', '');
  const typed = query.value.trim().toLowerCase();
  const matches = onQueryChange ? options : options.filter(option => option.label.toLowerCase().includes(typed));
  const shown = matches.slice(0, maxShown);
  const first = shown.find(option => !option.isDisabled);
  return (
    <LayerContext.Provider value="overlay">
      <view className={r.list.box}>
        <TextInput
          label={searchLabel}
          isLabelHidden
          type="search"
          size={size}
          placeholder={searchPlaceholder}
          value={query.value}
          onChange={text => { query.set(text); onQueryChange?.(text); }}
          // Return takes the first match, so a choice needs no pointer and no arrow keys.
          onSubmit={() => { if (first) onChoose(first.value); }}
          hasAutoFocus={hasAutoFocus}
          data-testid={testId && `${testId}-search`}
        />
        <scroll className={listClass}>
          {!isLoading && shown.length > 0 && (
            <view role="listbox" aria-label={label} className={r.list.box}>
              {shown.map(option => (
                <button
                  key={option.value}
                  role="option"
                  aria-selected={option.value === current}
                  disabled={option.isDisabled}
                  onClick={() => onChoose(option.value)}
                  data-testid={testId && `${testId}-${option.value}`}
                  className={cx(r.option.base, r.option.size[size])}
                >
                  <view className={r.optionText.base}>
                    <text className={r.optionLabel.base}>{option.label}</text>
                    {option.description !== undefined && <text className={r.optionDescription.base}>{option.description}</text>}
                  </view>
                  {option.value === current && <Glyph name="check" weight={2.5} className={r.optionCheck.base} />}
                </button>
              ))}
            </view>
          )}
          {isLoading && <vector aria-hidden paths={spinner} viewBox={spinnerBox} className={r.spinner.base} />}
          {!isLoading && shown.length === 0 && <text role="status" className={r.note.base}>{emptyLabel}</text>}
          {!isLoading && matches.length > shown.length && moreLabel !== undefined && <text className={r.note.base}>{moreLabel}</text>}
        </scroll>
      </view>
    </LayerContext.Provider>
  );
}

/** One choice from a long list, found by typing: a field that opens a search over its options. */
export function Combobox(props: ComboboxProps) {
  const {
    label, isLabelHidden, description, status, isRequired, hasRequiredMark, optionalLabel, isDisabled = false, disabledMessage, size = 'md',
    options, placeholder, searchLabel, searchPlaceholder, emptyLabel, moreLabel, closeLabel, onQueryChange, isLoading = false,
    className, 'data-testid': testId,
  } = props;
  const { env } = useView();
  // A finger gets a sheet with room for the keyboard; a mouse gets a panel under the field.
  const isSheet = env.pointer === 'coarse';
  const open = useLocal('open', false);
  const id = useId();
  const [panelId, valueId] = [`${id}-panel`, `${id}-value`];
  const current = props.bind ? props.bind.value : props.value;
  const chosen = options.find(option => option.value === current);
  const choose = (value: string) => {
    if (props.bind) props.bind.set(value);
    else props.onChange(value);
    open.set(false);
  };
  const panel = (listClass: string) => (
    <ComboboxPanel
      label={label}
      options={options}
      current={current}
      size={size}
      searchLabel={searchLabel}
      searchPlaceholder={searchPlaceholder}
      emptyLabel={emptyLabel}
      moreLabel={moreLabel}
      onQueryChange={onQueryChange}
      isLoading={isLoading}
      hasAutoFocus={!isSheet}
      listClass={listClass}
      testId={testId}
      onChoose={choose}
    />
  );
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
        <>
          <button
            popoverTarget={isSheet ? undefined : panelId}
            onClick={isSheet ? () => open.set(true) : undefined}
            aria-label={control['aria-label']}
            // The chosen option is read after the name, as a field's value is, then the help text.
            aria-describedby={control['aria-describedby'] ? `${valueId} ${control['aria-describedby']}` : valueId}
            aria-expanded={isSheet ? open.value : undefined}
            disabled={control.disabled}
            data-testid={testId}
            className={cx(r.trigger.base, r.trigger.size[size], status && r.trigger.status[status.type])}
          >
            <text id={valueId} className={cx(chosen ? r.value.base : r.value.placeholder, r.value.size[size], isDisabled && r.value.disabled)}>
              {chosen?.label ?? placeholder ?? ''}
            </text>
            <Glyph name="chevron-down" className={r.chevron.base} />
          </button>
          {isSheet ? (
            <dialog
              open={open.value}
              onClose={() => open.set(false)}
              closedby="any"
              aria-label={label}
              data-testid={testId && `${testId}-panel`}
              className={r.sheet.base}
              style={{ paddingBottom: env.safeArea.bottom + 16 }}
            >
              <view aria-hidden className={r.grabber.base} />
              <view className={r.header.base}>
                <text role="heading" headingLevel={2} className={r.title.base}>{label}</text>
                {closeLabel !== undefined && (
                  <button aria-label={closeLabel} onClick={() => open.set(false)} data-testid={testId && `${testId}-close`} className={r.close.base}>
                    <Glyph name="close" className={r.close.icon} />
                  </button>
                )}
              </view>
              {panel(r.list.sheet)}
            </dialog>
          ) : (
            <view id={panelId} popover aria-label={label} open={open.value} onToggle={open.set} data-testid={testId && `${testId}-panel`} className={r.popover.base}>
              {panel(r.list.popover)}
            </view>
          )}
        </>
      )}
    </Field>
  );
}

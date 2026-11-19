import { cx, reveal, useBox, useId, useLocal, useView, type Local } from '@lingxia/view';
import { useControlSize } from '../density';
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
  items: readonly ComboboxOption[];
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
   * Runs with each change of the search text. With it the list shows `items` as given, so the
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

const inWord = /[\p{L}\p{N}]/u;

/** How well a label matches: 0 from its start, 1 from the start of a later word, 2 inside a word, -1 not at all. */
function rank(label: string, typed: string): number {
  const text = label.toLowerCase();
  let at = text.indexOf(typed);
  if (at <= 0) return at;
  let best = 2;
  for (; at >= 0 && best === 2; at = text.indexOf(typed, at + 1)) if (!inWord.test(text[at - 1])) best = 1;
  return best;
}

/** The options whose label holds what was typed, best match first, otherwise in the order given. */
function ranked(items: readonly ComboboxOption[], typed: string): readonly ComboboxOption[] {
  if (typed === '') return items;
  const tiers: ComboboxOption[][] = [[], [], []];
  for (const option of items) tiers[rank(option.label, typed)]?.push(option);
  return tiers.flat();
}

interface OptionsProps {
  label: string;
  items: readonly ComboboxOption[];
  current: string;
  size: FieldSize;
  testId?: string;
  onChoose: (value: string) => void;
  /** The list's id and the active option's index, for a field that keeps focus while its list is open. */
  listId?: string;
  active?: number;
}

const optionId = (listId: string, index: number) => `${listId}-${index}`;

/** The options as a listbox; the chosen one carries a tick, the active one a wash. */
function Options({ label, items, current, size, testId, onChoose, listId, active }: OptionsProps) {
  return (
    <view id={listId} role="listbox" aria-label={label} className={r.list.box}>
      {items.map((option, index) => (
        <button
          key={option.value}
          id={listId === undefined ? undefined : optionId(listId, index)}
          role="option"
          aria-selected={option.value === current}
          disabled={option.isDisabled}
          onClick={() => onChoose(option.value)}
          data-testid={testId && `${testId}-${option.value}`}
          className={cx(r.option.base, r.option.size[size], index === active && r.option.active)}
        >
          <view className={r.optionText.base}>
            <text className={r.optionLabel.base}>{option.label}</text>
            {option.description !== undefined && <text className={r.optionDescription.base}>{option.description}</text>}
          </view>
          {option.value === current && <Glyph name="check" weight={2.5} className={r.optionCheck.base} />}
        </button>
      ))}
    </view>
  );
}

interface PanelProps {
  label: string;
  items: readonly ComboboxOption[];
  current: string;
  size: FieldSize;
  searchLabel: string;
  searchPlaceholder?: string;
  emptyLabel: string;
  moreLabel?: string;
  onQueryChange?: (query: string) => void;
  isLoading: boolean;
  listClass: string;
  testId?: string;
  onChoose: (value: string) => void;
}

/** The sheet's search field and list. It lives inside the sheet, so what was typed is forgotten when that closes. */
function ComboboxPanel({
  label, items, current, size, searchLabel, searchPlaceholder, emptyLabel, moreLabel, onQueryChange, isLoading, listClass, testId, onChoose,
}: PanelProps) {
  const query = useLocal('query', '');
  const typed = query.value.trim().toLowerCase();
  const matches = onQueryChange ? items : ranked(items, typed);
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
          data-testid={testId && `${testId}-search`}
        />
        <scroll className={listClass}>
          {!isLoading && shown.length > 0 && <Options label={label} items={shown} current={current} size={size} testId={testId} onChoose={onChoose} />}
          {isLoading && <vector aria-hidden paths={spinner} viewBox={spinnerBox} className={r.spinner.base} />}
          {!isLoading && shown.length === 0 && <text role="status" className={r.note.base}>{emptyLabel}</text>}
          {!isLoading && matches.length > shown.length && moreLabel !== undefined && <text className={r.note.base}>{moreLabel}</text>}
        </scroll>
      </view>
    </LayerContext.Provider>
  );
}

// The field box's side padding and borders, in points: what its measured content width leaves out.
const boxEdges = 26;

interface TypeaheadProps {
  control: { 'aria-label': string; 'aria-describedby'?: string; 'aria-invalid'?: boolean; 'aria-required'?: boolean; disabled?: boolean };
  label: string;
  items: readonly ComboboxOption[];
  current: string;
  size: FieldSize;
  statusType?: 'error' | 'warning' | 'success';
  isDisabled: boolean;
  placeholder?: string;
  emptyLabel: string;
  moreLabel?: string;
  onQueryChange?: (query: string) => void;
  isLoading: boolean;
  testId?: string;
  onChange: (value: string) => void;
}

/**
 * The field for a mouse and keyboard: the box is the editor. Typing narrows a list that hangs
 * under it while focus stays in the text; the arrow keys move through the list and Return chooses.
 */
function ComboboxTypeahead({
  control, label, items, current, size, statusType, isDisabled, placeholder, emptyLabel, moreLabel, onQueryChange, isLoading, testId, onChange,
}: TypeaheadProps) {
  const id = useId();
  const [boxId, listId, panelId] = [`${id}-box`, `${id}-list`, `${id}-panel`];
  const box = useBox('field');
  const open = useLocal('open', false);
  // What was typed since the list opened; `null` while the box still shows the choice, when the list shows every option.
  const query = useLocal<string | null>('query', null);
  const active = useLocal('active', -1);
  const chosen = items.find(option => option.value === current);
  const typed = (query.value ?? '').trim().toLowerCase();
  const matches = onQueryChange || query.value === null ? items : ranked(items, typed);
  const shown = matches.slice(0, maxShown);
  const close = () => { open.set(false); query.set(null); active.set(-1); };
  const choose = (value: string) => { onChange(value); close(); };
  const show = (index: number) => { open.set(true); active.set(index); if (index >= 0) reveal(optionId(listId, index)); };
  // The next option that can be chosen, from `from` in `direction`, stopping at the ends.
  const step = (from: number, direction: 1 | -1) => {
    for (let index = from + direction; index >= 0 && index < shown.length; index += direction) if (!shown[index].isDisabled) return index;
    return from;
  };
  const first = () => step(-1, 1);
  const last = () => step(shown.length, -1);
  const isShowing = open.value && !isDisabled;
  const keys = {
    ArrowDown: () => show(isShowing ? step(active.value, 1) : Math.max(first(), shown.findIndex(option => option.value === current))),
    ArrowUp: () => show(isShowing && active.value >= 0 ? step(active.value, -1) : last()),
    // Listed only while the list is open: closed, they belong to the text.
    ...(isShowing ? {
      Home: () => show(first()),
      End: () => show(last()),
      // Return takes the active option, or the first match of what was typed.
      Enter: () => { const pick = shown[active.value] ?? (query.value !== null ? shown[first()] : undefined); if (pick && !pick.isDisabled) choose(pick.value); },
      Escape: close,
    } : {}),
  };
  return (
    <>
      <view id={boxId} box={box} className={cx(r.trigger.base, r.trigger.size[size], r.trigger.editable, statusType && r.trigger.status[statusType], isDisabled && r.trigger.off)}>
        <input
          role="combobox"
          aria-autocomplete="list"
          aria-expanded={isShowing}
          aria-controls={listId}
          aria-activedescendant={isShowing && active.value >= 0 ? optionId(listId, active.value) : undefined}
          aria-label={control['aria-label']}
          aria-describedby={control['aria-describedby']}
          aria-invalid={control['aria-invalid']}
          aria-required={control['aria-required']}
          value={query.value ?? chosen?.label ?? ''}
          placeholder={placeholder}
          onInput={text => { query.set(text); open.set(true); active.set(-1); onQueryChange?.(text); }}
          onFocus={() => open.set(true)}
          // Leaving with the text emptied clears the choice; anything else typed and not chosen is dropped.
          onBlur={() => { if (query.value !== null && query.value.trim() === '' && current !== '') onChange(''); close(); }}
          onKeyDown={keys}
          disabled={control.disabled}
          data-testid={testId}
          className={cx(r.editor.base, r.value.size[size])}
        />
        <Glyph name="chevron-down" className={r.chevron.base} />
      </view>
      {/* A manual popover takes no focus and has no backdrop, so the text keeps the keys while the list is open. */}
      <view
        id={panelId}
        popover="manual"
        anchor={boxId}
        open={isShowing}
        onToggle={value => { if (!value) close(); }}
        data-testid={testId && `${testId}-panel`}
        className={r.popover.typeahead}
        style={box.value ? { width: box.value.width + boxEdges } : undefined}
      >
        <LayerContext.Provider value="overlay">
          <scroll className={r.list.popover}>
            {!isLoading && shown.length > 0 && <Options label={label} items={shown} current={current} size={size} testId={testId} onChoose={choose} listId={listId} active={active.value} />}
            {isLoading && <vector aria-hidden paths={spinner} viewBox={spinnerBox} className={r.spinner.base} />}
            {!isLoading && shown.length === 0 && <text role="status" className={r.note.base}>{emptyLabel}</text>}
            {!isLoading && matches.length > shown.length && moreLabel !== undefined && <text className={r.note.base}>{moreLabel}</text>}
          </scroll>
        </LayerContext.Provider>
      </view>
    </>
  );
}

/** One choice from a long list, found by typing: in the field itself with a keyboard, in a sheet with a finger. */
export function Combobox(props: ComboboxProps) {
  const {
    label, isLabelHidden, description, status, isRequired, hasRequiredMark, optionalLabel, info, isDisabled = false, disabledMessage, size: ownSize,
    items, placeholder, searchLabel, searchPlaceholder, emptyLabel, moreLabel, closeLabel, onQueryChange, isLoading = false,
    className, 'data-testid': testId,
  } = props;
  const size = useControlSize(ownSize);
  const { env } = useView();
  // A finger gets a sheet with room for the keyboard; a mouse and keyboard type in the field itself.
  const isSheet = env.pointer === 'coarse';
  const open = useLocal('open', false);
  const id = useId();
  const valueId = `${id}-value`;
  const current = props.bind ? props.bind.value : props.value;
  const chosen = items.find(option => option.value === current);
  const choose = (value: string) => {
    if (props.bind) props.bind.set(value);
    else props.onChange(value);
    open.set(false);
  };
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
      {control => (!isSheet ? (
        <ComboboxTypeahead
          control={control}
          label={label}
          items={items}
          current={current}
          size={size}
          statusType={status?.type}
          isDisabled={isDisabled}
          placeholder={placeholder}
          emptyLabel={emptyLabel}
          moreLabel={moreLabel}
          onQueryChange={onQueryChange}
          isLoading={isLoading}
          testId={testId}
          onChange={choose}
        />
      ) : (
        <>
          <button
            onClick={() => open.set(true)}
            aria-label={control['aria-label']}
            // The chosen option is read after the name, as a field's value is, then the help text.
            aria-describedby={control['aria-describedby'] ? `${valueId} ${control['aria-describedby']}` : valueId}
            aria-expanded={open.value}
            disabled={control.disabled}
            data-testid={testId}
            className={cx(r.trigger.base, r.trigger.size[size], r.trigger.pressable, status && r.trigger.status[status.type])}
          >
            <text id={valueId} className={cx(chosen ? r.value.base : r.value.placeholder, r.value.size[size], isDisabled && r.value.disabled)}>
              {chosen?.label ?? placeholder ?? ''}
            </text>
            <Glyph name="chevron-down" className={r.chevron.base} />
          </button>
          <dialog
            open={open.value}
            onClose={() => open.set(false)}
            closedby="any"
            aria-label={label}
            data-testid={testId && `${testId}-panel`}
            className={r.sheet.base}
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
            <ComboboxPanel
              label={label}
              items={items}
              current={current}
              size={size}
              searchLabel={searchLabel}
              searchPlaceholder={searchPlaceholder}
              emptyLabel={emptyLabel}
              moreLabel={moreLabel}
              onQueryChange={onQueryChange}
              isLoading={isLoading}
              listClass={r.list.sheet}
              testId={testId}
              onChoose={choose}
            />
          </dialog>
        </>
      ))}
    </Field>
  );
}

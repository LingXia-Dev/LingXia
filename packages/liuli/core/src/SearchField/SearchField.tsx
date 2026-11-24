import { cx, type LocalText } from '@lingxia/view';
import { liuliText } from '../i18n/text';
import { useControlSize } from '../density';
import type { FieldSize } from '../Field/Field';
import { TextInput } from '../TextInput/TextInput';
import { spinner, spinnerBox } from '../internal/spinner';
import { searchFieldRecipe as r } from './SearchField.recipe';
import { dataOf } from '../internal/data';

// What a native search input holds at its natural width: 20 characters.
const natural = '0'.repeat(20);

export type SearchFieldBinding =
  | {
      /** A text cell (`useLocal('')`) the field reads and writes; its `focus()` focuses the field. Use it or `value` with `onChange`. */
      bind: LocalText;
      value?: never;
      onChange?: never;
    }
  | {
      bind?: never;
      /** The search text; pass `onChange` with it. */
      value: string;
      /** Runs with each change of the text. */
      onChange: (value: string) => void;
    };

export type SearchFieldProps = SearchFieldBinding & {
  /** The accessible name: what is searched ("Search notes"), drawn above the field unless `isLabelHidden`. */
  label: string;
  /** Reads the label without drawing it: a search field at the top of a list is usually named by its place and placeholder alone. @default false */
  isLabelHidden?: boolean;
  /** A hint shown while empty: "Search notes". */
  placeholder?: string;
  /** The clear button's accessible name: "Clear search". The button shows while there is text. Defaults to "Clear search" in the page's language. */
  clearLabel?: string;
  /** What the search found, as the app words it: "12 notes", "No notes match". Announced when it changes. */
  resultsLabel?: string;
  /** Keeps `resultsLabel` for assistive tech only, when the list under the field already shows it. @default false */
  isResultsLabelHidden?: boolean;
  /** Shows a spinner in place of the magnifier while a search runs. @default false */
  isLoading?: boolean;
  /** Runs on the keyboard's search key, or Enter. */
  onSubmit?: (value: string) => void;
  /** Focus it when it first appears. @default false */
  hasAutoFocus?: boolean;
  /** @default 'md' */
  size?: FieldSize;
  /** @default false */
  isDisabled?: boolean;
  className?: string;
  /** On the editor. */
  'data-testid'?: string;
};

/** A field for searching what is on the page: a magnifier, the text, a clear button, and a line that says what was found. */
export function SearchField(props: SearchFieldProps) {
  const {
    label, isLabelHidden = false, placeholder, clearLabel = liuliText('@liuli.clearSearch'), resultsLabel, isResultsLabelHidden = false, isLoading = false, onSubmit,
    hasAutoFocus = false, size: ownSize, isDisabled = false, className, 'data-testid': testId,
  } = props;
  const size = useControlSize(ownSize);
  const shared = {
    label,
    isLabelHidden,
    type: 'search',
    placeholder,
    hasClear: true,
    clearLabel,
    startContent: isLoading ? <vector aria-hidden paths={spinner} viewBox={spinnerBox} className={r.spinner.base} /> : undefined,
    onSubmit,
    hasAutoFocus,
    size,
    isDisabled,
    'data-testid': testId, ...dataOf(props),
  } as const;
  return (
    <view aria-busy={isLoading || undefined} className={cx(r.root.base, className)}>
      {props.bind ? <TextInput {...shared} bind={props.bind} /> : <TextInput {...shared} value={props.value} onChange={props.onChange} />}
      <text aria-hidden className={cx(r.natural.base, r.natural.size[size])}>{natural}</text>
      {/* Always mounted: a status region announces a change, not its first appearance. */}
      <text role="status" data-testid={testId && `${testId}-results`} className={cx(r.results.base, (isResultsLabelHidden || resultsLabel === undefined) && r.results.hidden)}>
        {resultsLabel ?? ''}
      </text>
    </view>
  );
}

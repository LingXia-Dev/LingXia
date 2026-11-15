import { cx, type Local } from '@lingxia/view';
import type { FieldSize } from '../Field/Field';
import { TextInput } from '../TextInput/TextInput';
import { spinner, spinnerBox } from '../internal/spinner';
import { searchFieldRecipe as r } from './SearchField.recipe';

export type SearchFieldBinding =
  | {
      /** A View-local cell the field reads and writes; use it or `value` with `onChange`. */
      bind: Local<string>;
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
  /** The accessible name: what is searched ("Search notes"). Drawn only with `isLabelShown`. */
  label: string;
  /** Draws the label above the field. A search field at the top of a list is usually named by its placeholder's place alone. @default false */
  isLabelShown?: boolean;
  /** A hint shown while empty: "Search notes". */
  placeholder?: string;
  /** The clear button's accessible name: "Clear search". The button shows while there is text. */
  clearLabel: string;
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
    label, isLabelShown = false, placeholder, clearLabel, resultsLabel, isResultsLabelHidden = false, isLoading = false, onSubmit,
    hasAutoFocus = false, size = 'md', isDisabled = false, className, 'data-testid': testId,
  } = props;
  const shared = {
    label,
    isLabelHidden: !isLabelShown,
    type: 'search',
    placeholder,
    hasClear: true,
    clearLabel,
    startIcon: isLoading ? <vector aria-hidden paths={spinner} viewBox={spinnerBox} className={r.spinner.base} /> : undefined,
    onSubmit,
    hasAutoFocus,
    size,
    isDisabled,
    'data-testid': testId,
  } as const;
  return (
    <view aria-busy={isLoading || undefined} className={cx(r.root.base, className)}>
      {props.bind ? <TextInput {...shared} bind={props.bind} /> : <TextInput {...shared} value={props.value} onChange={props.onChange} />}
      {/* Always mounted: a status region announces a change, not its first appearance. */}
      <text role="status" data-testid={testId && `${testId}-results`} className={cx(r.results.base, (isResultsLabelHidden || resultsLabel === undefined) && r.results.hidden)}>
        {resultsLabel ?? ''}
      </text>
    </view>
  );
}

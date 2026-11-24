// Type test, checked by `tsc`: a text field binds a text cell, so code can focus the field through the
// cell it is bound to, as the docs show (`if (!ok) password.focus();`).
import type { Local } from '@lingxia/view';
import { Composer, type ComposerProps } from '../Composer/Composer';
import { SearchField, type SearchFieldProps } from '../SearchField/SearchField';
import { TextArea, type TextAreaProps } from '../TextArea/TextArea';
import { TextInput, type TextInputProps } from '../TextInput/TextInput';

type BindOf<P> = Extract<P, { bind: unknown }>['bind'];

export function focusAfterAFailedSubmit(cells: {
  password: BindOf<TextInputProps>;
  notes: BindOf<TextAreaProps>;
  draft: BindOf<ComposerProps>;
  query: BindOf<SearchFieldProps>;
}) {
  cells.password.focus();
  cells.notes.focus();
  cells.draft.focus();
  cells.query.focus();
}

declare const chosen: Local<string>;
export const refused = [
  // @ts-expect-error a cell that is not a text cell has no editor to focus
  <TextInput label="Password" bind={chosen} />,
  // @ts-expect-error
  <TextArea label="Notes" bind={chosen} />,
  // @ts-expect-error
  <Composer label="Message" bind={chosen} />,
  // @ts-expect-error
  <SearchField label="Search" bind={chosen} />,
];

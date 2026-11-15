import { useLocal } from '@lingxia/view';
import { Item, ItemList } from '../Item/Item';
import { SearchField } from './SearchField';

const notes = ['Budget for Q3', 'Kitchen plans', 'Reading list', 'Recipes to try', 'Trip to Lisbon', 'Weekly review'];

/** The list under the field narrows as you type, and the line between them says how many are left. */
export function Notes() {
  const query = useLocal('query', '');
  const text = query.value.trim().toLowerCase();
  const found = notes.filter(note => note.toLowerCase().includes(text));
  return (
    <view className="gap-3">
      <SearchField
        label="Search notes"
        placeholder="Search notes"
        clearLabel="Clear search"
        bind={query}
        resultsLabel={text === '' ? undefined : found.length === 0 ? 'No notes match' : `${found.length} of ${notes.length} notes`}
        data-testid="notes"
      />
      <ItemList label="Notes">
        {found.map(note => <Item key={note} label={note} />)}
      </ItemList>
    </view>
  );
}

/** While a search runs elsewhere, the magnifier gives way to a spinner. */
export function Loading() {
  return <SearchField label="Search the catalogue" placeholder="Search the catalogue" clearLabel="Clear search" value="lamp" onChange={() => {}} isLoading resultsLabel="Searching" data-testid="catalogue" />;
}

/** With a label above, where the field is one of several on a form. */
export function Labelled() {
  const query = useLocal('order', '');
  return <SearchField label="Find an order" isLabelShown placeholder="Order number or name" clearLabel="Clear" bind={query} data-testid="order" />;
}

export function Sizes() {
  const query = useLocal('sized', '');
  return (
    <view className="gap-3">
      <SearchField label="Search, small" placeholder="Small" clearLabel="Clear" size="sm" bind={query} data-testid="small" />
      <SearchField label="Search, medium" placeholder="Medium" clearLabel="Clear" bind={query} />
      <SearchField label="Search, large" placeholder="Large" clearLabel="Clear" size="lg" bind={query} data-testid="large" />
    </view>
  );
}

export function StateEmpty() {
  return <SearchField label="Search notes" placeholder="Search notes" clearLabel="Clear search" value="" onChange={() => {}} />;
}

export function StateFilled() {
  return <SearchField label="Search notes" placeholder="Search notes" clearLabel="Clear search" value="lisbon" onChange={() => {}} resultsLabel="1 of 6 notes" data-testid="filled" />;
}

export function StateNothing() {
  return <SearchField label="Search notes" placeholder="Search notes" clearLabel="Clear search" value="zebra" onChange={() => {}} resultsLabel="No notes match" />;
}

export function StateDisabled() {
  return <SearchField label="Search notes" placeholder="Search notes" clearLabel="Clear search" value="" onChange={() => {}} isDisabled data-testid="disabled" />;
}

/** The home tile. */
export function Specimen() {
  return <SearchField label="Search" placeholder="Search" clearLabel="Clear" value="" onChange={() => {}} size="sm" className="w-32" />;
}

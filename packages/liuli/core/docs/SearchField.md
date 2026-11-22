<!-- Generated from the doc file by scripts/generate.mjs; do not edit. -->

# SearchField

A field for searching what is on the page, with a clear button and a line that says what was found. Status: preview.

```ts
import { SearchField } from '@liulidesign/core/SearchField';
```

SearchField is the field at the top of a list or a catalogue: a magnifier, the text, a button that clears it, and under it a line the app words to say what the search found. That line is announced when it changes, so a person who cannot see the list still learns the result. While a search runs elsewhere, a spinner takes the magnifier's place.

## Examples

### A list that narrows as you type

```tsx
import { useLocal } from '@lingxia/view';
import { Item, ItemList } from '@liulidesign/core/Item';
import { SearchField } from '@liulidesign/core/SearchField';

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
        isLabelHidden
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
```

### While a search runs

```tsx
import { Held } from '../example';
import { SearchField } from '@liulidesign/core/SearchField';

/** While a search runs elsewhere, the magnifier gives way to a spinner. */
export function Loading() {
  return <Held value="lamp">{(query, set) => <SearchField label="Search the catalogue" isLabelHidden placeholder="Search the catalogue" clearLabel="Clear search" value={query} onChange={set} isLoading resultsLabel="Searching" data-testid="catalogue" />}</Held>;
}
```

### With its label shown

```tsx
import { useLocal } from '@lingxia/view';
import { SearchField } from '@liulidesign/core/SearchField';

/** With a label above, where the field is one of several on a form. */
export function Labelled() {
  const query = useLocal('order', '');
  return <SearchField label="Find an order" placeholder="Order number or name" clearLabel="Clear" bind={query} data-testid="order" />;
}
```

### Sizes

```tsx
import { useLocal } from '@lingxia/view';
import { SearchField } from '@liulidesign/core/SearchField';

export function Sizes() {
  const query = useLocal('sized', '');
  return (
    <view className="gap-3">
      <SearchField label="Search, small" isLabelHidden placeholder="Small" clearLabel="Clear" size="sm" bind={query} data-testid="small" />
      <SearchField label="Search, medium" isLabelHidden placeholder="Medium" clearLabel="Clear" bind={query} />
      <SearchField label="Search, large" isLabelHidden placeholder="Large" clearLabel="Clear" size="lg" bind={query} data-testid="large" />
    </view>
  );
}
```

## Props

| Prop | Type | Default | Description |
| --- | --- | --- | --- |
| `bind` | `Local<string>` |  | A View-local cell the field reads and writes; use it or `value` with `onChange`. |
| `value` | `string` |  | The search text; pass `onChange` with it. |
| `onChange` | `((value: string) => void)` |  | Runs with each change of the text. |
| `label` (required) | `string` |  | The accessible name: what is searched ("Search notes"), drawn above the field unless `isLabelHidden`. |
| `isLabelHidden` | `boolean` | `false` | Reads the label without drawing it: a search field at the top of a list is usually named by its place and placeholder alone. |
| `placeholder` | `string` |  | A hint shown while empty: "Search notes". |
| `clearLabel` | `string` |  | The clear button's accessible name: "Clear search". The button shows while there is text. Defaults to "Clear search" in the page's language. |
| `resultsLabel` | `string` |  | What the search found, as the app words it: "12 notes", "No notes match". Announced when it changes. |
| `isResultsLabelHidden` | `boolean` | `false` | Keeps `resultsLabel` for assistive tech only, when the list under the field already shows it. |
| `isLoading` | `boolean` | `false` | Shows a spinner in place of the magnifier while a search runs. |
| `onSubmit` | `((value: string) => void)` |  | Runs on the keyboard's search key, or Enter. |
| `hasAutoFocus` | `boolean` | `false` | Focus it when it first appears. |
| `size` | `FieldSize` | `'md'` |  |
| `isDisabled` | `boolean` | `false` |  |
| `className` | `string` |  |  |
| `data-testid` | `string` |  | On the editor. |

## Anatomy

| Part | Element | Required | Description |
| --- | --- | --- | --- |
| root | `<view>` | yes | The column: the field, then the results line. It is reported busy while loading. |
| spinner | `<vector>` | no | Takes the magnifier's place while `isLoading`. |
| results | `<text>` | yes | `resultsLabel`: drawn under the field, or kept for assistive tech only; a status region, always mounted. |

## What an app may override

- `className`: classes merged after the recipe's, so they win.
- The recipe, `searchFieldRecipe` from `@liulidesign/core/SearchField/recipe`: the classes of each part, to style an element of your own to match. The component does not read a changed copy.
- Scale tokens it reads, set in the app's stylesheet under `@theme`: `--text-footnote`.
- Colour roles it paints with, set by the theme pack: `text-muted`.

## Accessibility

- Pattern: textbox.
- Name: label.
- Description: none.
- States: busy (from isLoading); disabled (from isDisabled).
- Enter, or the keyboard's search key: Runs `onSubmit` with the text.
- Tab: Moves from the text to the clear button while there is text.
- Press target: sm 32 px, md 40 px, lg 48 px.
- A search editor named by `label`; a placeholder is never the name, so `label` is required even when it is not drawn.
- The results line is a status region: each new `resultsLabel` is announced. Word it for the ear: "3 of 6 notes", "No notes match".
- The clear button is named by `clearLabel` and appears only while there is text.
- The on-screen keyboard's action key reads "search".

| Contrast | Pair | Minimum | Light | Dark |
| --- | --- | --- | --- | --- |
| text | `text` on `surface` | 4.5:1 | 16.26:1 | 13.87:1 |
| placeholder, magnifier and results line | `text-muted` on `surface` | 4.5:1 | 5.89:1 | 6.64:1 |
| results line on the page | `text-muted` on `canvas` | 4.5:1 | 5.50:1 | 7.33:1 |
| field edge | `line-strong` on `surface` | 3:1 | 3.47:1 | 3.53:1 |

Ratios are computed from @lingxia/view/theme and rounded down.

## Guidance

- Do: Filter as the person types when the list is on the device; use `onSubmit` when a search costs a request.
- Do: Always pass `resultsLabel` once there is text, including when nothing matches.
- Do: Hide the results line with `isResultsLabelHidden` only when the list under the field already shows the count.
- Do: Keep the field where it is while results change, so typing is never interrupted.
- Do: At large text, give the field a line of its own rather than squeezing it beside a button, where it shrinks to its magnifier.
- Don't: Use it to choose one value for a form; that is a Combobox.
- Don't: Clear the text when a search finds nothing; let the person edit it.
- Don't: Show a spinner for filtering that happens at once.

## Not yet

| Missing | Needs | Meanwhile |
| --- | --- | --- |
| Recent searches and suggestions under the field while typing. | a popover anchored to an editor and opened from state, with key events from the editor | The page shows its own results under the field. |
| A cancel button beside the field that ends the search, as a phone's search bar has. | focus and blur events on an editor | The clear button empties the text; the page decides what an empty search shows. |

Related: TextInput, Combobox.

<!-- Generated from the doc file by scripts/generate.mjs; do not edit. -->

# Combobox

One choice from a long list, found by typing: a field that opens a search over its options. Status: preview.

```ts
import { Combobox } from '@liulidesign/core/Combobox';
```

Combobox is a Select for lists too long to scan: countries, people, products. The field shows the chosen option; pressing it opens a search field over the list, in a panel under the field for a mouse and in a sheet from the bottom edge for a finger. Typing narrows the list, and Return takes the first match. Matching ignores case and looks in the label only: options whose label starts with what was typed come first, then those where a later word starts with it, then those that hold it inside a word, each group in the order the options were given. With `onQueryChange` the app replaces that: Combobox reports each change to the search text and shows `options` as passed, so the app filters and orders them by its own rule or a server's. It takes Field's label, help text and status.

## Examples

### A long list, found by typing

```tsx
import { useLocal } from '@lingxia/view';
import { Combobox } from '@liulidesign/core/Combobox';

const countries = [
  'Argentina', 'Australia', 'Brazil', 'Canada', 'Chile', 'China', 'Egypt', 'France', 'Germany', 'India', 'Indonesia', 'Italy', 'Japan',
  'Kenya', 'Mexico', 'Morocco', 'Netherlands', 'Nigeria', 'Norway', 'Peru', 'Portugal', 'South Africa', 'South Korea', 'Spain', 'Sweden', 'Türkiye',
  'United Kingdom', 'United States', 'Vietnam',
].map(name => ({ value: name.toLowerCase().replace(/[^a-z]+/g, '-'), label: name }));

/** A long list found by typing: the field opens a search over its options. */
export function Country() {
  const country = useLocal('country', '');
  return (
    <Combobox
      label="Country"
      bind={country}
      options={countries}
      placeholder="Choose a country"
      searchLabel="Search countries"
      searchPlaceholder="Search"
      emptyLabel="No country matches"
      closeLabel="Close"
      data-testid="country"
    />
  );
}
```

### Options with a second line

```tsx
import { useLocal } from '@lingxia/view';
import { Combobox } from '@liulidesign/core/Combobox';

const people = [
  { value: 'ana', label: 'Ana Lee', description: 'Design' },
  { value: 'ben', label: 'Ben Okafor', description: 'Engineering' },
  { value: 'chloe', label: 'Chloé Martin', description: 'Engineering' },
  { value: 'dev', label: 'Dev Patel', description: 'On leave until June', isDisabled: true },
  { value: 'emi', label: 'Emi Sato', description: 'Research' },
];

/** A second line tells apart options with similar names; one that cannot be chosen stays in the list, dimmed. */
export function Assignee() {
  const person = useLocal('person', 'ben');
  return (
    <Combobox
      label="Assignee"
      description="They are notified when the task changes."
      bind={person}
      options={people}
      searchLabel="Search people"
      emptyLabel="Nobody by that name"
      data-testid="assignee"
    />
  );
}
```

### The app does the matching

```tsx
import { useLocal } from '@lingxia/view';
import { Combobox } from '@liulidesign/core/Combobox';

const airports = [
  { value: 'LIS', label: 'Lisbon', description: 'LIS · Humberto Delgado' },
  { value: 'LHR', label: 'London', description: 'LHR · Heathrow' },
  { value: 'LGW', label: 'London', description: 'LGW · Gatwick' },
  { value: 'NBO', label: 'Nairobi', description: 'NBO · Jomo Kenyatta' },
  { value: 'OSL', label: 'Oslo', description: 'OSL · Gardermoen' },
];

/** The app does the matching itself: here a code finds its airport as well as a city does. */
export function OwnMatching() {
  const airport = useLocal('airport', '');
  const typed = useLocal('typed', '');
  const text = typed.value.trim().toLowerCase();
  return (
    <Combobox
      label="Airport"
      bind={airport}
      options={airports.filter(each => each.label.toLowerCase().includes(text) || each.value.toLowerCase().includes(text))}
      placeholder="City or code"
      searchLabel="Search airports"
      emptyLabel="No airport matches"
      onQueryChange={typed.set}
      data-testid="airport"
    />
  );
}
```

### Sizes

```tsx
import { useLocal } from '@lingxia/view';
import { Combobox } from '@liulidesign/core/Combobox';

const countries = [
  'Argentina', 'Australia', 'Brazil', 'Canada', 'Chile', 'China', 'Egypt', 'France', 'Germany', 'India', 'Indonesia', 'Italy', 'Japan',
  'Kenya', 'Mexico', 'Morocco', 'Netherlands', 'Nigeria', 'Norway', 'Peru', 'Portugal', 'South Africa', 'South Korea', 'Spain', 'Sweden', 'Türkiye',
  'United Kingdom', 'United States', 'Vietnam',
].map(name => ({ value: name.toLowerCase().replace(/[^a-z]+/g, '-'), label: name }));

export function Sizes() {
  const value = useLocal('value', 'kenya');
  return (
    <view className="gap-3">
      <Combobox label="Small" size="sm" bind={value} options={countries} searchLabel="Search countries" emptyLabel="No country matches" data-testid="small" />
      <Combobox label="Medium" bind={value} options={countries} searchLabel="Search countries" emptyLabel="No country matches" />
      <Combobox label="Large" size="lg" bind={value} options={countries} searchLabel="Search countries" emptyLabel="No country matches" data-testid="large" />
    </view>
  );
}
```

### In a form

```tsx
import { useLocal } from '@lingxia/view';
import { Button } from '@liulidesign/core/Button';
import { Combobox } from '@liulidesign/core/Combobox';

const countries = [
  'Argentina', 'Australia', 'Brazil', 'Canada', 'Chile', 'China', 'Egypt', 'France', 'Germany', 'India', 'Indonesia', 'Italy', 'Japan',
  'Kenya', 'Mexico', 'Morocco', 'Netherlands', 'Nigeria', 'Norway', 'Peru', 'Portugal', 'South Africa', 'South Korea', 'Spain', 'Sweden', 'Türkiye',
  'United Kingdom', 'United States', 'Vietnam',
].map(name => ({ value: name.toLowerCase().replace(/[^a-z]+/g, '-'), label: name }));

/** In a form, beside the fields it belongs with. */
export function Shipping() {
  const country = useLocal('ship-country', 'portugal');
  const saved = useLocal('saved', '');
  return (
    <view className="gap-4">
      <Combobox label="Ship to" bind={country} options={countries} searchLabel="Search countries" emptyLabel="No country matches" isRequired data-testid="ship-to" />
      <Button label="Save address" variant="primary" width="content" onClick={() => saved.set(country.value)} data-testid="save-address" />
      <text role="status" data-testid="saved" className="text-footnote text-muted">{saved.value === '' ? 'Not saved' : `Saved: ${saved.value}`}</text>
    </view>
  );
}
```

## Props

| Prop | Type | Default | Description |
| --- | --- | --- | --- |
| `label` (required) | `string` |  | The visible label and the control's accessible name. |
| `isLabelHidden` | `boolean` | `false` | Hides the visible label; the control keeps it as its name. |
| `description` | `string` |  | Help text under the control, read as the control's description. |
| `status` | `FieldStatus` |  | An `error` status marks the control invalid; its message is read as the control's description. |
| `isRequired` | `boolean` | `false` | Marks the label with an asterisk and the control as required. Validation is the app's. |
| `hasRequiredMark` | `boolean` | `true` | Draws the asterisk on a required label; turn it off on a form where every field is required. |
| `optionalLabel` | `string` |  | A word after the label for a field that may be left empty, as the app writes it: "optional". Read before the help text. |
| `isDisabled` | `boolean` | `false` |  |
| `disabledMessage` | `string` |  | Shown in place of `description` while disabled. |
| `size` | `FieldSize` | `'md'` |  |
| `bind` | `Local<string>` |  | A View-local cell the combobox reads and writes; use it or `value` with `onChange`. |
| `value` | `string` |  | The chosen option's value; `''` shows the placeholder. |
| `onChange` | `((value: string) => void)` |  | Runs with the chosen option's value. |
| `options` (required) | `readonly ComboboxOption[]` |  | The choices. By default the list shows those whose label contains what was typed, whatever its case. |
| `placeholder` | `string` |  | Shown in the box while nothing is chosen. |
| `searchLabel` (required) | `string` |  | The search field's accessible name: "Search countries". |
| `searchPlaceholder` | `string` |  | Shown in the search field while it is empty. |
| `emptyLabel` (required) | `string` |  | Shown in place of the list when no option matches: "No country matches". |
| `moreLabel` | `string` |  | Shown under the list when more than fifty options match: "Keep typing to see more". |
| `closeLabel` | `string` |  | Names a close button in the sheet a finger gets; without it the sheet closes by a tap outside, a drag or Back. |
| `onQueryChange` | `((query: string) => void)` |  | Runs with each change of the search text. With it the list shows `options` as given, so the app does the matching: by its own rule, or by asking a server. |
| `isLoading` | `boolean` | `false` | Shows a spinner in place of the list while the app fetches options. |
| `className` | `string` |  |  |
| `data-testid` | `string` |  | On the trigger; the search field is `<id>-search`, an option `<id>-<value>`. |

## Anatomy

| Part | Element | Required | Description |
| --- | --- | --- | --- |
| trigger | `<button>` | yes | The field: the chosen option's label, or the placeholder, and a chevron. The button assistive tech names by `label`. |
| value | `<text>` | yes | The chosen option's label; read after the name, as a field's value is. |
| popover | `<view>` | no | With a mouse or trackpad: a panel under the field holding the search field and the list. |
| sheet | `<dialog>` | no | On touch: a sheet on the bottom edge, titled by `label`, holding the same. |
| list | `<scroll>` | yes | The matching options, scrolling under the search field, as a listbox; at most fifty are shown. |
| option | `<button>` | yes | One option: its label, an optional second line, and a tick on the chosen one. |
| note | `<text>` | no | `emptyLabel` when nothing matches, or `moreLabel` under a list cut at fifty. |

## Accessibility

- Pattern: button.
- Name: label.
- Description: the chosen option, then description, then status message.
- States: expanded (from the open panel or sheet); selected (from value, on the chosen option); disabled (from isDisabled); modal (from the sheet on touch).
- Enter or Space: On the field: opens the panel with focus in the search field.
- Return: In the search field: chooses the first match and closes.
- Tab, then Up and Down: From the search field into the list, and between options; Enter chooses.
- Escape: Closes without changing the choice; focus returns to the field.
- Press target: sm 32 px, md 40 px, lg 48 px.
- The field is a button named by `label`; the chosen option, then the help text and status, are read as its description. It reports expanded while open.
- The search field is named by `searchLabel`. The options are a listbox named by `label`, each an option that reports selected; `emptyLabel` is announced when nothing matches.
- On touch the sheet is modal and titled by `label`; a tap outside, a drag down or Back closes it. With a mouse a click outside or Escape closes the panel.
- Arrow keys do not move through the options while focus is in the search field: Return takes the first match, or Tab moves into the list.

| Contrast | Pair | Minimum | Light | Dark |
| --- | --- | --- | --- | --- |
| chosen option in the field | `text` on `surface` | 4.5:1 | 16.26:1 | 13.87:1 |
| placeholder | `text-muted` on `surface` | 4.5:1 | 5.89:1 | 6.64:1 |
| field edge | `line-strong` on `surface` | 3:1 | 3.66:1 | 3.69:1 |
| option in the panel | `text` on `surface-overlay` | 4.5:1 | 16.26:1 | 10.10:1 |
| option's second line and the notes | `text-muted` on `surface-overlay` | 4.5:1 | 5.89:1 | 4.84:1 |
| tick on the chosen option | `accent` on `surface-overlay` | 3:1 | 5.62:1 | 4.51:1 |
| error edge | `danger` on `surface` | 3:1 | 5.21:1 | 5.42:1 |

Ratios are computed from @lingxia/view/theme and rounded down.

## Guidance

- Do: Use it from about fifteen options up; below that a Select is quicker.
- Do: Order options as people expect to find them: by name, or most used first.
- Do: Write `emptyLabel` for what was searched: "No country matches".
- Do: Use `onQueryChange` to match more than the label (a code, an email) or to fetch options; show `isLoading` while fetching.
- Don't: Use it for free text with suggestions; the field only ever holds one of its options.
- Don't: Put actions in the list; an option chooses, nothing else.
- Don't: Load thousands of options into it; fetch by what was typed.

## Not yet

| Missing | Needs | Meanwhile |
| --- | --- | --- |
| Typing in the field itself with suggestions under it, and arrow keys moving through them while typing. | a popover anchored to an editor and opened from state, key events from an editor or an active option it points at, and the combobox role | The field is a button; the search field is inside the panel or sheet, and Return takes the first match. |
| A panel as wide as its field. | reading a laid-out box's size (`useBox`) | The panel is 288 points wide. |
| Several choices at once, and clearing the choice. | nothing from the View | One choice; an app that needs "none" adds an option for it. |

Related: Select, TextInput, DropdownMenu.

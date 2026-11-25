import { useLocal } from '@lingxia/view';
import { Held, unused } from '../example';
import { Button } from '../Button/Button';
import { Combobox, type ComboboxProps } from './Combobox';

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
      items={countries}
      placeholder="Choose a country"
      searchLabel="Search countries"
      searchPlaceholder="Search"
      emptyLabel="No country matches"
      hasCloseButton
      data-testid="country"
    />
  );
}

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
      items={people}
      searchLabel="Search people"
      emptyLabel="Nobody by that name"
      data-testid="assignee"
    />
  );
}

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
      items={airports.filter(each => each.label.toLowerCase().includes(text) || each.value.toLowerCase().includes(text))}
      placeholder="City or code"
      searchLabel="Search airports"
      emptyLabel="No airport matches"
      onQueryChange={typed.set}
      data-testid="airport"
    />
  );
}

export function Sizes() {
  const value = useLocal('value', 'kenya');
  return (
    <view className="gap-3">
      <Combobox label="Small" size="sm" bind={value} items={countries} searchLabel="Search countries" emptyLabel="No country matches" data-testid="small" />
      <Combobox label="Medium" bind={value} items={countries} searchLabel="Search countries" emptyLabel="No country matches" />
      <Combobox label="Large" size="lg" bind={value} items={countries} searchLabel="Search countries" emptyLabel="No country matches" data-testid="large" />
    </view>
  );
}

export function StateEmpty() {
  return <Held value="">{(country, set) => <Combobox label="Country" value={country} onChange={set} items={countries} placeholder="Choose a country" searchLabel="Search countries" emptyLabel="No country matches" />}</Held>;
}

export function StateChosen() {
  return <Held value="kenya">{(country, set) => <Combobox label="Country" value={country} onChange={set} items={countries} searchLabel="Search countries" emptyLabel="No country matches" data-testid="chosen" />}</Held>;
}

export function StateInvalid() {
  return (
    <Held value="">
      {(country, set) => (
        <Combobox
          label="Country"
          value={country}
          onChange={set}
          items={countries}
          placeholder="Choose a country"
          status={country === '' ? { type: 'error', message: 'Choose where to ship to.' } : undefined}
          isRequired
          searchLabel="Search countries"
          emptyLabel="No country matches"
          data-testid="invalid"
        />
      )}
    </Held>
  );
}

export function StateDisabled() {
  return <Combobox label="Country" value="kenya" onChange={unused} items={countries} isDisabled searchLabel="Search countries" emptyLabel="No country matches" data-testid="disabled" />;
}

/** In a form, beside the fields it belongs with. */
export function Shipping() {
  const country = useLocal('ship-country', 'portugal');
  const saved = useLocal('saved', '');
  return (
    <view className="gap-4">
      <Combobox label="Ship to" bind={country} items={countries} searchLabel="Search countries" emptyLabel="No country matches" isRequired data-testid="ship-to" />
      <Button label="Save address" variant="primary" width="content" onClick={() => saved.set(country.value)} data-testid="save-address" />
      {saved.value !== '' && <text role="status" data-testid="saved" className="text-footnote text-muted">{`Saved: ${saved.value}`}</text>}
    </view>
  );
}

/** A choice that can be taken back: the clear button empties it. */
export function Clearable() {
  const country = useLocal('country', 'portugal');
  return <Combobox label="Country" bind={country} items={countries} placeholder="Choose a country" hasClear data-testid="clearable" />;
}

/** The home tile. */
export function Specimen() {
  return <Combobox label="Country" isLabelHidden value="kenya" onChange={unused} items={countries} size="sm" searchLabel="Search countries" emptyLabel="No country matches" className="w-32" />;
}

/** The playground's props that are not plain values. */
export function playground() {
  return { items: countries } satisfies Partial<ComboboxProps>;
}

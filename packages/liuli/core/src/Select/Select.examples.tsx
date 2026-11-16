import { useLocal } from '@lingxia/view';
import { Held, unused } from '../example';
import { Select } from './Select';

const countries = [
  { value: 'de', label: 'Germany' },
  { value: 'fr', label: 'France' },
  { value: 'it', label: 'Italy' },
  { value: 'nl', label: 'Netherlands' },
  { value: 'es', label: 'Spain' },
];

/** A label, a placeholder until chosen, and help text. */
export function Country() {
  const country = useLocal('country', '');
  return (
    <Select
      bind={country}
      label="Country"
      placeholder="Choose a country"
      description="Sets the currency and the tax on your invoices."
      options={countries}
      data-testid="country"
    />
  );
}

export function Sizes() {
  const zone = useLocal('zone', 'cet');
  const options = [{ value: 'cet', label: 'Central European Time' }, { value: 'utc', label: 'Coordinated Universal Time' }];
  return (
    <view className="gap-4">
      <Select bind={zone} label="Small" size="sm" options={options} />
      <Select bind={zone} label="Medium" options={options} />
      <Select bind={zone} label="Large" size="lg" options={options} />
    </view>
  );
}

/** Required: an error after a send with nothing chosen, cleared by a choice. */
export function Required() {
  const role = useLocal('role', '');
  const tried = useLocal('tried', true);
  return (
    <Select
      bind={role}
      label="Role"
      isRequired
      placeholder="Choose a role"
      options={[{ value: 'viewer', label: 'Can view' }, { value: 'editor', label: 'Can edit' }, { value: 'owner', label: 'Owner', isDisabled: true }]}
      status={tried.value && role.value === '' ? { type: 'error', message: 'Choose a role for the invite.' } : undefined}
      description="Only an owner can add another owner."
      data-testid="role"
    />
  );
}

const sorts = [{ value: 'edited', label: 'Last edited' }, { value: 'created', label: 'Date created' }, { value: 'title', label: 'Title' }];

export function StateEmpty() {
  return <Held value="">{(sort, set) => <Select value={sort} onChange={set} label="Sort by" placeholder="Choose" options={sorts} />}</Held>;
}

export function StateChosen() {
  return <Held value="edited">{(sort, set) => <Select value={sort} onChange={set} label="Sort by" options={sorts} />}</Held>;
}

export function StateInvalid() {
  return <Held value="">{(sort, set) => <Select value={sort} onChange={set} label="Sort by" placeholder="Choose" options={sorts} status={sort === '' ? { type: 'error', message: 'Choose one.' } : undefined} />}</Held>;
}

export function StateDisabled() {
  return <Select value="edited" onChange={unused} label="Sort by" options={sorts} isDisabled disabledMessage="Set by the folder." />;
}

/** The home tile. */
export function Specimen() {
  return <Select value="title" onChange={unused} label="Sort by" isLabelHidden size="sm" options={sorts} className="w-32 max-w-full" />;
}

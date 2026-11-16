import { useLocal } from '@lingxia/view';
import { unused } from '../example';
import { MultiSelect } from './MultiSelect';

const labels = [
  { value: 'travel', label: 'Travel' },
  { value: 'work', label: 'Work' },
  { value: 'recipes', label: 'Recipes' },
  { value: 'garden', label: 'Garden' },
  { value: 'money', label: 'Money' },
  { value: 'archive', label: 'Archive', isDisabled: true },
];

/** Labels for a note: the chosen ones are tokens in the field, the list opens from its end. */
export function Labels() {
  const chosen = useLocal<readonly string[]>('chosen', ['travel', 'recipes']);
  return (
    <MultiSelect
      bind={chosen}
      label="Labels"
      options={labels}
      addLabel="Add a label"
      removeLabel={name => `Remove ${name}`}
      description="A note can carry several."
      data-testid="labels"
    />
  );
}

/** Nothing chosen yet, on a required field after a failed send. */
export function Empty() {
  const chosen = useLocal<readonly string[]>('none', []);
  return (
    <MultiSelect
      bind={chosen}
      label="Share with teams"
      options={[{ value: 'design', label: 'Design' }, { value: 'eng', label: 'Engineering' }, { value: 'support', label: 'Support' }]}
      placeholder="No teams yet"
      addLabel="Add a team"
      removeLabel={name => `Remove ${name}`}
      isRequired
      status={chosen.value.length === 0 ? { type: 'error', message: 'Choose at least one team.' } : undefined}
      data-testid="teams"
    />
  );
}

export function StateDisabled() {
  return <MultiSelect value={['travel']} onChange={unused} label="Labels" options={labels} addLabel="Add a label" removeLabel={name => `Remove ${name}`} isDisabled disabledMessage="Labels are set by the owner." />;
}

/** The home tile. */
export function Specimen() {
  return <MultiSelect value={['travel']} onChange={unused} label="Labels" isLabelHidden options={labels} addLabel="Add a label" removeLabel={name => `Remove ${name}`} className="w-36" />;
}

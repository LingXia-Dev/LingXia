import { useLocal } from '@lingxia/view';
import { unused } from '../example';
import { CheckboxList } from './CheckboxList';

const topics = [
  { value: 'mentions', label: 'Mentions', description: 'When someone names you in a note.' },
  { value: 'comments', label: 'Comments', description: 'Replies on notes you wrote.' },
  { value: 'digest', label: 'Weekly digest' },
];

/** Any number of the options can be on; the value is the list of those that are. */
export function Notifications() {
  const chosen = useLocal<readonly string[]>('topics', ['mentions']);
  return (
    <view className="gap-3">
      <CheckboxList label="Notify me about" bind={chosen} items={topics} data-testid="topics" />
      {chosen.value.length > 0 && <text role="status" data-testid="chosen" className="text-footnote text-muted">{chosen.value.join(', ')}</text>}
    </view>
  );
}

const days = ['Mon', 'Tue', 'Wed', 'Thu', 'Fri'].map(day => ({ value: day.toLowerCase(), label: day }));

/** A message about the group as a whole, when the choice so far cannot stand. */
export function AtLeastOne() {
  const chosen = useLocal<readonly string[]>('days', []);
  return (
    <CheckboxList
      label="Office days"
      description="We book a desk for each day you choose."
      isRequired
      bind={chosen}
      items={days}
      status={chosen.value.length === 0 ? { type: 'error', message: 'Choose at least one day.' } : undefined}
      data-testid="days"
    />
  );
}

const toppings = [
  { value: 'olives', label: 'Olives' },
  { value: 'mushrooms', label: 'Mushrooms' },
  { value: 'anchovies', label: 'Anchovies', description: 'Sold out today.', isDisabled: true },
];

export function WithUnavailable() {
  const chosen = useLocal<readonly string[]>('toppings', ['olives']);
  return <CheckboxList label="Toppings" bind={chosen} items={toppings} data-testid="toppings" />;
}

export function Sizes() {
  const chosen = useLocal<readonly string[]>('sized', ['a']);
  const two = [{ value: 'a', label: 'First' }, { value: 'b', label: 'Second' }];
  return (
    <view className="grid grid-cols-2 gap-4">
      <CheckboxList label="Small" size="sm" bind={chosen} items={two} data-testid="small" />
      <CheckboxList label="Large" size="lg" bind={chosen} items={two} data-testid="large" />
    </view>
  );
}

export function StateDisabled() {
  return <CheckboxList label="Notify me about" value={['mentions']} onChange={unused} items={topics.slice(0, 2)} isDisabled data-testid="disabled" />;
}

/** The home tile. */
export function Specimen() {
  return <CheckboxList label="Days" isLabelHidden value={['a']} onChange={unused} items={[{ value: 'a', label: 'Mentions' }, { value: 'b', label: 'Comments' }]} size="sm" />;
}

/** A box for all of them: checked when all are, a dash when some are. */
export function SelectAll() {
  const chosen = useLocal<readonly string[]>('folders', ['travel', 'recipes']);
  return (
    <CheckboxList
      bind={chosen}
      label="Folders to export"
      selectAllLabel="All folders"
      items={[{ value: 'travel', label: 'Travel' }, { value: 'work', label: 'Work' }, { value: 'recipes', label: 'Recipes' }, { value: 'garden', label: 'Garden' }]}
      data-testid="export"
    />
  );
}

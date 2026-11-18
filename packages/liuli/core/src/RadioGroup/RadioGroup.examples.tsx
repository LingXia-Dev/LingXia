import { useLocal } from '@lingxia/view';
import { Held, unused } from '../example';
import { RadioGroup } from './RadioGroup';

const deliveries = [
  { value: 'standard', label: 'Standard', description: '3 to 5 working days, free' },
  { value: 'express', label: 'Express', description: 'Next working day, €6' },
  { value: 'pickup', label: 'Pick up in store', description: 'Ready in two hours' },
];

/** Options with a second line each. */
export function Delivery() {
  const delivery = useLocal<string | null>('delivery', 'standard');
  return <RadioGroup bind={delivery} label="Delivery" items={deliveries} data-testid="delivery" />;
}

const plans = [
  { value: 'free', label: 'Free', description: 'Up to 3 notebooks, on one device' },
  { value: 'plus', label: 'Plus · €4 a month', description: 'Unlimited notebooks, sync across devices' },
  { value: 'team', label: 'Team · €8 a person', description: 'Shared notebooks, admin controls' },
];

/** Each option a card: the whole card chooses it. */
export function Plans() {
  const plan = useLocal<string | null>('plan', 'plus');
  return <RadioGroup bind={plan} label="Plan" items={plans} variant="card" data-testid="plan" />;
}

/** Short options, no second line. */
export function Plain() {
  const theme = useLocal<string | null>('theme', 'system');
  return (
    <RadioGroup
      bind={theme}
      label="Appearance"
      description="System follows the device."
      items={[{ value: 'system', label: 'System' }, { value: 'light', label: 'Light' }, { value: 'dark', label: 'Dark' }]}
      data-testid="appearance"
    />
  );
}

export function Sizes() {
  const pick = useLocal<string | null>('pick', 'a');
  const options = [{ value: 'a', label: 'Chosen' }, { value: 'b', label: 'Not chosen' }];
  return (
    <view className="gap-5">
      <RadioGroup bind={pick} label="Small" size="sm" items={options} />
      <RadioGroup bind={pick} label="Medium" items={options} />
      <RadioGroup bind={pick} label="Large" size="lg" items={options} />
    </view>
  );
}

/** Nothing chosen yet, required, after a failed send. */
export function Required() {
  const reason = useLocal<string | null>('reason', null);
  return (
    <RadioGroup
      bind={reason}
      label="Why are you leaving?"
      isRequired
      status={reason.value === null ? { type: 'error', message: 'Choose a reason.' } : undefined}
      items={[{ value: 'price', label: 'It costs too much' }, { value: 'features', label: 'It is missing something I need' }, { value: 'other', label: 'Something else' }]}
      data-testid="reason"
    />
  );
}

const pair = [{ value: 'monthly', label: 'Monthly' }, { value: 'yearly', label: 'Yearly' }];

export function StateDefault() {
  return <Held value={'monthly' as string | null}>{(billing, set) => <RadioGroup value={billing} onChange={set} label="Billing" items={pair} />}</Held>;
}

export function StateInvalid() {
  return <Held value={null as string | null}>{(billing, set) => <RadioGroup value={billing} onChange={set} label="Billing" items={pair} status={billing === null ? { type: 'error', message: 'Choose one.' } : undefined} />}</Held>;
}

export function StateDisabledOption() {
  return <Held value={'monthly' as string | null}>{(billing, set) => <RadioGroup value={billing} onChange={set} label="Billing" items={[pair[0], { ...pair[1], isDisabled: true }]} />}</Held>;
}

export function StateDisabled() {
  return <RadioGroup value="monthly" onChange={unused} label="Billing" items={pair} isDisabled disabledMessage="Set by your team." />;
}

/** The home tile. */
export function Specimen() {
  return <RadioGroup value="a" onChange={unused} label="Plan" isLabelHidden size="sm" items={[{ value: 'a', label: 'Monthly' }, { value: 'b', label: 'Yearly' }]} />;
}

/** Two short options side by side. */
export function Inline() {
  const billing = useLocal<string | null>('billing', 'yearly');
  return (
    <RadioGroup
      bind={billing}
      label="Billing"
      orientation="horizontal"
      items={[{ value: 'monthly', label: 'Monthly' }, { value: 'yearly', label: 'Yearly' }]}
      description="Yearly saves two months."
      data-testid="billing"
    />
  );
}

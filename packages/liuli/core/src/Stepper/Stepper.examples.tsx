import { useLocal } from '@lingxia/view';
import { Button } from '../Button/Button';
import { Stepper } from './Stepper';

const states = { done: 'Done', current: 'Current step', upcoming: 'Not started' };
const checkout = [
  { key: 'cart', label: 'Cart', description: 'Three items' },
  { key: 'address', label: 'Address', description: 'Where to deliver' },
  { key: 'payment', label: 'Payment', description: 'Card or transfer' },
  { key: 'review', label: 'Review', description: 'Check and place the order' },
];

/** A task in four steps: finished steps go back, the button moves on. */
export function Checkout() {
  const at = useLocal('at', 1);
  return (
    <view className="gap-4">
      <Stepper label="Checkout steps" steps={checkout} current={at.value} stateLabels={states} onStepClick={at.set} data-testid="checkout" />
      <Button label={at.value >= checkout.length - 1 ? 'Place order' : 'Continue'} variant="primary" onClick={() => at.set(Math.min(checkout.length, at.value + 1))} data-testid="continue" />
    </view>
  );
}

/** Across the top of a wide form: up to four short labels. */
export function Across() {
  return <Stepper label="Sign-up steps" orientation="horizontal" steps={[{ key: 'account', label: 'Account' }, { key: 'profile', label: 'Profile' }, { key: 'done', label: 'Done' }]} current={1} stateLabels={states} data-testid="across" />;
}

/** The home tile. */
export function Specimen() {
  return <Stepper label="Steps" orientation="horizontal" steps={[{ key: 'a', label: 'One' }, { key: 'b', label: 'Two' }, { key: 'c', label: 'Three' }]} current={1} stateLabels={states} className="w-32" />;
}

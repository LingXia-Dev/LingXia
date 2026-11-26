import { useLocal } from '@lingxia/view';
import { Button } from '../Button/Button';
import { Stepper, type StepperProps } from './Stepper';

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
      <Stepper label="Checkout steps" steps={checkout} bind={at} data-testid="checkout" />
      <Button label={at.value >= checkout.length - 1 ? 'Place order' : 'Continue'} variant="primary" onClick={() => at.set(Math.min(checkout.length, at.value + 1))} data-testid="continue" />
    </view>
  );
}

/** Across the top of a wide form: up to four short labels. */
export function Across() {
  return <Stepper label="Sign-up steps" orientation="horizontal" steps={[{ key: 'account', label: 'Account' }, { key: 'profile', label: 'Profile' }, { key: 'done', label: 'Done' }]} value={1} data-testid="across" />;
}

/** A payment the bank refused: that step is marked to fix, and the address, once sent to the courier, cannot be changed. */
export function NeedsAttention() {
  const at = useLocal('at', 3);
  const steps = [
    { key: 'cart', label: 'Cart', description: 'Three items' },
    { key: 'address', label: 'Address', description: 'Sent to the courier', isDisabled: true },
    { key: 'payment', label: 'Payment', description: 'The bank declined the card', status: 'error' as const },
    { key: 'review', label: 'Review', description: 'Check and place the order' },
  ];
  return <Stepper label="Checkout steps" steps={steps} bind={at} data-testid="attention" />;
}

/** The home tile. */
export function Specimen() {
  return <Stepper label="Steps" orientation="horizontal" steps={[{ key: 'a', label: 'One' }, { key: 'b', label: 'Two' }, { key: 'c', label: 'Three' }]} value={1} className="w-32" />;
}

/** The playground's props that are not plain values. */
export function playground() {
  return { steps: checkout } satisfies Partial<StepperProps>;
}

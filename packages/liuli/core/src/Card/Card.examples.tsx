import { useLocal } from '@lingxia/view';
import { Button } from '../Button/Button';
import { Card, ClickableCard } from './Card';

export function Variants() {
  return (
    <view className="gap-3">
      <Card data-testid="outlined">
        <text className="text-base font-semibold text-text">Outlined</text>
        <text className="text-sm text-muted">Sits on the page.</text>
      </Card>
      <Card variant="raised">
        <text className="text-base font-semibold text-text">Raised</text>
        <text className="text-sm text-muted">Floats above the page.</text>
      </Card>
      <Card variant="sunken">
        <text className="text-base font-semibold text-text">Sunken</text>
        <text className="text-sm text-muted">Groups content inside another surface.</text>
      </Card>
    </view>
  );
}

/** Actions go in a plain Card, never in a ClickableCard. */
export function WithActions() {
  return (
    <Card>
      <text role="heading" headingLevel={3} className="text-base font-semibold text-text">Back up your notes</text>
      <text className="text-sm text-muted">Keep a copy on another device in case this one is lost.</text>
      <view className="flex-row justify-end gap-2">
        <Button label="Not now" variant="ghost" size="sm" />
        <Button label="Turn on" variant="primary" size="sm" data-testid="turn-on" />
      </view>
    </Card>
  );
}

const plans = [
  { id: 'monthly', name: 'Monthly', price: '4 a month' },
  { id: 'yearly', name: 'Yearly', price: '40 a year' },
] as const;

/** Choosing one of several: the chosen card is reported as selected. */
export function Choice() {
  const plan = useLocal('plan', 'monthly');
  return (
    <view className="flex-row gap-3">
      {plans.map(item => (
        <ClickableCard
          key={item.id}
          label={`${item.name}, ${item.price}`}
          isSelected={plan.value === item.id}
          onClick={() => plan.set(item.id)}
          className="grow basis-0"
          data-testid={`plan-${item.id}`}
        >
          <text className="text-base font-semibold text-text">{item.name}</text>
          <text className="text-sm text-muted">{item.price}</text>
        </ClickableCard>
      ))}
    </view>
  );
}

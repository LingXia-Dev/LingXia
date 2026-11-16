import { useLocal } from '@lingxia/view';
import { Held, Said, unused } from '../example';
import { Button } from '../Button/Button';
import { Card, ClickableCard, ClickableCardGroup } from './Card';

export function Variants() {
  return (
    <view className="gap-3">
      <Card data-testid="outlined">
        <text className="text-callout font-semibold text-text">Outlined</text>
        <text className="text-footnote text-muted">Sits on the page.</text>
      </Card>
      <Card variant="raised">
        <text className="text-callout font-semibold text-text">Raised</text>
        <text className="text-footnote text-muted">Rests one step above the page.</text>
      </Card>
      <Card variant="sunken">
        <text className="text-callout font-semibold text-text">Sunken</text>
        <text className="text-footnote text-muted">Groups content inside another surface.</text>
      </Card>
    </view>
  );
}

/** Actions go in a plain Card, never in a ClickableCard. */
export function WithActions() {
  return (
    <Said>
      {say => (
        <Card>
          <text role="heading" headingLevel={3} className="text-callout font-semibold text-text">Back up your notes</text>
          <text className="text-footnote text-muted">Keep a copy on another device in case this one is lost.</text>
          <view className="flex-row justify-end gap-2">
            <Button label="Not now" variant="ghost" size="sm" onClick={() => say('Put off the backup')} />
            <Button label="Turn on" variant="primary" size="sm" onClick={() => say('Turned on backups')} data-testid="turn-on" />
          </view>
        </Card>
      )}
    </Said>
  );
}

const plans = [
  { id: 'monthly', name: 'Monthly', price: '€4 a month' },
  { id: 'yearly', name: 'Yearly', price: '€40 a year' },
] as const;

/** Choosing one of several: the cards are a radio group, and the chosen one is checked. */
export function Choice() {
  const plan = useLocal<string | null>('plan', 'monthly');
  return (
    <ClickableCardGroup label="Plan" bind={plan} className="flex-row" data-testid="plans">
      {plans.map(item => (
        <ClickableCard
          key={item.id}
          value={item.id}
          label={`${item.name}, ${item.price}`}
          className="grow basis-0"
          data-testid={`plan-${item.id}`}
        >
          <text className="text-callout font-semibold text-text">{item.name}</text>
          <text className="text-footnote text-muted">{item.price}</text>
        </ClickableCard>
      ))}
    </ClickableCardGroup>
  );
}

/** The home tile. */
export function Specimen() {
  return (
    <Card padding="sm" className="w-28">
      <text className="text-footnote font-semibold text-text">Weekly report</text>
      <text className="text-caption text-muted">12 notes</text>
    </Card>
  );
}

export function StateUnselected() {
  return (
    <Held value={false}>
      {(isSelected, select) => (
        <ClickableCard label="Monthly" isSelected={isSelected} onClick={() => select(!isSelected)} padding="sm">
          <text className="text-body font-medium text-text">Monthly</text>
        </ClickableCard>
      )}
    </Held>
  );
}

export function StateSelected() {
  return (
    <Held value={true}>
      {(isSelected, select) => (
        <ClickableCard label="Yearly" isSelected={isSelected} onClick={() => select(!isSelected)} padding="sm">
          <text className="text-body font-medium text-text">Yearly</text>
        </ClickableCard>
      )}
    </Held>
  );
}

export function StateDisabled() {
  return (
    <ClickableCard label="Lifetime" isDisabled onClick={unused} padding="sm">
      <text className="text-body font-medium text-text">Lifetime</text>
    </ClickableCard>
  );
}

/** Floating: over a map or a picture, where an outline alone would be lost. */
export function Floating() {
  return (
    <view className="h-52 justify-end rounded-container bg-accent-soft p-3">
      <Card variant="floating" padding="sm" data-testid="floating">
        <text className="text-body font-medium text-text">Pier 4, east gate</text>
        <text className="text-footnote text-muted">6 minutes on foot · Opens at 08:00</text>
      </Card>
    </view>
  );
}

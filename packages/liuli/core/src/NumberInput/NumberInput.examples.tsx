import { useLocal } from '@lingxia/view';
import { NumberInput } from './NumberInput';

/** A count people nudge more often than they type. */
export function Guests() {
  const guests = useLocal<number | null>('guests', 2);
  return (
    <NumberInput
      label="Guests"
      description="Up to 12 at one table."
      bind={guests}
      min={1}
      max={12}
      decrementLabel="Fewer guests"
      incrementLabel="More guests"
      data-testid="guests"
    />
  );
}

/** A unit after the number, and steps of a tenth. */
export function Weight() {
  const weight = useLocal<number | null>('weight', 71.5);
  return (
    <NumberInput label="Weight" bind={weight} min={0} step={0.1} suffix="kg" decrementLabel="Lighter" incrementLabel="Heavier" data-testid="weight" />
  );
}

/** A number people type: a price, with its currency, and no steppers. */
export function Price() {
  const price = useLocal<number | null>('price', null);
  return (
    <NumberInput
      label="Price"
      bind={price}
      min={0}
      step={0.01}
      prefix="$"
      placeholder="0.00"
      hasSteppers={false}
      optionalLabel="optional"
      decrementLabel="Lower"
      incrementLabel="Higher"
      data-testid="price"
    />
  );
}

export function Sizes() {
  const value = useLocal<number | null>('sized', 4);
  return (
    <view className="gap-3">
      <NumberInput label="Small" size="sm" bind={value} decrementLabel="Less" incrementLabel="More" data-testid="small" />
      <NumberInput label="Medium" bind={value} decrementLabel="Less" incrementLabel="More" />
      <NumberInput label="Large" size="lg" bind={value} decrementLabel="Less" incrementLabel="More" data-testid="large" />
    </view>
  );
}

const none = () => {};

export function StateEmpty() {
  return <NumberInput label="Quantity" value={null} onChange={none} min={0} placeholder="0" decrementLabel="Less" incrementLabel="More" />;
}

export function StateAtMinimum() {
  return <NumberInput label="Quantity" value={1} onChange={none} min={1} max={9} decrementLabel="Less" incrementLabel="More" data-testid="at-minimum" />;
}

export function StateInvalid() {
  return (
    <NumberInput
      label="Quantity"
      value={14}
      onChange={none}
      status={{ type: 'error', message: 'Only 9 left in stock.' }}
      decrementLabel="Less"
      incrementLabel="More"
      data-testid="invalid"
    />
  );
}

export function StateDisabled() {
  return <NumberInput label="Quantity" value={3} onChange={none} isDisabled decrementLabel="Less" incrementLabel="More" data-testid="disabled" />;
}

/** The home tile. */
export function Specimen() {
  return <NumberInput label="Guests" isLabelHidden value={2} onChange={none} size="sm" decrementLabel="Fewer" incrementLabel="More" className="w-32" />;
}

import { useLocal } from '@lingxia/view';
import { Field } from './Field';

const periods = [
  { value: 'monthly', label: 'Monthly' },
  { value: 'yearly', label: 'Yearly' },
] as const;

/** A raw select takes Field's label, help and status until Liuli has its own. */
export function WrappedSelect() {
  const period = useLocal('period', '');
  return (
    <Field
      label="Billing period"
      description="You can change it later."
      status={period.value ? undefined : { type: 'warning', message: 'Choose a period to see the price.' }}
      isRequired
      data-testid="field"
    >
      <select
        bind={period}
        aria-label="Billing period"
        placeholder="Choose…"
        options={periods}
        data-testid="period"
        className="min-h-control-md px-3 rounded-control border border-line-strong bg-surface text-sm text-text"
      />
    </Field>
  );
}

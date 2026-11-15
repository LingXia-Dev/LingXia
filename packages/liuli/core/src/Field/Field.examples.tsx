import { useLocal } from '@lingxia/view';
import { TextInput } from '../TextInput/TextInput';
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
      {control => (
        <select
          bind={period}
          aria-label={control['aria-label']}
          aria-describedby={control['aria-describedby']}
          aria-invalid={control['aria-invalid']}
          aria-required={control['aria-required']}
          disabled={control.disabled}
          placeholder="Choose…"
          options={periods}
          data-testid="period"
          className="min-h-control-md px-3 rounded-control border border-line-strong bg-surface text-body text-text"
        />
      )}
    </Field>
  );
}

/** The home tile. */
export function Specimen() {
  return (
    <Field label="Plan" size="sm" className="w-36">
      {control => (
        <select
          value="yearly"
          onChange={() => {}}
          aria-label={control['aria-label']}
          disabled={control.disabled}
          options={periods}
          className="min-h-control-sm px-3 rounded-control border border-line-strong bg-surface text-footnote text-text"
        />
      )}
    </Field>
  );
}

function PeriodField({ status, isDisabled = false }: { status?: { type: 'error' | 'warning' | 'success'; message: string }; isDisabled?: boolean }) {
  return (
    <Field label="Billing" description="Change it any time." status={status} isDisabled={isDisabled}>
      {control => (
        <select
          value="yearly"
          onChange={() => {}}
          aria-label={control['aria-label']}
          aria-describedby={control['aria-describedby']}
          aria-invalid={control['aria-invalid']}
          disabled={control.disabled}
          options={periods}
          className="min-h-control-md px-3 rounded-control border border-line-strong bg-surface text-body text-text"
        />
      )}
    </Field>
  );
}

export function StateHelp() {
  return <PeriodField />;
}

export function StateWarning() {
  return <PeriodField status={{ type: 'warning', message: 'Prices rise next month.' }} />;
}

export function StateError() {
  return <PeriodField status={{ type: 'error', message: 'Choose a period.' }} />;
}

export function StateDisabled() {
  return <PeriodField isDisabled />;
}

/** A note behind an info button, for what only some people need to know. */
export function Info() {
  const number = useLocal('vat', '');
  return (
    <TextInput
      bind={number}
      label="VAT number"
      optionalLabel="optional"
      info={{ label: 'About the VAT number', content: 'Businesses in the EU can add it to have VAT left off their invoices.' }}
      placeholder="PT123456789"
      data-testid="vat"
    />
  );
}

import { useLocal } from '@lingxia/view';
import { CheckboxInput } from './CheckboxInput';

export function Basic() {
  const digest = useLocal('digest', true);
  return <CheckboxInput bind={digest} label="Weekly digest" description="A summary of your notes every Monday." data-testid="digest" />;
}

/** Owned by the app: the checkbox reports, the app decides. */
export function Controlled() {
  const accepted = useLocal('accepted', false);
  return (
    <CheckboxInput
      value={accepted.value}
      onChange={next => accepted.set(next)}
      label="I accept the terms of service"
      isRequired
      status={accepted.value ? undefined : { type: 'error', message: 'Accept the terms to create the account.' }}
      data-testid="terms"
    />
  );
}

export function Sizes() {
  const small = useLocal('small', false);
  const medium = useLocal('medium', true);
  const large = useLocal('large', false);
  return (
    <view className="gap-2">
      <CheckboxInput bind={small} label="Small" size="sm" />
      <CheckboxInput bind={medium} label="Medium" />
      <CheckboxInput bind={large} label="Large" size="lg" />
    </view>
  );
}

/** The home tile. */
export function Specimen() {
  return <CheckboxInput value onChange={() => {}} label="Remember me" size="sm" />;
}

export function StateOff() {
  return <CheckboxInput value={false} onChange={() => {}} label="Updates" />;
}

export function StateOn() {
  return <CheckboxInput value onChange={() => {}} label="Updates" />;
}

export function StateInvalid() {
  return <CheckboxInput value={false} onChange={() => {}} label="Terms" status={{ type: 'error', message: 'Required.' }} />;
}

export function StateDisabled() {
  return <CheckboxInput value onChange={() => {}} label="Backups" isDisabled />;
}

/** Partly chosen: one box that stands for a set of which some are on. */
export function StateMixed() {
  return <CheckboxInput value={false} onChange={() => {}} isIndeterminate label="All folders" description="3 of 8 chosen" data-testid="mixed" />;
}

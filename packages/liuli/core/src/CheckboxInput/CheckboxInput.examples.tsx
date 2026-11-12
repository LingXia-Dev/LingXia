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

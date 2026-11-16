import { useLocal } from '@lingxia/view';
import { Held, unused } from '../example';
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
  return <CheckboxInput value onChange={unused} label="Remember me" size="sm" />;
}

export function StateOff() {
  return <Held value={false}>{(on, set) => <CheckboxInput value={on} onChange={set} label="Updates" />}</Held>;
}

export function StateOn() {
  return <Held value={true}>{(on, set) => <CheckboxInput value={on} onChange={set} label="Updates" />}</Held>;
}

export function StateInvalid() {
  return <Held value={false}>{(on, set) => <CheckboxInput value={on} onChange={set} label="Terms" status={on ? undefined : { type: 'error', message: 'Required.' }} />}</Held>;
}

export function StateDisabled() {
  return <CheckboxInput value onChange={unused} label="Backups" isDisabled />;
}

/** Partly chosen: one box that stands for a set of which some are on. */
export function StateMixed() {
  return (
    // A press on a mixed box chooses the whole set; after that it is an ordinary checkbox.
    <Held value="mixed">
      {(state, set) => (
        <CheckboxInput
          value={state === 'on'}
          onChange={next => set(next ? 'on' : 'off')}
          isIndeterminate={state === 'mixed'}
          label="All folders"
          description={state === 'mixed' ? '3 of 8 chosen' : state === 'on' ? '8 of 8 chosen' : '0 of 8 chosen'}
          data-testid="mixed"
        />
      )}
    </Held>
  );
}

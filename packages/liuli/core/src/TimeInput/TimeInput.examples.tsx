import { useLocal } from '@lingxia/view';
import { Held, unused } from '../example';
import { TimeInput } from './TimeInput';

/** A daily reminder, bound to a View-local cell. */
export function Reminder() {
  const at = useLocal('at', '09:00');
  return (
    <view className="gap-2">
      <TimeInput bind={at} label="Remind me at" description="Every weekday." data-testid="remind" />
      <text className="text-footnote text-muted" data-testid="remind-value">{`Set for ${at.value}`}</text>
    </view>
  );
}

/** Quiet hours: every 15 minutes, within the evening. */
export function QuietHours() {
  const from = useLocal('from', '22:00');
  return <TimeInput bind={from} label="Quiet from" step={900} min="18:00" max="23:45" data-testid="quiet" />;
}

export function Sizes() {
  return (
    <view className="gap-3">
      <Held value="09:30">{(time, set) => <TimeInput label="Small" size="sm" value={time} onChange={set} />}</Held>
      <Held value="09:30">{(time, set) => <TimeInput label="Medium" value={time} onChange={set} />}</Held>
      <Held value="09:30">{(time, set) => <TimeInput label="Large" size="lg" value={time} onChange={set} />}</Held>
    </view>
  );
}

export function StateEmpty() {
  return <Held value="">{(time, set) => <TimeInput label="Start" placeholder="Add a time" value={time} onChange={set} />}</Held>;
}

export function StateFilled() {
  return <Held value="14:45">{(time, set) => <TimeInput label="Start" value={time} onChange={set} />}</Held>;
}

export function StateInvalid() {
  return <Held value="03:00">{(time, set) => <TimeInput label="Start" value={time} onChange={set} status={time < '08:00' ? { type: 'error', message: 'Choose a time after 8:00.' } : undefined} />}</Held>;
}

export function StateDisabled() {
  return <TimeInput label="Start" value="14:45" onChange={unused} isDisabled />;
}

/** A time that can be taken back: the clear button empties the field. */
export function Clearable() {
  const at = useLocal('at', '09:30');
  return <TimeInput bind={at} label="Remind me at" placeholder="No reminder" hasClear data-testid="clearable" />;
}

/** The home tile. */
export function Specimen() {
  return <TimeInput label="Start" isLabelHidden value="09:30" onChange={unused} size="sm" className="w-28" />;
}

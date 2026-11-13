import { useLocal } from '@lingxia/view';
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
      <TimeInput label="Small" size="sm" value="09:30" onChange={() => {}} />
      <TimeInput label="Medium" value="09:30" onChange={() => {}} />
      <TimeInput label="Large" size="lg" value="09:30" onChange={() => {}} />
    </view>
  );
}

export function StateEmpty() {
  return <TimeInput label="Start" placeholder="Add a time" value="" onChange={() => {}} />;
}

export function StateFilled() {
  return <TimeInput label="Start" value="14:45" onChange={() => {}} />;
}

export function StateInvalid() {
  return <TimeInput label="Start" value="03:00" onChange={() => {}} status={{ type: 'error', message: 'Choose a time after 8:00.' }} />;
}

export function StateDisabled() {
  return <TimeInput label="Start" value="14:45" onChange={() => {}} isDisabled />;
}

/** The home tile. */
export function Specimen() {
  return <TimeInput label="Start" isLabelHidden value="09:30" onChange={() => {}} size="sm" className="w-28" />;
}

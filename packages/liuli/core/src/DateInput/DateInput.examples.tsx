import { useLocal } from '@lingxia/view';
import { DateInput } from './DateInput';

/** A due date, bound to a View-local cell. */
export function DueDate() {
  const due = useLocal('due', '');
  return (
    <view className="gap-2">
      <DateInput bind={due} label="Due date" placeholder="Choose a date" data-testid="due" />
      <text className="text-footnote text-muted" data-testid="due-value">{due.value === '' ? 'No due date' : `Due ${due.value}`}</text>
    </view>
  );
}

/** A trip: the return cannot come before the departure. */
export function Trip() {
  const leave = useLocal('leave', '2026-12-18');
  const back = useLocal('back', '2026-12-27');
  const isBefore = back.value !== '' && back.value < leave.value;
  return (
    <view className="gap-4">
      <DateInput bind={leave} label="Leave" min="2026-11-12" data-testid="leave" />
      <DateInput
        bind={back}
        label="Return"
        min={leave.value}
        status={isBefore ? { type: 'error', message: 'Return after you leave.' } : undefined}
        data-testid="back"
      />
    </view>
  );
}

export function Sizes() {
  return (
    <view className="gap-3">
      <DateInput label="Small" size="sm" value="2026-12-18" onChange={() => {}} />
      <DateInput label="Medium" value="2026-12-18" onChange={() => {}} />
      <DateInput label="Large" size="lg" value="2026-12-18" onChange={() => {}} />
    </view>
  );
}

export function StateEmpty() {
  return <DateInput label="Birthday" placeholder="Add a date" value="" onChange={() => {}} />;
}

export function StateFilled() {
  return <DateInput label="Birthday" value="1990-04-21" onChange={() => {}} />;
}

export function StateInvalid() {
  return <DateInput label="Start" value="2026-01-04" onChange={() => {}} status={{ type: 'error', message: 'Choose a day this year.' }} />;
}

export function StateDisabled() {
  return <DateInput label="Start" value="2026-01-04" onChange={() => {}} isDisabled />;
}

/** The home tile. */
export function Specimen() {
  return <DateInput label="Due date" isLabelHidden value="2026-12-18" onChange={() => {}} size="sm" className="w-32" />;
}

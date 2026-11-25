import { useLocal } from '@lingxia/view';
import { Held, unused } from '../example';
import { TimeInput } from '../TimeInput/TimeInput';
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

/** A day and a time sharing a narrow row: each field keeps room for its whole value. */
export function Meeting() {
  const day = useLocal('day', '2026-12-18');
  const at = useLocal('at', '14:30');
  return (
    <view className="flex-row gap-3">
      <DateInput bind={day} label="Day" className="grow basis-0" data-testid="meeting-day" />
      <TimeInput bind={at} label="Starts" className="grow basis-0" data-testid="meeting-time" />
    </view>
  );
}

export function Sizes() {
  return (
    <view className="gap-3">
      <Held value="2026-12-18">{(date, set) => <DateInput label="Small" size="sm" value={date} onChange={set} />}</Held>
      <Held value="2026-12-18">{(date, set) => <DateInput label="Medium" value={date} onChange={set} />}</Held>
      <Held value="2026-12-18">{(date, set) => <DateInput label="Large" size="lg" value={date} onChange={set} />}</Held>
    </view>
  );
}

export function StateEmpty() {
  return <Held value="">{(date, set) => <DateInput label="Birthday" placeholder="Add a date" value={date} onChange={set} />}</Held>;
}

export function StateFilled() {
  return <Held value="1990-04-21">{(date, set) => <DateInput label="Birthday" value={date} onChange={set} />}</Held>;
}

export function StateInvalid() {
  return <Held value="2026-01-04">{(date, set) => <DateInput label="Start" value={date} onChange={set} status={{ type: 'error', message: 'Choose a day this year.' }} />}</Held>;
}

export function StateDisabled() {
  return <DateInput label="Start" value="2026-01-04" onChange={unused} isDisabled />;
}

/** A day that can be taken back: the clear button empties the field. */
export function Clearable() {
  const due = useLocal('due', '2026-12-18');
  return <DateInput bind={due} label="Due date" placeholder="No due date" hasClear data-testid="clearable" />;
}

/** The home tile. */
export function Specimen() {
  return <DateInput label="Due date" isLabelHidden value="2026-12-18" onChange={unused} size="sm" className="w-32" />;
}

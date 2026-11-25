import { useLocal } from '@lingxia/view';
import { unused } from '../example';
import { DateTimeInput } from './DateTimeInput';

/** When a reminder goes off: one value, two parts. */
export function Reminder() {
  const when = useLocal('when', '2026-11-20T09:00');
  return (
    <view className="gap-2">
      <DateTimeInput bind={when} label="Remind me" dateLabel="Reminder date" timeLabel="Reminder time" description="In your own time zone." data-testid="remind" />
      {when.value !== '' && <text role="status" className="text-footnote text-muted">{`Set for ${when.value.replace('T', ' at ')}`}</text>}
    </view>
  );
}

/** Empty and required, after a send with no time. */
export function Required() {
  const starts = useLocal('starts', '');
  return (
    <DateTimeInput
      bind={starts}
      label="Starts"
      dateLabel="Start date"
      timeLabel="Start time"
      isRequired
      minDate="2026-11-15"
      status={starts.value === '' ? { type: 'error', message: 'Choose a day and a time.' } : undefined}
      data-testid="starts"
    />
  );
}

export function StateDisabled() {
  return <DateTimeInput value="2026-11-20T09:00" onChange={unused} label="Published" dateLabel="Publish date" timeLabel="Publish time" isDisabled disabledMessage="Set when it was published." />;
}

/** In a narrow place, such as a side panel, the time goes under the date and both fill the width. */
export function Narrow() {
  const when = useLocal('due', '2026-12-01T17:30');
  return (
    <view className="w-56">
      <DateTimeInput bind={when} label="Due" dateLabel="Due date" timeLabel="Due time" data-testid="due" />
    </view>
  );
}

/** A moment that can be taken back: the clear button empties both parts. */
export function Clearable() {
  const when = useLocal('when', '2026-11-20T09:00');
  return <DateTimeInput bind={when} label="Remind me" dateLabel="Reminder date" timeLabel="Reminder time" hasClear data-testid="clearable" />;
}

/** The home tile. */
export function Specimen() {
  return <DateTimeInput value="2026-11-20T09:00" onChange={unused} label="When" isLabelHidden size="sm" className="w-full max-w-36" data-testid="tile-when" />;
}

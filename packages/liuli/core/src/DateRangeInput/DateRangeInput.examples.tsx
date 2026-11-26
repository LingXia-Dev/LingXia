import { useLocal } from '@lingxia/view';
import { unused } from '../example';
import type { CalendarRange } from '../Calendar/Calendar';
import { DateRangeInput, type DateRangeInputProps } from './DateRangeInput';

const today = '2026-11-15';
const months = ['Jan', 'Feb', 'Mar', 'Apr', 'May', 'Jun', 'Jul', 'Aug', 'Sep', 'Oct', 'Nov', 'Dec'];
const short = (date: string) => `${Number(date.slice(8))} ${months[Number(date.slice(5, 7)) - 1]}`;

/** A span of days in a form: pressing the field opens a calendar, and the second day closes it. */
export function Stay() {
  const stay = useLocal<CalendarRange>('stay', { start: '2026-11-18', end: '2026-11-24' });
  return (
    <DateRangeInput
      label="Stay"
      description="Check-in from 3 pm, check-out by 11 am."
      bind={stay}
      format={short}
      today={today}
      min={today}
      data-testid="stay"
    />
  );
}

/** Nothing chosen yet, and required. */
export function Empty() {
  const leave = useLocal<CalendarRange>('leave', { start: null, end: null });
  return (
    <DateRangeInput
      label="Time off"
      placeholder="First and last day"
      isRequired
      bind={leave}
      format={short}
      today={today}
      data-testid="leave"
    />
  );
}

const set = { start: '2026-12-01', end: '2026-12-05' };

export function StateInvalid() {
  const stay = useLocal<CalendarRange>('taken', set);
  const isTaken = stay.value.start === set.start && stay.value.end === set.end;
  return <DateRangeInput label="Stay" bind={stay} format={short} today={today} status={isTaken ? { type: 'error', message: 'These nights are taken.' } : undefined} data-testid="invalid" />;
}

export function StateDisabled() {
  return <DateRangeInput label="Stay" value={set} onChange={unused} format={short} today={today} isDisabled data-testid="disabled" />;
}

/** A span that can be taken back: the clear button empties it. */
export function Clearable() {
  const stay = useLocal<CalendarRange>('stay', { start: '2026-11-18', end: '2026-11-24' });
  return <DateRangeInput label="Stay" bind={stay} format={short} today={today} placeholder="Any dates" hasClear data-testid="clearable" />;
}

/** Named spans over the calendar: one pressed fills the field and closes the calendar. */
export function Presets() {
  const period = useLocal<CalendarRange>('period', { start: '2026-11-09', end: '2026-11-15' });
  return (
    <DateRangeInput
      label="Report period"
      bind={period}
      format={short}
      today={today}
      max={today}
      presets={[
        { label: 'Last 7 days', range: { start: '2026-11-09', end: '2026-11-15' } },
        { label: 'Last 30 days', range: { start: '2026-10-17', end: '2026-11-15' } },
        { label: 'This month', range: { start: '2026-11-01', end: '2026-11-15' } },
        { label: 'Last month', range: { start: '2026-10-01', end: '2026-10-31' } },
      ]}
      data-testid="period"
    />
  );
}

/** The home tile. */
export function Specimen() {
  return <DateRangeInput label="Stay" isLabelHidden size="sm" value={set} onChange={unused} format={short} today={today} className="w-36" />;
}

/** The playground's props that are not plain values. */
export function playground() {
  return { value: { start: '2026-11-18', end: '2026-11-24' }, format: short } satisfies Partial<DateRangeInputProps>;
}

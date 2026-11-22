import { cx, useBox, useLocal, useView, type Local } from '@lingxia/view';
import { liuliText } from '../i18n/text';
import { useControlSize } from '../density';
import { DateInput } from '../DateInput/DateInput';
import { Field, type FieldOwnProps } from '../Field/Field';
import { TimeInput } from '../TimeInput/TimeInput';
import { dateTimeInputRecipe as r } from './DateTimeInput.recipe';

export type DateTimeInputBinding =
  | {
      /** A View-local cell holding `YYYY-MM-DDTHH:mm`, or `''` until both parts are set. */
      bind: Local<string>;
      value?: never;
      onChange?: never;
    }
  | {
      bind?: never;
      /** `YYYY-MM-DDTHH:mm` in the user's own time, or `''`. */
      value: string;
      /** Runs with the whole value once both parts are set, and with `''` when either is cleared. */
      onChange: (value: string) => void;
    };

export type DateTimeInputProps = FieldOwnProps & DateTimeInputBinding & {
  /** The date part's accessible name: "Start date". Defaults to "Date" in the page's language. */
  dateLabel?: string;
  /** The time part's accessible name: "Start time". Defaults to "Time" in the page's language. */
  timeLabel?: string;
  /** The earliest day, as `YYYY-MM-DD`. */
  minDate?: string;
  /** The latest day, as `YYYY-MM-DD`. */
  maxDate?: string;
  /** The time's step in seconds. @default 60 */
  timeStep?: number;
  className?: string;
  /** On the group; the parts get `<testid>-date` and `<testid>-time`. */
  'data-testid'?: string;
};

// A long date and time as the editors set them; a part needs its value's width plus this, for its padding, border and mark.
const longDate = 'Sep 30, 2026';
const longTime = { h12: '12:00 PM', h23: '23:59' } as const;
const [partChrome, partGap] = [50, 8];

/** A day and a time of day as one value, under one label. */
export function DateTimeInput(props: DateTimeInputProps) {
  const {
    label, isLabelHidden, description, status, isRequired, hasRequiredMark, optionalLabel, info, isDisabled = false, disabledMessage, size: ownSize,
    dateLabel = liuliText('@liuli.date'), timeLabel = liuliText('@liuli.time'), minDate, maxDate, timeStep, className, 'data-testid': testId,
  } = props;
  const size = useControlSize(ownSize);
  const { env } = useView();
  const [groupBox, dateBox, timeBox] = [useBox('group'), useBox('date'), useBox('time')];
  const isStacked = groupBox.value !== null && dateBox.value !== null && timeBox.value !== null
    && dateBox.value.width + timeBox.value.width + partChrome * 2 + partGap > groupBox.value.width;
  const whole = props.bind ? props.bind.value : props.value;
  const [wholeDate = '', wholeTime = ''] = whole.split('T');
  // One part can be set before the other; the whole value exists only once both are.
  const date = useLocal('date', wholeDate);
  const time = useLocal('time', wholeTime);
  const shownDate = whole === '' ? date.value : wholeDate;
  const shownTime = whole === '' ? time.value : wholeTime;
  const emit = (nextDate: string, nextTime: string) => {
    date.set(nextDate);
    time.set(nextTime);
    const next = nextDate !== '' && nextTime !== '' ? `${nextDate}T${nextTime}` : '';
    if (props.bind) props.bind.set(next);
    else props.onChange(next);
  };
  // The parts take the status colour; the message is said once, under the pair.
  const partStatus = status === undefined ? undefined : { type: status.type };
  return (
    <Field
      label={label}
      isLabelHidden={isLabelHidden}
      description={description}
      status={status}
      isRequired={isRequired}
      hasRequiredMark={hasRequiredMark}
      optionalLabel={optionalLabel}
      info={info}
      isDisabled={isDisabled}
      disabledMessage={disabledMessage}
      size={size}
      className={className}
    >
      {control => (
        <view box={groupBox} role="group" aria-label={control['aria-label']} aria-describedby={control['aria-describedby']} data-testid={testId} className={r.group.base}>
          <DateInput
            label={dateLabel}
            isLabelHidden
            value={shownDate}
            onChange={next => emit(next, shownTime)}
            min={minDate}
            max={maxDate}
            status={partStatus}
            isRequired={isRequired}
            isDisabled={isDisabled}
            size={size}
            className={cx(r.date.base, isStacked && r.stacked)}
            data-testid={testId && `${testId}-date`}
          />
          <TimeInput
            label={timeLabel}
            isLabelHidden
            value={shownTime}
            onChange={next => emit(shownDate, next)}
            step={timeStep}
            status={partStatus}
            isRequired={isRequired}
            isDisabled={isDisabled}
            size={size}
            className={cx(r.time.base, isStacked && r.stacked)}
            data-testid={testId && `${testId}-time`}
          />
          <view aria-hidden className={r.measure.base}>
            <text box={dateBox} className={cx(r.measure.sample, r.measure.size[size])}>{longDate}</text>
            <text box={timeBox} className={cx(r.measure.sample, r.measure.size[size])}>{longTime[env.hourCycle]}</text>
          </view>
        </view>
      )}
    </Field>
  );
}

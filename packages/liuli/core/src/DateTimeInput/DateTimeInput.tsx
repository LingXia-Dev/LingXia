import { cx, useBox, useLocal, useView, type Local } from '@lingxia/view';
import { liuliText } from '../i18n/text';
import { useControlSize } from '../density';
import { DateInput } from '../DateInput/DateInput';
import { Field, type FieldOwnProps } from '../Field/Field';
import { timeInputRecipe } from '../TimeInput/TimeInput.recipe';
import { dateTimeInputRecipe as r } from './DateTimeInput.recipe';
import { dataOf } from '../internal/data';
import { glyphGrowth, useTextGrowth } from '../internal/Glyph';
import { PickerField, pickerMark } from '../internal/PickerField';

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
  /** A button at the end of the time part that empties both parts, shown while either holds a value and the field is enabled. @default false */
  hasClear?: boolean;
  /** Names the clear button. Defaults to "Clear" and the field's label, in the page's language. */
  clearLabel?: string;
  className?: string;
  /** On the group; the parts get `<testid>-date` and `<testid>-time`. */
  'data-testid'?: string;
};

// A long date and time as the editors set them; a part needs its value's width plus its padding, border, gap and mark.
const longDate = 'Sep 30, 2026';
const longTime = { h12: '12:00 PM', h23: '23:59' } as const;
const [partFrame, partGap] = [34, 8];
// The clear button inside the time part, less the end padding it overlaps, and its gap.
const clearRoom = 32 - 6 + 8;

/** A day and a time of day as one value, under one label. */
export function DateTimeInput(props: DateTimeInputProps) {
  const {
    label, isLabelHidden, description, status, isRequired, hasRequiredMark, optionalLabel, info, isDisabled = false, disabledMessage, size: ownSize,
    dateLabel = liuliText('@liuli.date'), timeLabel = liuliText('@liuli.time'), minDate, maxDate, timeStep, hasClear = false, clearLabel, className, 'data-testid': testId,
  } = props;
  const size = useControlSize(ownSize);
  const { env } = useView();
  const [groupBox, dateBox, timeBox] = [useBox('group'), useBox('date'), useBox('time')];
  // Each part's mark grows with the text, so the room it takes does too.
  const partChrome = partFrame + (useTextGrowth(pickerMark, glyphGrowth)?.width ?? pickerMark);
  const whole = props.bind ? props.bind.value : props.value;
  const [wholeDate = '', wholeTime = ''] = whole.split('T');
  // One part can be set before the other; the whole value exists only once both are. The draft remembers what it
  // sent, so a reset from outside (a full value to '') empties both parts rather than showing the old ones.
  const draft = useLocal('draft', { date: wholeDate, time: wholeTime, sent: whole });
  const isDraft = whole === '' && draft.value.sent === '';
  const shownDate = isDraft ? draft.value.date : wholeDate;
  const shownTime = isDraft ? draft.value.time : wholeTime;
  const showClear = hasClear && (shownDate !== '' || shownTime !== '') && !isDisabled;
  const isStacked = groupBox.value !== null && dateBox.value !== null && timeBox.value !== null
    && dateBox.value.width + timeBox.value.width + partChrome * 2 + partGap + (showClear ? clearRoom : 0) > groupBox.value.width;
  const emit = (nextDate: string, nextTime: string) => {
    const next = nextDate !== '' && nextTime !== '' ? `${nextDate}T${nextTime}` : '';
    draft.set({ date: nextDate, time: nextTime, sent: next });
    if (props.bind) props.bind.set(next);
    else props.onChange(next);
  };
  // The parts take the status colour; the message is said once, under the pair.
  const partStatus = status === undefined ? undefined : { type: status.type };
  // The time part is TimeInput's field, holding the clear button that empties both parts.
  const timePart = (className: string) => (
    <PickerField
      type="time"
      icon="clock"
      recipe={timeInputRecipe}
      label={timeLabel}
      isLabelHidden
      value={shownTime}
      onChange={next => emit(shownDate, next)}
      step={timeStep}
      status={partStatus}
      isRequired={isRequired}
      isDisabled={isDisabled}
      size={size}
      clearLabel={clearLabel}
      clearsGroup={hasClear ? { isShown: showClear, label, onClear: () => emit('', ''), testId } : undefined}
      className={className}
      data-testid={testId && `${testId}-time`}
    />
  );
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
        <view box={groupBox} role="group" aria-label={control['aria-label']} aria-describedby={control['aria-describedby']} data-testid={testId} {...dataOf(props)} className={r.group.base}>
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
            className={cx(hasClear ? r.clearable.date : r.date.base, isStacked && r.stacked)}
            data-testid={testId && `${testId}-date`}
          />
          {timePart(cx(hasClear ? r.clearable.time : r.time.base, isStacked && r.stacked))}
          <view aria-hidden className={r.measure.base}>
            <text box={dateBox} className={cx(r.measure.sample, r.measure.size[size])}>{longDate}</text>
            <text box={timeBox} className={cx(r.measure.sample, r.measure.size[size])}>{longTime[env.hourCycle]}</text>
          </view>
        </view>
      )}
    </Field>
  );
}

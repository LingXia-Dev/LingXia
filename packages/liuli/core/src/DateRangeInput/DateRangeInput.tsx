import { cx, useId, useLocal, useView, type Local } from '@lingxia/view';
import { useControlSize } from '../density';
import { Calendar, type CalendarDate, type CalendarRange, type CalendarWeekday } from '../Calendar/Calendar';
import { comboboxRecipe as c } from '../Combobox/Combobox.recipe';
import { Field, type FieldOwnProps } from '../Field/Field';
import { liuliText } from '../i18n/text';
import { Glyph } from '../internal/Glyph';
import { LayerContext } from '../internal/layer';
import { dateRangeInputRecipe as r } from './DateRangeInput.recipe';
import { dataOf } from '../internal/data';

export type DateRangeInputBinding =
  | {
      /** A View-local cell the control reads and writes; use it or `value` with `onChange`. */
      bind: Local<CalendarRange>;
      value?: never;
      onChange?: never;
    }
  | {
      bind?: never;
      /** The chosen span; both ends `null` shows the placeholder. */
      value: CalendarRange;
      /** Runs with each press in the calendar: the first day alone, then the finished span. */
      onChange: (range: CalendarRange) => void;
    };

export type DateRangeInputProps = FieldOwnProps & DateRangeInputBinding & {
  /** Turns a day into the text the field shows: "18 Nov". The app formats dates. */
  format: (date: CalendarDate) => string;
  /** Today, from the page's Logic. */
  today: CalendarDate;
  /** Shown while no day is chosen. */
  placeholder?: string;
  min?: CalendarDate;
  max?: CalendarDate;
  isDateDisabled?: (date: CalendarDate) => boolean;
  /** The calendar's first day of the week, as Calendar's `firstWeekday`: 1 is Monday to 7 is Sunday. @default the user's first weekday */
  firstWeekday?: CalendarWeekday;
  /** Shows a close button in the calendar's sheet; without it the sheet closes by a tap outside, a drag or Back. @default true */
  hasCloseButton?: boolean;
  /** Names the close button. Defaults to "Close" in the page's language. */
  closeLabel?: string;
  className?: string;
  /** On the field's button; the calendar is `<id>-calendar`. */
  'data-testid'?: string;
};

/** A span of days in a form: a field that opens a calendar and closes once both ends are chosen. */
export function DateRangeInput(props: DateRangeInputProps) {
  const {
    label, isLabelHidden, description, status, isRequired, hasRequiredMark, optionalLabel, info, isDisabled = false, disabledMessage, size: ownSize,
    format, today, placeholder, min, max, isDateDisabled, firstWeekday, hasCloseButton = true, closeLabel = liuliText('@liuli.close'), className, 'data-testid': testId,
  } = props;
  const range = props.bind ? props.bind.value : props.value;
  const onRangeChange = (next: CalendarRange) => props.bind ? props.bind.set(next) : props.onChange(next);
  const size = useControlSize(ownSize);
  const { env } = useView();
  // A finger gets a sheet; a mouse gets the calendar in a panel under the field.
  const isSheet = env.pointer === 'coarse';
  const open = useLocal('open', false);
  const id = useId();
  const [panelId, valueId] = [`${id}-panel`, `${id}-value`];
  const isEmpty = range.start === null;
  // A span in progress shows its start and an open end.
  const shown = isEmpty ? placeholder ?? '' : `${format(range.start!)} – ${range.end === null ? '' : format(range.end)}`;
  const choose = (next: CalendarRange) => {
    onRangeChange(next);
    if (next.end !== null) open.set(false);
  };
  const calendar = (
    <LayerContext.Provider value="overlay">
      <Calendar
        label={label}
        today={today}
        isRange
        value={range}
        onChange={choose}
        min={min}
        max={max}
        isDateDisabled={isDateDisabled}
        firstWeekday={firstWeekday}
        data-testid={testId && `${testId}-calendar`}
      />
    </LayerContext.Provider>
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
        <>
          <button
            popoverTarget={isSheet ? undefined : panelId}
            onClick={isSheet ? () => open.set(true) : undefined}
            aria-label={control['aria-label']}
            // The chosen span is read after the name, as a field's value is, then the help text.
            aria-describedby={control['aria-describedby'] ? `${valueId} ${control['aria-describedby']}` : valueId}
            aria-expanded={isSheet ? open.value : undefined}
            disabled={control.disabled}
            data-testid={testId} {...dataOf(props)}
            className={cx(c.trigger.base, c.trigger.size[size], c.trigger.pressable, status && c.trigger.status[status.type])}
          >
            <text id={valueId} className={cx(isEmpty ? c.value.placeholder : c.value.base, c.value.size[size], r.value.base, isDisabled && c.value.disabled)}>{shown}</text>
            <Glyph name="calendar" className={r.icon.base} />
          </button>
          {isSheet ? (
            <dialog
              open={open.value}
              onClose={() => open.set(false)}
              closedby="any"
              aria-label={label}
              data-testid={testId && `${testId}-panel`}
              className={c.sheet.base}
            >
              <view aria-hidden className={c.grabber.base} />
              <view className={c.header.base}>
                <text role="heading" headingLevel={2} className={c.title.base}>{label}</text>
                {hasCloseButton && (
                  <button aria-label={closeLabel} onClick={() => open.set(false)} data-testid={testId && `${testId}-close`} className={c.close.base}>
                    <Glyph name="close" className={c.close.icon} />
                  </button>
                )}
              </view>
              {calendar}
            </dialog>
          ) : (
            <view id={panelId} popover aria-label={label} open={open.value} onToggle={open.set} data-testid={testId && `${testId}-panel`} className={r.popover.base}>
              {calendar}
            </view>
          )}
        </>
      )}
    </Field>
  );
}

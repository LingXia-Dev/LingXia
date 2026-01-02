import { hilog } from '@kit.PerformanceAnalysisKit';
import { i18n } from '@kit.LocalizationKit';
import { onCallback } from 'liblingxia.so';

export interface DatePickerConfig {
  mode: string;
  fields: string;
  value?: string | string[];
  start?: string;
  end?: string;
  cancelText: string;
  cancelButtonColor: string;
  cancelTextColor: string;
  confirmText: string;
  confirmButtonColor: string;
  confirmTextColor: string;
}

export interface LxDatePickerResult {
  value?: string | string[];
  cancel?: boolean;
  confirm?: boolean;
}

const DOMAIN = 0x0000;
const TAG = 'LingXia.DatePicker';

export class LxAppDatePicker {
  private static pickerManagerRef: DatePickerManager | null = null;

  public static localCallbacks: Map<string, (success: boolean, result: string) => void> = new Map();

  public static showDatePicker(
    mode: string,
    fields: string,
    value: string,
    start: string,
    end: string,
    cancelText: string,
    cancelButtonColor: string,
    cancelTextColor: string,
    confirmText: string,
    confirmButtonColor: string,
    confirmTextColor: string,
    callbackId: string
  ): void {
    const config: DatePickerConfig = {
      mode,
      fields,
      value: fields === 'range' ? (value ? JSON.parse(value) : undefined) : value,
      start: start || undefined,
      end: end || undefined,
      cancelText,
      cancelButtonColor,
      cancelTextColor,
      confirmText,
      confirmButtonColor,
      confirmTextColor
    };

    if (LxAppDatePicker.pickerManagerRef) {
      LxAppDatePicker.pickerManagerRef.showPicker(config, callbackId);
    } else {
      hilog.warn(DOMAIN, TAG, 'DatePicker manager not initialized');
    }
  }

  public static setManagerRef(manager: DatePickerManager): void {
    LxAppDatePicker.pickerManagerRef = manager;
  }

  public static clearManagerRef(): void {
    LxAppDatePicker.pickerManagerRef = null;
  }
}

class QuickSelectOption {
  key: string;
  title: ResourceStr;

  constructor(key: string, title: ResourceStr) {
    this.key = key;
    this.title = title;
  }
}

class DayCellProps {
  dayText: string = '';
  isSelected: boolean = false;
  isToday: boolean = false;
  isDisabled: boolean = false;
  showRangeBackground: boolean = false;
  radius: BorderRadiuses | number = 0;
  isInCurrentMonth: boolean = false;
  date: Date = new Date();
}

class DayCellState {
  isSelected: boolean = false;
  isStart: boolean = false;
  isEnd: boolean = false;
  isInRange: boolean = false;
  showRangeBackground: boolean = false;
}

@Component
export struct DatePickerManager {
  @State isVisible: boolean = false;
  @State config: DatePickerConfig | null = null;
  @State selectedValue: Date | string | string[] | null = null;
  @State currentMonth: Date = new Date();
  @State selectedDate: Date = new Date();
  @State rangeStart: Date | null = null;
  @State rangeEnd: Date | null = null;
  @State rangeSelectingStart: boolean = true;
  @State rangeComplete: boolean = false;
  @State timeHourRange: string[] = [];
  @State timeMinuteRange: string[] = [];
  @State timeHour: string = '09';
  @State timeMinute: string = '00';
  @State quickSelectRow1: QuickSelectOption[] = [];
  @State quickSelectRow2: QuickSelectOption[] = [];

  private callbackId: string = "";
  private isInitializing: boolean = false;
  private minDate: Date | null = null;
  private maxDate: Date | null = null;
  private minTimeMinutes: number = 0;
  private maxTimeMinutes: number = 23 * 60 + 59;

  aboutToAppear() {
    LxAppDatePicker.setManagerRef(this);
    this.updateQuickSelectRows();
  }

  private updateQuickSelectRows() {
    this.quickSelectRow1 = [
      new QuickSelectOption('last7Days', $r('app.string.lx_date_last_7_days')),
      new QuickSelectOption('last30Days', $r('app.string.lx_date_last_30_days')),
      new QuickSelectOption('thisWeek', $r('app.string.lx_date_this_week'))
    ];
    this.quickSelectRow2 = [
      new QuickSelectOption('lastWeek', $r('app.string.lx_date_last_week')),
      new QuickSelectOption('thisMonth', $r('app.string.lx_date_this_month')),
      new QuickSelectOption('lastMonth', $r('app.string.lx_date_last_month'))
    ];
  }

  aboutToDisappear() {
    LxAppDatePicker.clearManagerRef();
  }

  public showPicker(config: DatePickerConfig, callbackId: string): void {
    this.isInitializing = true;
    this.config = config;
    this.callbackId = callbackId;

    if (config.mode === 'time') {
      this.initTimePicker(config);
    } else if (config.fields === 'range') {
      this.initRangePicker(config);
    } else if (config.fields === 'year') {
      const years = this.getYearRange();
      let year = (config.value as string) || new Date().getFullYear().toString();
      if (years.length > 0 && !years.includes(year)) year = years[0];
      this.selectedValue = year;
    } else if (config.fields === 'month') {
      const years = this.getYearRange();
      const months = this.getMonthRange();
      const initial = (config.value as string) || this.formatYearMonth(new Date());

      const parts = initial.split('-');
      let year = parts[0] || new Date().getFullYear().toString();
      let month = parts[1] || '01';
      if (years.length > 0 && !years.includes(year)) year = years[0];
      if (months.length > 0 && !months.includes(month)) month = months[0];

      this.selectedValue = `${year}-${month}`;
    } else {
      this.initDayPicker(config);
    }

    this.isVisible = true;

    setTimeout(() => {
      this.isInitializing = false;
    }, 20);
  }

  private hidePicker(): void {
    this.isVisible = false;
    this.config = null;
    this.callbackId = "";
    this.selectedValue = null;
    this.minDate = null;
    this.maxDate = null;
    this.minTimeMinutes = 0;
    this.maxTimeMinutes = 23 * 60 + 59;
    this.rangeStart = null;
    this.rangeEnd = null;
    this.rangeSelectingStart = true;
    this.rangeComplete = false;
  }

  private onCancel(): void {
    const localCallback = LxAppDatePicker.localCallbacks.get(this.callbackId);
    try {
      if (localCallback) {
        const result: LxDatePickerResult = { cancel: true };
        localCallback(false, JSON.stringify(result));
        LxAppDatePicker.localCallbacks.delete(this.callbackId);
      } else {
        onCallback(this.callbackId, false, '2000');
      }
    } catch (err) {
      hilog.error(DOMAIN, TAG, `onCancel callback failed: ${err}`);
    } finally {
      this.hidePicker();
    }
  }

  private onConfirm(): void {
    let value: string | string[];

    if (this.config?.mode === 'time') {
      value = `${this.timeHour}:${this.timeMinute}`;
    } else if (this.config?.fields === 'range') {
      if (this.rangeComplete && this.rangeStart && this.rangeEnd) {
        value = [this.formatDate(this.rangeStart), this.formatDate(this.rangeEnd)];
      } else {
        const today = this.startOfDay(new Date());
        value = [this.formatDate(today), this.formatDate(today)];
      }
    } else if (this.config?.fields === 'year') {
      value = this.selectedValue as string || new Date().getFullYear().toString();
    } else if (this.config?.fields === 'month') {
      value = this.selectedValue as string || this.formatYearMonth(new Date());
    } else {
      const date = this.selectedDate || new Date();
      value = this.formatDate(date);
    }

    const result: LxDatePickerResult = {
      value: value,
      confirm: true
    };

    const resultJSON: string = JSON.stringify(result);
    const localCallback = LxAppDatePicker.localCallbacks.get(this.callbackId);
    try {
      if (localCallback) {
        localCallback(true, resultJSON);
        LxAppDatePicker.localCallbacks.delete(this.callbackId);
      } else {
        onCallback(this.callbackId, true, resultJSON);
      }
    } catch (err) {
      hilog.error(DOMAIN, TAG, `onConfirm callback failed: ${err}`);
    } finally {
      this.hidePicker();
    }
  }

  private sendScrollResult(value: string | string[]): void {
    if (this.isInitializing) return;

    const result: LxDatePickerResult = { value: value };
    const resultJSON = JSON.stringify(result);

    const localCallback = LxAppDatePicker.localCallbacks.get(this.callbackId);
    if (localCallback) {
      localCallback(true, resultJSON);
    } else {
      onCallback(this.callbackId, true, resultJSON);
    }
  }

  private formatDate(date: Date): string {
    const year = date.getFullYear();
    const month = (date.getMonth() + 1).toString().padStart(2, '0');
    const day = date.getDate().toString().padStart(2, '0');
    return `${year}-${month}-${day}`;
  }

  private formatYearMonth(date: Date): string {
    const year = date.getFullYear();
    const month = (date.getMonth() + 1).toString().padStart(2, '0');
    return `${year}-${month}`;
  }

  private parseDate(dateStr: string): Date {
    try {
      const parts = dateStr.split('-');
      if (parts.length === 3) {
        return new Date(parseInt(parts[0]), parseInt(parts[1]) - 1, parseInt(parts[2]));
      }
    } catch (e) {
      hilog.error(DOMAIN, TAG, `Failed to parse date: ${dateStr}`);
    }
    return new Date();
  }

  private startOfDay(date: Date): Date {
    return new Date(date.getFullYear(), date.getMonth(), date.getDate());
  }

  private isSameDay(a: Date, b: Date): boolean {
    return a.getFullYear() === b.getFullYear()
      && a.getMonth() === b.getMonth()
      && a.getDate() === b.getDate();
  }

  private isChineseLocale(): boolean {
    try {
      return i18n.System.getSystemLanguage().toLowerCase().startsWith('zh');
    } catch (e) {
      return false;
    }
  }

  private firstWeekdayIndex(): number {
    return this.isChineseLocale() ? 1 : 0; // 0=Sun, 1=Mon
  }

  private formatMonthLabel(date: Date): string {
    try {
      const language = i18n.System.getSystemLanguage();
      const region = i18n.System.getSystemRegion();
      const locale = `${language}-${region}`;
      return new Intl.DateTimeFormat(locale, { month: 'long', year: 'numeric' }).format(date);
    } catch (e) {
      const y = date.getFullYear();
      const m = (date.getMonth() + 1).toString().padStart(2, '0');
      return `${y}-${m}`;
    }
  }

  private weekdayLabels(): string[] {
    try {
      const language = i18n.System.getSystemLanguage();
      const region = i18n.System.getSystemRegion();
      const locale = `${language}-${region}`;
      const fmt = new Intl.DateTimeFormat(locale, { weekday: 'short' });
      const base = new Date(2024, 0, 7); // Sunday
      const first = this.firstWeekdayIndex();
      const labels: string[] = [];
      for (let i = 0; i < 7; i++) {
        const d = new Date(base);
        d.setDate(base.getDate() + ((first + i) % 7));
        labels.push(fmt.format(d));
      }
      return labels;
    } catch (e) {
      return this.firstWeekdayIndex() === 1
        ? ['Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat', 'Sun']
        : ['Sun', 'Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat'];
    }
  }

  private parseDateBound(value?: string, isEnd: boolean = false): Date | null {
    if (!value) return null;
    const raw = value.trim();
    if (!raw) return null;

    const parts = raw.split('-').filter(p => p.length > 0);
    const year = parseInt(parts[0]);
    if (isNaN(year)) return null;

    if (parts.length === 1) {
      return isEnd ? new Date(year, 11, 31) : new Date(year, 0, 1);
    }

    const month = parseInt(parts[1]);
    if (isNaN(month)) return null;

    if (parts.length === 2) {
      return isEnd ? new Date(year, month, 0) : new Date(year, month - 1, 1);
    }

    const day = parseInt(parts[2]);
    if (isNaN(day)) return null;
    return new Date(year, month - 1, day);
  }

  private addMonths(date: Date, offset: number): Date {
    return new Date(date.getFullYear(), date.getMonth() + offset, 1);
  }

  private addYears(date: Date, offset: number): Date {
    return new Date(date.getFullYear() + offset, date.getMonth(), 1);
  }

  private monthStart(date: Date): Date {
    return new Date(date.getFullYear(), date.getMonth(), 1);
  }

  private monthEnd(date: Date): Date {
    return new Date(date.getFullYear(), date.getMonth() + 1, 0);
  }

  private canGoPrevMonth(): boolean {
    if (!this.minDate) return true;
    const prev = this.addMonths(this.currentMonth, -1);
    return this.monthEnd(prev).getTime() >= this.startOfDay(this.minDate).getTime();
  }

  private canGoNextMonth(): boolean {
    if (!this.maxDate) return true;
    const next = this.addMonths(this.currentMonth, 1);
    return this.monthStart(next).getTime() <= this.startOfDay(this.maxDate).getTime();
  }

  private canGoPrevYear(): boolean {
    if (!this.minDate) return true;
    const prev = this.addYears(this.currentMonth, -1);
    return this.monthEnd(prev).getTime() >= this.startOfDay(this.minDate).getTime();
  }

  private canGoNextYear(): boolean {
    if (!this.maxDate) return true;
    const next = this.addYears(this.currentMonth, 1);
    return this.monthStart(next).getTime() <= this.startOfDay(this.maxDate).getTime();
  }

  private calendarGridDays(): Date[] {
    const firstOfMonth = this.monthStart(this.currentMonth);
    const weekday = firstOfMonth.getDay(); // 0..6 (Sun..Sat)
    const leading = (weekday - this.firstWeekdayIndex() + 7) % 7;
    const start = new Date(firstOfMonth);
    start.setDate(firstOfMonth.getDate() - leading);

    const days: Date[] = [];
    for (let i = 0; i < 42; i++) {
      const d = new Date(start);
      d.setDate(start.getDate() + i);
      days.push(this.startOfDay(d));
    }
    return days;
  }

  private initDayPicker(config: DatePickerConfig): void {
    this.minDate = this.parseDateBound(config.start, false);
    this.maxDate = this.parseDateBound(config.end, true);

    const initial = typeof config.value === 'string' ? config.value : '';
    const selected = initial ? this.parseDate(initial) : new Date();
    let clamped = this.startOfDay(selected);
    if (this.minDate && clamped.getTime() < this.startOfDay(this.minDate).getTime()) clamped = this.startOfDay(this.minDate);
    if (this.maxDate && clamped.getTime() > this.startOfDay(this.maxDate).getTime()) clamped = this.startOfDay(this.maxDate);
    this.selectedDate = clamped;
    this.currentMonth = this.monthStart(this.selectedDate);
    this.selectedValue = this.selectedDate;
  }

  private initRangePicker(config: DatePickerConfig): void {
    this.minDate = this.parseDateBound(config.start, false);
    this.maxDate = this.parseDateBound(config.end, true);

    const initialRange = Array.isArray(config.value) ? config.value : undefined;
    if (initialRange && initialRange.length === 2) {
      let start = this.startOfDay(this.parseDate(initialRange[0]));
      let end = this.startOfDay(this.parseDate(initialRange[1]));

      if (this.minDate) {
        const minT = this.startOfDay(this.minDate).getTime();
        if (start.getTime() < minT) start = this.startOfDay(this.minDate);
        if (end.getTime() < minT) end = this.startOfDay(this.minDate);
      }
      if (this.maxDate) {
        const maxT = this.startOfDay(this.maxDate).getTime();
        if (start.getTime() > maxT) start = this.startOfDay(this.maxDate);
        if (end.getTime() > maxT) end = this.startOfDay(this.maxDate);
      }

      const s = start.getTime() <= end.getTime() ? start : end;
      const e = start.getTime() <= end.getTime() ? end : start;
      this.rangeStart = s;
      this.rangeEnd = e;
      this.rangeComplete = true;
      this.rangeSelectingStart = true;
      this.currentMonth = this.monthStart(s);
      this.selectedValue = [this.formatDate(s), this.formatDate(e)];
    } else {
      this.rangeStart = null;
      this.rangeEnd = null;
      this.rangeComplete = false;
      this.rangeSelectingStart = true;
      this.currentMonth = this.monthStart(new Date());
      this.selectedValue = null;
    }
  }

  private parseTimeToMinutes(value?: string): number | null {
    if (!value) return null;
    const parts = value.split(':').map(p => parseInt(p));
    if (parts.length < 2 || isNaN(parts[0]) || isNaN(parts[1])) return null;
    const hour = Math.max(0, Math.min(parts[0], 23));
    const minute = Math.max(0, Math.min(parts[1], 59));
    return hour * 60 + minute;
  }

  private pad2(value: number): string {
    return value.toString().padStart(2, '0');
  }

  private timeMinutesForHour(hour: number): number[] {
    const minHour = Math.floor(this.minTimeMinutes / 60);
    const maxHour = Math.floor(this.maxTimeMinutes / 60);
    const minMinute = this.minTimeMinutes % 60;
    const maxMinute = this.maxTimeMinutes % 60;

    if (minHour === maxHour) {
      const minutes: number[] = [];
      for (let m = minMinute; m <= maxMinute; m++) minutes.push(m);
      return minutes;
    }
    if (hour === minHour) {
      const minutes: number[] = [];
      for (let m = minMinute; m <= 59; m++) minutes.push(m);
      return minutes;
    }
    if (hour === maxHour) {
      const minutes: number[] = [];
      for (let m = 0; m <= maxMinute; m++) minutes.push(m);
      return minutes;
    }
    const minutes: number[] = [];
    for (let m = 0; m <= 59; m++) minutes.push(m);
    return minutes;
  }

  private updateTimeMinuteRange(): void {
    const hour = parseInt(this.timeHour);
    const minutes = this.timeMinutesForHour(isNaN(hour) ? 0 : hour);
    this.timeMinuteRange = minutes.map(m => this.pad2(m));
    if (!this.timeMinuteRange.includes(this.timeMinute)) {
      this.timeMinute = this.timeMinuteRange[0] || '00';
    }
    this.selectedValue = `${this.timeHour}:${this.timeMinute}`;
  }

  private initTimePicker(config: DatePickerConfig): void {
    const start = this.parseTimeToMinutes(config.start) ?? 0;
    const end = this.parseTimeToMinutes(config.end) ?? (23 * 60 + 59);
    this.minTimeMinutes = Math.min(start, end);
    this.maxTimeMinutes = Math.max(start, end);

    const minHour = Math.floor(this.minTimeMinutes / 60);
    const maxHour = Math.floor(this.maxTimeMinutes / 60);
    const hours: string[] = [];
    for (let h = minHour; h <= maxHour; h++) hours.push(this.pad2(h));
    this.timeHourRange = hours;

    const initial = this.parseTimeToMinutes(typeof config.value === 'string' ? config.value : undefined) ?? this.minTimeMinutes;
    const clamped = Math.max(this.minTimeMinutes, Math.min(initial, this.maxTimeMinutes));
    this.timeHour = this.pad2(Math.floor(clamped / 60));
    this.timeMinute = this.pad2(clamped % 60);
    this.updateTimeMinuteRange();
  }

  private applyQuickRange(type: string): void {
    const today = this.startOfDay(new Date());
    const firstWeekday = this.firstWeekdayIndex(); // 0 Sun, 1 Mon
    const weekday = today.getDay();
    const daysFromStart = (weekday - firstWeekday + 7) % 7;

    let start = today;
    let end = today;

    switch (type) {
      case 'last7Days':
        start = new Date(today);
        start.setDate(today.getDate() - 6);
        break;
      case 'last30Days':
        start = new Date(today);
        start.setDate(today.getDate() - 29);
        break;
      case 'thisWeek':
        start = new Date(today);
        start.setDate(today.getDate() - daysFromStart);
        end = today;
        break;
      case 'lastWeek':
        start = new Date(today);
        start.setDate(today.getDate() - daysFromStart - 7);
        end = new Date(start);
        end.setDate(start.getDate() + 6);
        break;
      case 'thisMonth':
        start = new Date(today.getFullYear(), today.getMonth(), 1);
        end = today;
        break;
      case 'lastMonth':
        start = new Date(today.getFullYear(), today.getMonth() - 1, 1);
        end = new Date(today.getFullYear(), today.getMonth(), 0);
        break;
    }

    start = this.startOfDay(start);
    end = this.startOfDay(end);

    if (this.minDate && start.getTime() < this.startOfDay(this.minDate).getTime()) start = this.startOfDay(this.minDate);
    if (this.maxDate && end.getTime() > this.startOfDay(this.maxDate).getTime()) end = this.startOfDay(this.maxDate);
    if (start.getTime() > end.getTime()) return;

    this.rangeStart = start;
    this.rangeEnd = end;
    this.rangeComplete = true;
    this.rangeSelectingStart = true;
    this.currentMonth = this.monthStart(start);
    this.selectedValue = [this.formatDate(start), this.formatDate(end)];
    this.sendScrollResult(this.selectedValue as string[]);
  }

  private canConfirm(): boolean {
    if (this.config?.mode === 'date' && this.config?.fields === 'range') {
      return this.rangeComplete && !!this.rangeStart && !!this.rangeEnd;
    }
    return true;
  }

  private pickerHeight(): number {
    if (this.config?.mode === 'time') return 220;
    if (this.config?.fields === 'year' || this.config?.fields === 'month') return 220;
    if (this.config?.fields === 'range') return 364;
    return 292;
  }

  build() {
    if (this.isVisible && this.config) {
      Stack({ alignContent: Alignment.Bottom }) {
        Column()
          .width('100%')
          .height('100%')
          .backgroundColor('rgba(0, 0, 0, 0.5)')
          .onClick(() => {
            this.onCancel();
          })

        this.buildPickerContent()
      }
      .width('100%')
      .height('100%')
      .zIndex(1000)
    }
  }

  @Builder
  private buildPickerContent() {
    Column({ space: 0 }) {
      this.buildPickerWheels()

      Divider()
        .height(0.5)
        .color('#F0F0F0')

      Row() {
        Button({ type: ButtonType.Normal, stateEffect: true }) {
          Text(this.config?.cancelText || $r('app.string.lx_common_cancel'))
            .fontSize(17)
            .fontColor(this.config?.cancelTextColor || '#007AFF')
            .fontWeight(FontWeight.Normal)
            .textAlign(TextAlign.Center)
            .width('100%')
        }
        .layoutWeight(1)
        .height(46)
        .backgroundColor(this.config?.cancelButtonColor || '#F2F2F2')
        .borderRadius(12)
        .onClick(() => {
          this.onCancel();
        })

        Column().width(12)

        Button({ type: ButtonType.Normal, stateEffect: true }) {
          Text(this.config?.confirmText || $r('app.string.lx_common_confirm'))
            .fontSize(17)
            .fontColor(this.config?.confirmTextColor || '#FFFFFF')
            .fontWeight(FontWeight.Medium)
            .textAlign(TextAlign.Center)
            .width('100%')
        }
        .layoutWeight(1)
        .height(46)
        .backgroundColor(this.config?.confirmButtonColor || '#007AFF')
        .borderRadius(12)
        .enabled(this.canConfirm())
        .opacity(this.canConfirm() ? 1 : 0.45)
        .onClick(() => {
          this.onConfirm();
        })
      }
      .width('100%')
      .backgroundColor('#FFFFFF')
      .justifyContent(FlexAlign.Center)
      .alignItems(VerticalAlign.Center)
      .padding({ left: 16, right: 16, top: 10, bottom: 24 })
    }
    .width('100%')
    .backgroundColor(Color.White)
    .borderRadius({ topLeft: 16, topRight: 16, bottomLeft: 0, bottomRight: 0 })
  }

  @Builder
  private buildPickerWheels() {
    Column() {
      if (this.config?.mode === 'time') {
        this.buildTimePicker()
      } else if (this.config?.fields === 'year') {
        this.buildYearPicker()
      } else if (this.config?.fields === 'month') {
        this.buildMonthPicker()
      } else if (this.config?.fields === 'range') {
        this.buildCalendarPicker(true)
      } else {
        this.buildCalendarPicker(false)
      }
    }
    .width('100%')
    .height(this.pickerHeight())
    .padding({ left: 12, right: 12, top: 6, bottom: 4 })
  }

  @Builder
  private buildTimePicker() {
    Row({ space: 8 }) {
      TextPicker({
        range: this.timeHourRange,
        selected: Math.max(0, this.timeHourRange.indexOf(this.timeHour))
      })
        .onChange((value: string | string[]) => {
          const hour = Array.isArray(value) ? value[0] : value;
          this.timeHour = hour;
          this.updateTimeMinuteRange();
          const timeStr = `${this.timeHour}:${this.timeMinute}`;
          this.sendScrollResult(timeStr);
        })
        .height(220)
        .defaultPickerItemHeight(40)
        .canLoop(false)
        .layoutWeight(1)

      Text(':')
        .fontSize(20)
        .fontColor('#666666')
        .fontWeight(FontWeight.Medium)
        .margin({ bottom: 2 })

      TextPicker({
        range: this.timeMinuteRange,
        selected: Math.max(0, this.timeMinuteRange.indexOf(this.timeMinute))
      })
        .onChange((value: string | string[]) => {
          const minute = Array.isArray(value) ? value[0] : value;
          this.timeMinute = minute;
          this.selectedValue = `${this.timeHour}:${this.timeMinute}`;
          const timeStr = `${this.timeHour}:${this.timeMinute}`;
          this.sendScrollResult(timeStr);
        })
        .height(220)
        .defaultPickerItemHeight(40)
        .canLoop(false)
        .layoutWeight(1)
    }
    .width('100%')
    .height(220)
  }

  private getYearSelectedIndex(): number {
    const range = this.getYearRange();
    const selectedYear = (this.selectedValue as string) || new Date().getFullYear().toString();
    return Math.max(0, range.indexOf(selectedYear));
  }

  @Builder
  private buildYearPicker() {
    TextPicker({
      range: this.getYearRange(),
      selected: this.getYearSelectedIndex()
    })
      .onChange((value: string | string[], index: number | number[]) => {
        const year: string = Array.isArray(value) ? value[0] : value;
        this.selectedValue = year;
        this.sendScrollResult(year);
      })
      .height(220)
      .defaultPickerItemHeight(40)
      .canLoop(false)
      .width('100%')
  }

  private getYearRange(): string[] {
    const minYear: number = parseInt(this.config?.start?.substring(0, 4) || '1970');
    const maxYear: number = parseInt(this.config?.end?.substring(0, 4) || '2100');
    const years: string[] = [];
    for (let y = minYear; y <= maxYear; y++) {
      years.push(y.toString());
    }
    return years;
  }

  private getMonthYearSelectedIndex(): number {
    const currentVal: string = (this.selectedValue as string) || this.formatYearMonth(new Date());
    const currentYear = currentVal.split('-')[0] || new Date().getFullYear().toString();
    const yearRange = this.getYearRange();
    return Math.max(0, yearRange.indexOf(currentYear));
  }

  private getMonthMonthSelectedIndex(): number {
    const currentVal: string = (this.selectedValue as string) || this.formatYearMonth(new Date());
    const currentMonth = currentVal.split('-')[1] || '01';
    const monthRange = this.getMonthRange();
    return Math.max(0, monthRange.indexOf(currentMonth));
  }

  @Builder
  private buildMonthPicker() {
    Row({ space: 10 }) {
      TextPicker({
        range: this.getYearRange(),
        selected: this.getMonthYearSelectedIndex()
      })
        .onChange((value: string | string[], index: number | number[]) => {
          const year: string = Array.isArray(value) ? value[0] : value;
          const currentVal: string = (this.selectedValue as string) || '';
          const month: string = currentVal.split('-')[1] || '01';
          const newValue: string = `${year}-${month}`;
          this.selectedValue = newValue;
          this.sendScrollResult(newValue);
        })
        .height(220)
        .defaultPickerItemHeight(40)
        .canLoop(false)
        .layoutWeight(1)

      TextPicker({
        range: this.getMonthRange(),
        selected: this.getMonthMonthSelectedIndex()
      })
        .onChange((value: string | string[], index: number | number[]) => {
          const month: string = Array.isArray(value) ? value[0] : value;
          const currentVal: string = (this.selectedValue as string) || '';
          const year: string = currentVal.split('-')[0] || new Date().getFullYear().toString();
          const newValue: string = `${year}-${month}`;
          this.selectedValue = newValue;
          this.sendScrollResult(newValue);
        })
        .height(220)
        .defaultPickerItemHeight(40)
        .canLoop(false)
        .layoutWeight(1)
    }
    .width('100%')
    .height(220)
  }

  private getMonthRange(): string[] {
    const months: string[] = [];
    for (let m = 1; m <= 12; m++) {
      months.push(m.toString().padStart(2, '0'));
    }
    return months;
  }

  private getCalendarRowDays(rowIndex: number): Date[] {
    const days = this.calendarGridDays();
    return days.slice(rowIndex * 7, rowIndex * 7 + 7);
  }

  @Builder
  private buildCalendarPicker(isRange: boolean) {
    Column({ space: 0 }) {
      if (isRange) {
        this.buildQuickSelectChips()
      }

      Row() {
        this.buildNavButton('<<', this.canGoPrevYear(), true, () => {
          this.currentMonth = this.addYears(this.currentMonth, -1);
        })

        this.buildNavButton('<', this.canGoPrevMonth(), false, () => {
          this.currentMonth = this.addMonths(this.currentMonth, -1);
        })

        Text(this.formatMonthLabel(this.currentMonth))
          .fontSize(17)
          .fontColor('#111111')
          .fontWeight(FontWeight.Medium)
          .textAlign(TextAlign.Center)
          .layoutWeight(1)

        this.buildNavButton('>', this.canGoNextMonth(), false, () => {
          this.currentMonth = this.addMonths(this.currentMonth, 1);
        })

        this.buildNavButton('>>', this.canGoNextYear(), true, () => {
          this.currentMonth = this.addYears(this.currentMonth, 1);
        })
      }
      .width('100%')
      .height(32)
      .margin({ bottom: 12 })

      Row() {
        ForEach(this.weekdayLabels(), (label: string) => {
          Text(label)
            .fontSize(11)
            .fontColor('#8E8E93')
            .fontWeight(FontWeight.Medium)
            .textAlign(TextAlign.Center)
            .layoutWeight(1)
        })
      }
      .width('100%')
      .height(20)
      .margin({ bottom: 4 })

      Column({ space: 1 }) {
        ForEach([0, 1, 2, 3, 4, 5], (rowIndex: number) => {
          Row() {
            ForEach(this.getCalendarRowDays(rowIndex), (date: Date) => {
              this.buildCalendarDayCell(date, isRange)
            }, (date: Date) => {
              // Include selection state in key to force re-render when selection changes
              let stateSuffix = '';
              if (isRange) {
                stateSuffix = `${this.rangeStart?.getTime()}_${this.rangeEnd?.getTime()}`;
              } else {
                stateSuffix = `${this.selectedDate?.getTime()}`;
              }
              return `${date.getTime()}_${stateSuffix}`;
            })
          }
          .width('100%')
          .height(34)
        }, (index: number) => index.toString())
      }
      .width('100%')
    }
    .width('100%')
    .height(this.pickerHeight())
  }

      

        @Builder

        private buildNavButton(label: string, enabled: boolean, isYear: boolean, onClick: () => void) {

          Button({ type: ButtonType.Normal, stateEffect: true }) {

            Text(label)

              .fontSize(16)

              .fontColor('#007AFF')

              .fontWeight(FontWeight.Medium)

              .textAlign(TextAlign.Center)

              .width('100%')

          }

          .width(32)

          .height(32)

          .backgroundColor(Color.Transparent)

          .enabled(enabled)

          .opacity(enabled ? (isYear ? 0.55 : 1) : 0.25)

          .onClick(() => {

            if (enabled) onClick();

          })

        }

      

        @Builder

        private buildQuickSelectChips() {

          Column({ space: 6 }) {

            Row({ space: 8 }) {

              ForEach(this.quickSelectRow1, (item: QuickSelectOption) => {

                Button({ type: ButtonType.Normal, stateEffect: true }) {

                  Text(item.title)

                    .fontSize(13)

                    .fontColor('#007AFF')

                    .fontWeight(FontWeight.Medium)

                    .textAlign(TextAlign.Center)

                    .width('100%')

                }

                .layoutWeight(1)

                .height(28)

                .backgroundColor('rgba(0, 122, 255, 0.08)')

                .borderRadius(14)

                .onClick(() => {

                  this.applyQuickRange(item.key);

                })

              })

            }

            .width('100%')

      

            Row({ space: 8 }) {

              ForEach(this.quickSelectRow2, (item: QuickSelectOption) => {

                Button({ type: ButtonType.Normal, stateEffect: true }) {

                  Text(item.title)

                    .fontSize(13)

                    .fontColor('#007AFF')

                    .fontWeight(FontWeight.Medium)

                    .textAlign(TextAlign.Center)

                    .width('100%')

                }

                .layoutWeight(1)

                .height(28)

                .backgroundColor('rgba(0, 122, 255, 0.08)')

                .borderRadius(14)

                .onClick(() => {

                  this.applyQuickRange(item.key);

                })

              })

            }

            .width('100%')

          }

          .width('100%')

          .margin({ top: 2, bottom: 6 })

        }

      

        private calculateDayCellState(date: Date, isRange: boolean): DayCellState {

          const state = new DayCellState();

          

          if (isRange && this.rangeStart) {

            const start = this.rangeStart;

            const end = (this.rangeEnd ?? this.rangeStart);

            const s = start.getTime() <= end.getTime() ? start : end;

            const e = start.getTime() <= end.getTime() ? end : start;

            const dateTime = this.startOfDay(date).getTime();

            

            state.isStart = this.isSameDay(date, s);

            state.isEnd = this.isSameDay(date, e);

            state.isInRange = dateTime >= s.getTime() && dateTime <= e.getTime();

            state.showRangeBackground = state.isInRange && !(this.isSameDay(s, e));

            state.isSelected = state.isStart || state.isEnd;

          } else if (!isRange) {

            state.isSelected = this.isSameDay(date, this.selectedDate);

          }

          

          return state;

        }

      

      

      

        private getDayRadius(state: DayCellState): BorderRadiuses | number {

          if (state.showRangeBackground) {

             return state.isStart

                ? { topLeft: 15, bottomLeft: 15, topRight: 0, bottomRight: 0 }

                : (state.isEnd ? { topLeft: 0, bottomLeft: 0, topRight: 15, bottomRight: 15 } : 0);

          }

          return 0;

        }

      

        private getDayCellProps(date: Date, isRange: boolean): DayCellProps {

          const props = new DayCellProps();

          props.date = date;

          props.dayText = date.getDate().toString();

      

          const today = this.startOfDay(new Date());

          props.isToday = this.isSameDay(date, today);

          props.isInCurrentMonth = date.getFullYear() === this.currentMonth.getFullYear() && date.getMonth() === this.currentMonth.getMonth();

          

          const minTime = this.minDate ? this.startOfDay(this.minDate).getTime() : null;

          const maxTime = this.maxDate ? this.startOfDay(this.maxDate).getTime() : null;

          const dateTime = this.startOfDay(date).getTime();

          props.isDisabled = (minTime !== null && dateTime < minTime) || (maxTime !== null && dateTime > maxTime);

      

          const state = this.calculateDayCellState(date, isRange);

          props.isSelected = state.isSelected;

          props.showRangeBackground = state.showRangeBackground;

          props.radius = this.getDayRadius(state);

      

          return props;

        }

      

        @Builder

        private renderDayCell(props: DayCellProps) {

          Stack({ alignContent: Alignment.Center }) {

            if (props.showRangeBackground) {

              Stack()

                .width('100%')

                .height(30)

                .backgroundColor('rgba(0, 122, 255, 0.15)')

                .borderRadius(props.radius)

            }

      

            if (props.isSelected) {

              Stack()

                .width(30)

                .height(30)

                .backgroundColor('#007AFF')

                .borderRadius(15)

            } else if (props.isToday && !props.isDisabled) {

              Stack()

                .width(30)

                .height(30)

                .backgroundColor(Color.Transparent)

                .borderRadius(15)

                .border({ width: 1.5, color: '#007AFF' })

            }

      

            Text(props.dayText)

              .fontSize(15)

              .fontColor(props.isDisabled ? '#C7C7CC' : (props.isSelected ? '#FFFFFF' : (props.isInCurrentMonth ? '#111111' : '#636366')))

              .fontWeight(FontWeight.Medium)

          }

          .layoutWeight(1)

          .height(34)

          .opacity(props.isDisabled ? 0.3 : 1)

          .onClick(() => {

            if (props.isDisabled) return;

      

            const date = props.date;

            if (this.config?.fields === 'range') { // Check fields from config implicitly via context or just assume isRange logic applies

               // Re-implement click logic or call a handler

               this.handleDayClick(date);

            } else {

               this.handleDayClick(date);

            }

          })

        }

      

          private handleDayClick(date: Date) {

      

              hilog.info(DOMAIN, TAG, `handleDayClick: ${date.toISOString()}`);

      

              if (this.config?.fields === 'range') {

              if (this.rangeSelectingStart) {

                this.rangeStart = date;

                this.rangeEnd = date;

                this.rangeComplete = false;

                this.rangeSelectingStart = false;

                this.selectedValue = [this.formatDate(date), this.formatDate(date)];

                this.sendScrollResult(this.selectedValue as string[]);

              } else {

                const start = this.rangeStart ?? date;

                const s = start.getTime() <= date.getTime() ? start : date;

                const e = start.getTime() <= date.getTime() ? date : start;

                this.rangeStart = s;

                this.rangeEnd = e;

                this.rangeComplete = true;

                this.rangeSelectingStart = true;

                this.selectedValue = [this.formatDate(s), this.formatDate(e)];

                this.sendScrollResult(this.selectedValue as string[]);

              }

            } else {

              this.selectedDate = date;

              this.selectedValue = date;

              this.sendScrollResult(this.formatDate(date));

            }

      

            if (date.getMonth() !== this.currentMonth.getMonth()) { // Check mismatch logic properly

               // Simplification: if not in current view, jump

               // But I don't have isInCurrentMonth passed here easily without props. 

               // Actually I can recalc or just check month.

               if (date.getFullYear() !== this.currentMonth.getFullYear() || date.getMonth() !== this.currentMonth.getMonth()) {

                   this.currentMonth = this.monthStart(date);

               }

            }

        }

  @Builder
  private buildCalendarDayCell(date: Date, isRange: boolean) {
    this.renderDayCell(this.getDayCellProps(date, isRange))
  }
}

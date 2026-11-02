// The View's own pickers for `type="date"`/`"time"` editors and `<select>`: drawn like the other
// overlays, themed by roles, the same on every platform. Native opens one when the control is tapped or
// activated, forwards the picker keys while it is open, and applies the result through the control's
// own path (the editor's pick, the select's choice).
import { h } from './nodes.js';
import { listTag } from './elements.js';
import { monthGrid, weekday, weekdayOrder, moveDate, within, clampDate, compare, addMonths, parseDate, parseTime, iso, timeIso,
  hourValues, minuteValues, wordsFor } from './calendar.js';

let picker = null, results = [], nextKey = 1;

let fixedToday = null;
const today = () => {
  if (fixedToday) return fixedToday;
  const now = new Date();
  return { year: now.getFullYear(), month: now.getMonth() + 1, day: now.getDate() };
};

/**
 * A date or time editor asked for its picker: `{identity, kind, value, min, max, step}` from native,
 * `identity` being the editor's native key, returned with the result. False for any other kind.
 */
export function openPicker({ identity, kind, value, min, max, step, today: day, options, selected, label, anchorWidth } = {}) {
  if (typeof identity !== 'string' || !['date', 'time', 'select'].includes(kind)) return false;
  fixedToday = parseDate(day);
  const key = nextKey++;
  if (kind === 'select') {
    const list = Array.isArray(options) ? options.slice(0, 1024).map(option => ({
      value: String(option.value), label: String(option.label), enabled: option.enabled !== false })) : [];
    if (!list.length) return false;
    const chosen = Number.isInteger(selected) && list[selected] ? selected : -1;
    // As wide as its field up to 360 pt, never narrower than its options.
    const width = Number.isFinite(anchorWidth) ? Math.min(Math.max(anchorWidth, 0), 360) : 0;
    picker = { kind, key, identity, options: list, selected: chosen, focus: chosen >= 0 ? chosen : list.findIndex(o => o.enabled),
      label: typeof label === 'string' ? label : '', typed: '', typedAt: 0, width };
    return true;
  }
  if (kind === 'date') {
    const low = parseDate(min), high = parseDate(max);
    picker = { kind, key, identity, min: low, max: high, value: parseDate(value), focus: clampDate(parseDate(value) ?? today(), low, high) };
  } else {
    picker = { kind, key, identity, step: Number(step), value: parseTime(value) ?? { hour: 9, minute: 0 }, min: parseTime(min), max: parseTime(max) };
  }
  return true;
}

export const hasPicker = () => picker !== null;
export const pickerIdentity = () => picker?.identity ?? null;

function settle(value) {
  if (!picker) return;
  results.push({ identity: picker.identity, value });
  picker = null;
}

/** The editor lost focus some other way (blur, unmount): the picker goes without a choice. */
export function closePicker() { picker = null; }

/** Results the picker produced: `[{identity, value}]`, `value` null for a cancel. */
export function takePickerResults() { const out = results; results = []; return JSON.stringify(out); }

function settleSelect(index) {
  const option = picker.options[index];
  if (!option?.enabled) return;
  results.push({ identity: picker.identity, value: option.value, index });
  picker = null;
}

function selectKey(key) {
  const { options } = picker;
  const step = { ArrowDown: 1, ArrowUp: -1, PageDown: 8, PageUp: -8 }[key];
  if (step !== undefined) {
    let next = picker.focus;
    for (let moved = 0; moved < Math.abs(step); moved++) {
      let probe = next;
      do probe += Math.sign(step); while (probe >= 0 && probe < options.length && !options[probe].enabled);
      if (probe < 0 || probe >= options.length) break;
      next = probe;
    }
    picker.focus = next;
    return true;
  }
  if (key === 'Enter') { settleSelect(picker.focus); return true; }
  if (key.length === 1) {
    // Type-ahead: letters typed within a second match the start of an option's label.
    const now = Date.now();
    picker.typed = (now - picker.typedAt < 1000 ? picker.typed : '') + key.toLowerCase();
    picker.typedAt = now;
    const found = options.findIndex(o => o.enabled && o.label.toLowerCase().startsWith(picker.typed));
    if (found >= 0) picker.focus = found;
    return true;
  }
  return false;
}

/** A key while the picker is open; true when the picker used it. */
export function pickerKey(key, rtl = false) {
  if (!picker) return false;
  if (key === 'Escape') { settle(null); return true; }
  if (picker.kind === 'select') return selectKey(key);
  if (picker.kind === 'date') {
    if (key === 'Enter') { if (within(picker.focus, picker.min, picker.max)) settle(iso(picker.focus)); return true; }
    const moved = moveDate(picker.focus, key, rtl);
    if (!moved) return false;
    picker.focus = clampDate(moved, picker.min, picker.max);
    return true;
  }
  if (key === 'Enter') { settle(timeIso(picker.value)); return true; }
  if (/^[0-9]$/.test(key)) {
    // Typed digits fill HH then MM, as a desktop time field does.
    picker.typed = ((picker.typed ?? '') + key).slice(-4);
    const digits = picker.typed.padStart(4, '0');
    const hour = Number(digits.slice(0, 2)), minute = Number(digits.slice(2));
    if (hour < 24 && minute < 60) picker.value = { hour, minute };
    return true;
  }
  const delta = { ArrowUp: -1, ArrowDown: 1 }[key];
  if (delta === undefined) return false;
  const minutes = minuteValues(picker.step);
  const at = Math.max(0, minutes.indexOf(picker.value.minute));
  const index = at + delta;
  if (index < 0) picker.value = { hour: (picker.value.hour + 23) % 24, minute: minutes.at(-1) };
  else if (index >= minutes.length) picker.value = { hour: (picker.value.hour + 1) % 24, minute: minutes[0] };
  else picker.value = { ...picker.value, minute: minutes[index] };
  return true;
}

// On touch the sheet's toolbar carries the month title; the body keeps the month arrows.
function dateBody(words, env, titled) {
  const { focus, value, min, max } = picker;
  const first = env?.firstWeekday ?? 1, now = today();
  const shift = months => { picker.focus = clampDate(addMonths(picker.focus, months), min, max); };
  const iconButton = (testId, label, d, months) => h('button', { 'data-testid': testId, 'aria-label': label, onClick: () => shift(months),
    style: { width: 36, height: 36, borderRadius: 18, alignItems: 'center', justifyContent: 'center', color: 'text' } },
    h('vector', { 'aria-hidden': true, paths: [{ d, fill: 'none', stroke: 'currentColor', strokeWidth: 2, strokeLinecap: 'round', strokeLinejoin: 'round' }],
      style: { width: 20, height: 20 } }));
  const title = words.title(words.months[focus.month - 1], focus.year);
  const cells = monthGrid(focus, first);
  const rows = Array.from({ length: 6 }, (_, week) => cells.slice(week * 7, week * 7 + 7));
  const cell = ({ date, inMonth }) => {
    const id = iso(date), enabled = within(date, min, max);
    const selected = value && compare(date, value) === 0, focused = compare(date, focus) === 0, isToday = compare(date, now) === 0;
    const name = words.full(date.day, words.months[date.month - 1], date.year, words.longDays[weekday(date.year, date.month, date.day)]);
    return h('button', { key: id, 'data-testid': `lx-picker-day-${id}`, role: 'gridcell', 'aria-label': name, 'aria-selected': Boolean(selected),
      disabled: !enabled, onClick: () => settle(id),
      style: { width: 36, height: 36, borderRadius: 18, alignItems: 'center', justifyContent: 'center',
        backgroundColor: selected ? 'accent' : focused ? 'surface-sunken' : 'transparent',
        borderWidth: isToday && !selected ? 1 : 0, borderColor: 'accent',
        color: selected ? 'on-accent' : !inMonth || !enabled ? 'text-faint' : 'text', fontSize: 14, fontWeight: selected || isToday ? 600 : 400 } },
      String(date.day));
  };
  // Content-sized and centred, so a full-width sheet keeps the month compact.
  return h('column', { style: { gap: 8, alignSelf: 'center' } },
    h('row', { style: { alignItems: 'center', gap: 4, justifyContent: titled ? 'start' : 'space-between' } },
      titled && h('text', { 'data-testid': 'lx-picker-title', 'aria-live': 'polite', style: { flexGrow: 1, fontSize: 16, fontWeight: 600, color: 'text', paddingLeft: 8 } }, title),
      iconButton('lx-picker-previous', words.previous, 'M12.5 4l-6 6 6 6', -1),
      iconButton('lx-picker-next', words.next, 'M7.5 4l6 6-6 6', 1)),
    h('column', { role: 'grid', 'aria-label': title, style: { gap: 2 } },
      h('row', { 'aria-hidden': true, style: { gap: 2 } }, ...weekdayOrder(first).map(day => h('text', { key: `w${day}`,
        style: { width: 36, textAlign: 'center', fontSize: 12, color: 'text-muted' } }, words.days[day]))),
      ...rows.map((week, index) => h('row', { key: `r${index}`, style: { gap: 2 } }, ...week.map(cell)))));
}

const ROW = 40, VISIBLE = 5;

// A column of choices whose selected row starts in the middle band; scrolling snaps row by row. A
// cyclic column (hours, minutes) repeats its values around the middle copy, so it never shows blanks.
function timeColumn(testId, label, values, selected, format, choose, cyclic = false) {
  const copies = cyclic ? 3 : 1, middle = cyclic ? 1 : 0;
  const index = middle * values.length + Math.max(0, values.indexOf(selected));
  const pad = cyclic ? 0 : ROW * Math.floor(VISIBLE / 2);
  const offset = cyclic ? Math.floor(VISIBLE / 2) : 0;
  return h('column', { 'data-testid': testId, role: 'listbox', 'aria-label': label, style: { width: 64 } },
    h(listTag, { count: values.length * copies, itemKeys: Array.from({ length: values.length * copies }, (_, i) => `${Math.floor(i / values.length)}:${values[i % values.length]}`),
      height: () => ROW, anchor: { index: Math.max(0, index - offset), align: 'start', seq: index },
      style: { height: ROW * VISIBLE, paddingTop: pad, paddingBottom: pad, scrollSnapType: 'y' },
      row: i => {
        const value = values[i % values.length], copy = Math.floor(i / values.length), on = value === selected;
        // Automation names the middle copy's rows; the others are its repeats.
        const id = copy === middle ? `${testId}-${value}` : `${testId}-${value}-${copy}`;
        return h('button', { key: `${copy}:${value}`, 'data-testid': id, role: 'option', 'aria-selected': on && copy === middle,
          'aria-hidden': copy !== middle, onClick: () => choose(value),
          style: { height: ROW, alignItems: 'center', justifyContent: 'center', borderRadius: 8, scrollSnapAlign: 'center',
            backgroundColor: on ? 'accent/12' : 'transparent', color: on ? 'accent' : 'text',
            fontSize: 17, fontWeight: on ? 600 : 400, fontVariantNumeric: 'tabular-nums' } }, format(value));
      } }));
}

function timeBody(words, env) {
  const cycle = env?.hourCycle === 'h12' ? 'h12' : 'h23';
  const { hour, minute } = picker.value;
  const two = value => String(value).padStart(2, '0');
  const set = next => { picker.value = { ...picker.value, ...next }; picker.typed = ''; };
  const columns = [
    timeColumn('lx-picker-hour', words.hour, hourValues(cycle), cycle === 'h12' ? (hour % 12 || 12) : hour, cycle === 'h12' ? String : two,
      value => set({ hour: cycle === 'h12' ? (value % 12) + (hour >= 12 ? 12 : 0) : value }), true),
    timeColumn('lx-picker-minute', words.minute, minuteValues(picker.step), minute, two, value => set({ minute: value }), true),
  ];
  if (cycle === 'h12') columns.push(timeColumn('lx-picker-period', words.period, ['am', 'pm'], hour >= 12 ? 'pm' : 'am', period => words[period],
    period => set({ hour: (hour % 12) + (period === 'pm' ? 12 : 0) })));
  return h('row', { style: { gap: 8, justifyContent: 'center' } }, ...columns);
}

function selectBody() {
  const { options, selected, focus } = picker;
  const check = 'M4 10.5l4 4 8-9';
  // The band follows the keyboard; the trailing check marks the choice.
  return h('column', { role: 'listbox', 'aria-label': picker.label || 'Options', style: { gap: 2, minWidth: Math.max(160, picker.width - 24) } },
    ...options.map((option, index) => h('button', { key: `o${index}`, 'data-testid': `lx-picker-option-${index}`, role: 'option',
      'aria-selected': index === selected, disabled: !option.enabled, onClick: () => settleSelect(index),
      style: { flexDirection: 'row', alignItems: 'center', gap: 8, minHeight: 36, paddingLeft: 12, paddingRight: 10, borderRadius: 8,
        backgroundColor: index === focus ? 'surface-sunken' : 'transparent', color: option.enabled ? 'text' : 'text-faint', fontSize: 15 } },
      h('text', { style: { flexGrow: 1, textAlign: 'start' } }, option.label),
      index === selected
        ? h('vector', { 'aria-hidden': true, paths: [{ d: check, fill: 'none', stroke: 'currentColor', strokeWidth: 2,
          strokeLinecap: 'round', strokeLinejoin: 'round' }], style: { width: 18, height: 18, color: 'accent' } })
        : h('spacer', { style: { width: 18, height: 18 } }))));
}

/** The popover's anchor: the editor it opened from, or null for a sheet. */
export const pickerAnchor = env => picker && (env?.pointer ?? 'coarse') !== 'coarse' ? picker.identity : null;

const sheetStyle = ({ borderRadius, ...rest }) => rest;
const clock = ({ hour, minute }, words, env) => {
  const mm = String(minute).padStart(2, '0');
  if (env?.hourCycle !== 'h12') return `${String(hour).padStart(2, '0')}:${mm}`;
  return `${hour % 12 || 12}:${mm} ${hour < 12 ? words.am : words.pm}`;
};

/** The open picker above the page, or null. */
export function pickerNode(env) {
  if (!picker) return null;
  const words = wordsFor(env?.displayLanguage);
  const touch = (env?.pointer ?? 'coarse') === 'coarse';
  const body = picker.kind === 'date' ? dateBody(words, env, !touch) : picker.kind === 'time' ? timeBody(words, env) : selectBody();
  const commit = () => picker.kind === 'date' ? settle(iso(picker.focus))
    : picker.kind === 'time' ? settle(timeIso(picker.value)) : settleSelect(picker.focus);
  const cancel = style => h('button', { 'data-testid': 'lx-picker-cancel', onClick: () => settle(null), style }, words.cancel);
  const done = style => h('button', { 'data-testid': 'lx-picker-done', onClick: commit, style }, words.done);
  const monthTitle = picker.kind === 'date' && words.title(words.months[picker.focus.month - 1], picker.focus.year);
  // Touch: a toolbar on top, as iOS's and WeChat's picker sheets have, clear of the home indicator.
  const text = { paddingTop: 8, paddingBottom: 8, paddingLeft: 4, paddingRight: 4, fontSize: 16, backgroundColor: 'transparent' };
  const toolbar = touch && h('row', { style: { alignItems: 'center', gap: 8 } },
    cancel({ ...text, color: 'text-muted' }),
    h('text', { 'data-testid': 'lx-picker-title', 'aria-live': 'polite',
      style: { flexGrow: 1, flexBasis: 0, minWidth: 0, textAlign: 'center', fontSize: 16, fontWeight: 600, color: 'text', lineClamp: 1 } },
      monthTitle || (picker.kind === 'time' ? clock(picker.value, words, env) : picker.label || '')),
    done({ ...text, color: 'accent', fontWeight: 600 }));
  // A mouse: the time popover confirms with a button pair below its columns.
  const button = { paddingTop: 8, paddingBottom: 8, paddingLeft: 14, paddingRight: 14, borderRadius: 8, fontSize: 15 };
  const footer = !touch && picker.kind === 'time' && h('row', { style: { gap: 8, justifyContent: 'end' } },
    cancel({ ...button, color: 'text' }), done({ ...button, backgroundColor: 'accent', color: 'on-accent', fontWeight: 600 }));
  const name = picker.kind === 'date' ? monthTitle : picker.kind === 'time' ? words.hour : picker.label || 'Options';
  const panel = { 'data-testid': 'lx-picker', key: `picker-${picker.key}`, 'aria-label': name,
    style: { padding: 12, gap: 12, borderRadius: 14, backgroundColor: 'surface-raised', borderWidth: touch ? 0 : 1, borderColor: 'line',
      boxShadow: [{ offsetX: 0, offsetY: 8, blur: 24, spread: 0, color: 'shadow/20' }] } };
  // Touch: a bottom sheet. A mouse: a popover that native moves against its editor.
  return touch
    ? h('dialog', { ...panel, open: true, closedby: 'any', onClose: () => settle(null),
      // Edge to edge and flush with the bottom; the content clears the home indicator.
      style: { ...sheetStyle(panel.style), marginTop: 'auto', marginBottom: 0, marginLeft: 0, marginRight: 0, width: '100%',
        borderTopLeftRadius: 14, borderTopRightRadius: 14, paddingBottom: 12 + (env?.safeArea?.bottom ?? 0),
        paddingLeft: 12 + (env?.safeArea?.left ?? 0), paddingRight: 12 + (env?.safeArea?.right ?? 0) } }, toolbar, body)
    : h('view', { ...panel, id: 'lx-picker', popover: 'auto', open: true, onToggle: open => { if (!open) settle(null); },
      style: { ...panel.style, position: 'absolute', top: 0, left: 0 } }, body, footer);
}

export function resetPicker() { picker = null; results = []; }

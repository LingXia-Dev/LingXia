// Calendar arithmetic and names for the View's own date and time pickers. The runtime has no
// Intl, so names come from these tables; other languages read English names with their own
// week start and clock from `env`. The words (Cancel, Done, …) are `@view.*`.
import { word } from './words.js';

const names = {
  en: { months: ['January', 'February', 'March', 'April', 'May', 'June', 'July', 'August', 'September', 'October', 'November', 'December'],
    days: ['Sun', 'Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat'], longDays: ['Sunday', 'Monday', 'Tuesday', 'Wednesday', 'Thursday', 'Friday', 'Saturday'],
    title: (month, year) => `${month} ${year}`, full: (day, month, year, weekday) => `${weekday}, ${month} ${day}, ${year}`,
    am: 'AM', pm: 'PM' },
  zh: { months: ['1月', '2月', '3月', '4月', '5月', '6月', '7月', '8月', '9月', '10月', '11月', '12月'],
    days: ['日', '一', '二', '三', '四', '五', '六'], longDays: ['星期日', '星期一', '星期二', '星期三', '星期四', '星期五', '星期六'],
    title: (month, year) => `${year}年${month}`, full: (day, month, year, weekday) => `${year}年${month}${day}日${weekday}`,
    am: '上午', pm: '下午' },
  ja: { months: ['1月', '2月', '3月', '4月', '5月', '6月', '7月', '8月', '9月', '10月', '11月', '12月'],
    days: ['日', '月', '火', '水', '木', '金', '土'], longDays: ['日曜日', '月曜日', '火曜日', '水曜日', '木曜日', '金曜日', '土曜日'],
    title: (month, year) => `${year}年${month}`, full: (day, month, year, weekday) => `${year}年${month}${day}日 ${weekday}`,
    am: '午前', pm: '午後' },
  ko: { months: ['1월', '2월', '3월', '4월', '5월', '6월', '7월', '8월', '9월', '10월', '11월', '12월'],
    days: ['일', '월', '화', '수', '목', '금', '토'], longDays: ['일요일', '월요일', '화요일', '수요일', '목요일', '금요일', '토요일'],
    title: (month, year) => `${year}년 ${month}`, full: (day, month, year, weekday) => `${year}년 ${month} ${day}일 ${weekday}`,
    am: '오전', pm: '오후' },
  de: { months: ['Januar', 'Februar', 'März', 'April', 'Mai', 'Juni', 'Juli', 'August', 'September', 'Oktober', 'November', 'Dezember'],
    days: ['So', 'Mo', 'Di', 'Mi', 'Do', 'Fr', 'Sa'], longDays: ['Sonntag', 'Montag', 'Dienstag', 'Mittwoch', 'Donnerstag', 'Freitag', 'Samstag'],
    title: (month, year) => `${month} ${year}`, full: (day, month, year, weekday) => `${weekday}, ${day}. ${month} ${year}`,
    am: 'AM', pm: 'PM' },
  fr: { months: ['janvier', 'février', 'mars', 'avril', 'mai', 'juin', 'juillet', 'août', 'septembre', 'octobre', 'novembre', 'décembre'],
    days: ['dim.', 'lun.', 'mar.', 'mer.', 'jeu.', 'ven.', 'sam.'], longDays: ['dimanche', 'lundi', 'mardi', 'mercredi', 'jeudi', 'vendredi', 'samedi'],
    title: (month, year) => `${month} ${year}`, full: (day, month, year, weekday) => `${weekday} ${day} ${month} ${year}`,
    am: 'AM', pm: 'PM' },
  es: { months: ['enero', 'febrero', 'marzo', 'abril', 'mayo', 'junio', 'julio', 'agosto', 'septiembre', 'octubre', 'noviembre', 'diciembre'],
    days: ['dom', 'lun', 'mar', 'mié', 'jue', 'vie', 'sáb'], longDays: ['domingo', 'lunes', 'martes', 'miércoles', 'jueves', 'viernes', 'sábado'],
    title: (month, year) => `${month} de ${year}`, full: (day, month, year, weekday) => `${weekday}, ${day} de ${month} de ${year}`,
    am: 'a. m.', pm: 'p. m.' },
  pt: { months: ['janeiro', 'fevereiro', 'março', 'abril', 'maio', 'junho', 'julho', 'agosto', 'setembro', 'outubro', 'novembro', 'dezembro'],
    days: ['dom.', 'seg.', 'ter.', 'qua.', 'qui.', 'sex.', 'sáb.'], longDays: ['domingo', 'segunda-feira', 'terça-feira', 'quarta-feira', 'quinta-feira', 'sexta-feira', 'sábado'],
    title: (month, year) => `${month} de ${year}`, full: (day, month, year, weekday) => `${weekday}, ${day} de ${month} de ${year}`,
    am: 'AM', pm: 'PM' },
  ru: { months: ['январь', 'февраль', 'март', 'апрель', 'май', 'июнь', 'июль', 'август', 'сентябрь', 'октябрь', 'ноябрь', 'декабрь'],
    days: ['вс', 'пн', 'вт', 'ср', 'чт', 'пт', 'сб'], longDays: ['воскресенье', 'понедельник', 'вторник', 'среда', 'четверг', 'пятница', 'суббота'],
    title: (month, year) => `${month} ${year}`, full: (day, month, year, weekday) => `${weekday}, ${day} ${month} ${year}`,
    am: 'AM', pm: 'PM' },
  ar: { months: ['يناير', 'فبراير', 'مارس', 'أبريل', 'مايو', 'يونيو', 'يوليو', 'أغسطس', 'سبتمبر', 'أكتوبر', 'نوفمبر', 'ديسمبر'],
    days: ['أحد', 'إثنين', 'ثلاثاء', 'أربعاء', 'خميس', 'جمعة', 'سبت'], longDays: ['الأحد', 'الإثنين', 'الثلاثاء', 'الأربعاء', 'الخميس', 'الجمعة', 'السبت'],
    title: (month, year) => `${month} ${year}`, full: (day, month, year, weekday) => `${weekday}، ${day} ${month} ${year}`,
    am: 'ص', pm: 'م' },
};

/** A time as the user's clock shows it: `21:30` (h23) or `9:30 PM` (h12), seconds when given. */
export function clockText({ hour, minute, second }, words, hourCycle) {
  const rest = `:${pad(minute)}${second === undefined ? '' : `:${pad(second)}`}`;
  if (hourCycle !== 'h12') return `${pad(hour)}${rest}`;
  return `${hour % 12 || 12}${rest} ${hour < 12 ? words.am : words.pm}`;
}

/**
 * What a date or time field shows for its ISO value, and the widest text it can show in this
 * format, one candidate per line; digits in `sizeText` stand for any digit.
 */
export function fieldText(kind, value, { seconds, language, hourCycle }) {
  if (kind === 'date') {
    const format = dateFormat(language);
    const date = parseDate(value);
    // Every month's name with the widest day and year: the field is as wide as its widest value.
    const sizeText = [...new Set(Array.from({ length: 12 }, (_, month) =>
      format({ year: 2000, month: month + 1, day: 10 }).replace(/\d/g, '0')))].join('\n');
    return { displayValue: date ? format(date) : value, sizeText };
  }
  const words = wordsFor(language);
  const shown = parseTime(value);
  const widest = second => [words.am, words.pm].map(period =>
    clockText({ hour: 12, minute: 0, second }, { am: period, pm: period }, hourCycle).replace(/\d/g, '0'));
  const sizeText = [...new Set(widest(seconds || shown?.second !== undefined ? 0 : undefined))].join('\n');
  return { displayValue: shown ? clockText(shown, words, hourCycle) : value, sizeText };
}

// Short month names for the date formats that spell the month.
const shortMonths = {
  en: ['Jan', 'Feb', 'Mar', 'Apr', 'May', 'Jun', 'Jul', 'Aug', 'Sep', 'Oct', 'Nov', 'Dec'],
  fr: ['janv.', 'févr.', 'mars', 'avr.', 'mai', 'juin', 'juil.', 'août', 'sept.', 'oct.', 'nov.', 'déc.'],
  es: ['ene', 'feb', 'mar', 'abr', 'may', 'jun', 'jul', 'ago', 'sept', 'oct', 'nov', 'dic'],
  pt: ['jan.', 'fev.', 'mar.', 'abr.', 'mai.', 'jun.', 'jul.', 'ago.', 'set.', 'out.', 'nov.', 'dez.'],
  ru: ['янв.', 'февр.', 'мар.', 'апр.', 'мая', 'июн.', 'июл.', 'авг.', 'сент.', 'окт.', 'нояб.', 'дек.'],
};
// English puts the month first in these regions and the day first elsewhere.
const monthFirst = new Set(['US', 'CA', 'PH']);

/**
 * A date as a field shows it in a BCP 47 language: the medium date its platforms show
 * (`Nov 14, 2026`, `14 Nov 2026`, `2026年11月14日`, `14.11.2026`); English's for others.
 */
export function dateFormat(language) {
  const parts = String(language ?? '').split(/[-_]/);
  const lang = parts[0].toLowerCase();
  const region = parts.slice(1).find(part => /^[A-Za-z]{2}$|^\d{3}$/.test(part))?.toUpperCase();
  switch (lang) {
    case 'zh': return ({ year, month, day }) => `${year}年${month}月${day}日`;
    case 'ja': return ({ year, month, day }) => `${year}/${pad(month)}/${pad(day)}`;
    case 'ko': return ({ year, month, day }) => `${year}. ${month}. ${day}.`;
    case 'de': return ({ year, month, day }) => `${pad(day)}.${pad(month)}.${year}`;
    case 'fr': return ({ year, month, day }) => `${day} ${shortMonths.fr[month - 1]} ${year}`;
    case 'es': return ({ year, month, day }) => `${day} ${shortMonths.es[month - 1]} ${year}`;
    case 'pt': return ({ year, month, day }) => `${day} de ${shortMonths.pt[month - 1]} de ${year}`;
    case 'ru': return ({ year, month, day }) => `${day} ${shortMonths.ru[month - 1]} ${year} г.`;
    case 'ar': return ({ year, month, day }) => `${day}\u200f/${month}\u200f/${year}`;
    default:
      return lang === 'en' && region !== undefined && !monthFirst.has(region)
        ? ({ year, month, day }) => `${day} ${shortMonths.en[month - 1]} ${year}`
        : ({ year, month, day }) => `${shortMonths.en[month - 1]} ${day}, ${year}`;
  }
}

/** Picker names and words for a BCP 47 language; names in English when it has no table. */
export function wordsFor(language) {
  const table = names[String(language ?? '').split(/[-_]/)[0].toLowerCase()] ?? names.en;
  const w = key => word(`@view.${key}`, language);
  return { ...table, cancel: w('cancel'), done: w('done'), previous: w('previousMonth'), next: w('nextMonth'),
    hour: w('hour'), minute: w('minute'), second: w('second'), period: w('period'), options: w('options') };
}

const pad = (value, width = 2) => String(value).padStart(width, '0');
const isLeap = year => year % 4 === 0 && (year % 100 !== 0 || year % 400 === 0);
export const daysIn = (year, month) => [31, isLeap(year) ? 29 : 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31][month - 1];

/** 0 = Sunday for a proleptic Gregorian date. */
export function weekday(year, month, day) {
  const t = [0, 3, 2, 5, 0, 3, 5, 1, 4, 6, 2, 4];
  const y = month < 3 ? year - 1 : year;
  return (y + Math.floor(y / 4) - Math.floor(y / 100) + Math.floor(y / 400) + t[month - 1] + day) % 7;
}

export const iso = ({ year, month, day }) => `${pad(year, 4)}-${pad(month)}-${pad(day)}`;
export function parseDate(value) {
  const match = /^(\d{4})-(\d{2})-(\d{2})$/.exec(value ?? '');
  if (!match) return null;
  const [year, month, day] = match.slice(1).map(Number);
  return month >= 1 && month <= 12 && day >= 1 && day <= daysIn(year, month) ? { year, month, day } : null;
}
/** `HH:MM` or `HH:MM:SS`; `second` only when the text has one. */
export function parseTime(value) {
  const match = /^(\d{2}):(\d{2})(?::(\d{2}))?$/.exec(value ?? '');
  if (!match) return null;
  const [hour, minute] = match.slice(1, 3).map(Number);
  if (hour > 23 || minute > 59) return null;
  if (match[3] === undefined) return { hour, minute };
  const second = Number(match[3]);
  return second < 60 ? { hour, minute, second } : null;
}

/** The date `days` after `date` (negative before). */
export function addDays({ year, month, day }, days) {
  let ordinal = toOrdinal(year, month, day) + days;
  return fromOrdinal(ordinal);
}
/** The same day `months` later, clamped to the month's length. */
export function addMonths({ year, month, day }, months) {
  const index = year * 12 + (month - 1) + months;
  const y = Math.floor(index / 12), m = index - y * 12 + 1;
  return { year: y, month: m, day: Math.min(day, daysIn(y, m)) };
}
function toOrdinal(year, month, day) {
  // Days since 0000-03-01, for whole-day arithmetic without Date.
  const y = month <= 2 ? year - 1 : year, m = month <= 2 ? month + 9 : month - 3;
  return 365 * y + Math.floor(y / 4) - Math.floor(y / 100) + Math.floor(y / 400) + Math.floor((153 * m + 2) / 5) + day - 1;
}
function fromOrdinal(ordinal) {
  let y = Math.floor((10000 * ordinal + 14780) / 3652425);
  let rest = ordinal - (365 * y + Math.floor(y / 4) - Math.floor(y / 100) + Math.floor(y / 400));
  if (rest < 0) { y -= 1; rest = ordinal - (365 * y + Math.floor(y / 4) - Math.floor(y / 100) + Math.floor(y / 400)); }
  const mi = Math.floor((100 * rest + 52) / 3060);
  const month = (mi + 2) % 12 + 1, year = y + Math.floor((mi + 2) / 12), day = rest - Math.floor((mi * 306 + 5) / 10) + 1;
  return { year, month, day };
}
export const compare = (a, b) => (a.year - b.year) || (a.month - b.month) || (a.day - b.day);

/**
 * Six weeks of cells for the month of `view`, starting on `firstWeekday` (1 = Monday … 7 = Sunday):
 * `{date, inMonth}` each, so every month draws the same grid height.
 */
export function monthGrid({ year, month }, firstWeekday = 7) {
  const lead = (weekday(year, month, 1) - firstWeekday % 7 + 7) % 7;
  const start = addDays({ year, month, day: 1 }, -lead);
  return Array.from({ length: 42 }, (_, index) => {
    const date = addDays(start, index);
    return { date, inMonth: date.month === month && date.year === year };
  });
}

/** Weekday indexes (0 = Sunday) in column order, from `firstWeekday` (1 = Monday … 7 = Sunday). */
export const weekdayOrder = firstWeekday => Array.from({ length: 7 }, (_, index) => (firstWeekday + index) % 7);

/** The date a key moves `date` to, or null for a key the grid does not use. */
export function moveDate(date, key, rtl = false) {
  switch (key) {
    case 'ArrowLeft': return addDays(date, rtl ? 1 : -1);
    case 'ArrowRight': return addDays(date, rtl ? -1 : 1);
    case 'ArrowUp': return addDays(date, -7);
    case 'ArrowDown': return addDays(date, 7);
    case 'PageUp': return addMonths(date, -1);
    case 'PageDown': return addMonths(date, 1);
    default: return null;
  }
}

export const within = (date, min, max) => (!min || compare(date, min) >= 0) && (!max || compare(date, max) <= 0);
export const clampDate = (date, min, max) => min && compare(date, min) < 0 ? min : max && compare(date, max) > 0 ? max : date;

/** Hour labels and values for a column: 0–23, or 1–12 with the day period separate. */
export function hourValues(cycle) {
  return cycle === 'h12' ? [12, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11] : Array.from({ length: 24 }, (_, hour) => hour);
}
/** Minutes a `step` (seconds) allows; at least one minute apart. */
export function minuteValues(step) {
  const every = Number.isFinite(step) && step >= 60 ? Math.max(1, Math.round(step / 60)) : 1;
  return Array.from({ length: Math.ceil(60 / every) }, (_, index) => index * every).filter(minute => minute < 60);
}
/** Seconds a `step` under a minute allows, or null when the step is whole minutes. */
export function secondValues(step) {
  if (!Number.isFinite(step) || step <= 0 || step >= 60) return null;
  const every = Math.max(1, Math.round(step));
  return Array.from({ length: Math.ceil(60 / every) }, (_, index) => index * every).filter(second => second < 60);
}
export const timeIso = ({ hour, minute, second }) => `${pad(hour)}:${pad(minute)}${second === undefined ? '' : `:${pad(second)}`}`;

import { packageText } from '@lingxia/view';
import words from './words';

/** Liuli's word `key` in the page's language; an lxapp's catalogue overrides it by naming the key. */
export const liuliText = (key: `@liuli.${string}`, args?: Readonly<Record<string, unknown>>) => packageText(words, key, args);

const months = ['jan', 'feb', 'mar', 'apr', 'may', 'jun', 'jul', 'aug', 'sep', 'oct', 'nov', 'dec'] as const;
const weekdays = ['sun', 'mon', 'tue', 'wed', 'thu', 'fri', 'sat'] as const;

/** `month` from 1. */
export const monthName = (month: number) => liuliText('@liuli.monthName', { month: months[month - 1] });
/** `weekday` 0 for Sunday. */
export const weekdayName = (weekday: number) => liuliText('@liuli.weekdayName', { weekday: weekdays[weekday] });
export const weekdayInitial = (weekday: number) => liuliText('@liuli.weekdayInitial', { weekday: weekdays[weekday] });

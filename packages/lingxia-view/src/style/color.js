import { roles } from '../generated/contract.js';

const names = new Set(roles);

/** A role or `role/NN` (alpha percent 0-100), as the wire carries it. */
export function isRole(value) {
  const match = typeof value === 'string' && /^([a-z0-9-]+)(?:\/(\d{1,3}))?$/.exec(value);
  return Boolean(match) && names.has(match[1]) && (match[2] === undefined || Number(match[2]) <= 100);
}

/** Why a page colour is refused, or undefined for a role, `currentColor` or `transparent`. */
export function colorProblem(value, where = 'a page colour') {
  if (value === 'currentColor' || value === 'transparent' || /^#[0-9a-fA-F]{6}00$/.test(value) || isRole(value)) return undefined;
  return `UI_COLOR: ${JSON.stringify(value)} is not a role; ${where} uses a role such as 'surface', 'text-muted', 'line', 'accent' or 'accent/12'`;
}

/** Runtime admits what the wire carries: hex too, since the build already refused authored hex. */
export function wireColorProblem(value, where) {
  return /^#[0-9a-fA-F]{6}([0-9a-fA-F]{2})?$/.test(value) ? undefined : colorProblem(value, where);
}

/** Plot series take the categorical roles. */
export function seriesProblem(value) {
  return isRole(value) ? undefined : `UI_COLOR: ${JSON.stringify(value)} is not a role; plot series use 'chart-1' to 'chart-5'`;
}

export const defaultSeries = ['chart-1', 'chart-2', 'chart-3', 'chart-4', 'chart-5'];

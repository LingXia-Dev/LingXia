import { roles } from '../generated/contract.js';

const names = new Set(roles);

const alpha = percent => percent === undefined || Number(percent) <= 100;

/** A role or `role/NN` (alpha percent 0-100), as the wire carries it. */
export function isRole(value) {
  const match = typeof value === 'string' && /^([a-z0-9-]+)(?:\/(\d{1,3}))?$/.exec(value);
  return Boolean(match) && names.has(match[1]) && alpha(match[2]);
}

/** The name of an app colour, `app.<name>` or `app.<name>/NN`, or undefined. */
export function appColorName(value) {
  const match = typeof value === 'string' && /^app\.([a-z][a-z0-9]*(?:-[a-z0-9]+)*)(?:\/(\d{1,3}))?$/.exec(value);
  return match && alpha(match[2]) ? match[1] : undefined;
}

/** A role or an app colour; which app colours exist is the build's question. */
export const isColorName = value => isRole(value) || appColorName(value) !== undefined;

const appList = appColors => appColors?.size ? `; this app's colours: ${[...appColors].map(name => `app.${name}`).join(', ')}` : '';

/**
 * Why a page colour is refused, or undefined for a role, an app colour, `currentColor` or
 * `transparent`. With `appColors`, the names the app's theme declares, an `app.` name must be one.
 */
export function colorProblem(value, where = 'a page colour', appColors) {
  if (value === 'currentColor' || value === 'transparent' || /^#[0-9a-fA-F]{6}00$/.test(value) || isRole(value)) return undefined;
  const app = appColorName(value);
  if (app !== undefined) return !appColors || appColors.has(app) ? undefined : `UI_COLOR: ${JSON.stringify(value)} is not in this app's theme${appList(appColors)}`;
  return `UI_COLOR: ${JSON.stringify(value)} is not a role; ${where} uses a role such as 'surface', 'text-muted', 'line', 'accent' or 'accent/12'${appList(appColors)}`;
}

/** Runtime admits what the wire carries: hex too, since the build already refused authored hex. */
export function wireColorProblem(value, where) {
  return /^#[0-9a-fA-F]{6}([0-9a-fA-F]{2})?$/.test(value) ? undefined : colorProblem(value, where);
}

/** Plot series take the categorical roles or the app's own colours. */
export function seriesProblem(value, appColors) {
  if (isRole(value)) return undefined;
  const app = appColorName(value);
  if (app !== undefined && (!appColors || appColors.has(app))) return undefined;
  return `UI_COLOR: ${JSON.stringify(value)} is not a role; plot series use 'chart-1' to 'chart-8'${appList(appColors)}`;
}

export const defaultSeries = ['chart-1', 'chart-2', 'chart-3', 'chart-4', 'chart-5'];

// `useNow(granularity)`: the wall clock floored to a second or a minute. The host sets it before
// each render and ticks it only while a shown page reads it; a page that never reads it pays nothing.
import { render, page } from './state.js';
import { noteRead } from './memo.js';

const steps = { second: 1000, minute: 60000 };
const cells = Object.fromEntries(Object.entries(steps).map(([name, step]) => [name, { value: null, clockStep: step }]));
let now = null;

const floored = (ms, step) => Math.floor(ms / step) * step;
const current = cell => page.updates.has(cell) ? page.updates.get(cell) : cell.value;

/** A clock cell read in a render, its own or a recalled subtree's: the host ticks at its step. */
export function noteClock(cell) {
  if (cell.clockStep && !(render.clockStep > 0 && render.clockStep <= cell.clockStep)) render.clockStep = cell.clockStep;
}

export function useNow(granularity) {
  const cell = cells[granularity];
  if (!cell) throw Error('UI_PROP: useNow takes "second" or "minute"');
  if (!render.active) throw Error('UI_SCOPE: useNow is called during render');
  if (now === null) setNow(Date.now());
  noteRead(cell);
  noteClock(cell);
  return current(cell);
}

/** Sets the clock in epoch ms; true when a value some render shows moved to a new step. */
export function setNow(ms) {
  if (typeof ms !== 'number' || !Number.isFinite(ms)) return false;
  now = ms;
  let moved = false;
  for (const cell of Object.values(cells)) {
    const next = floored(ms, cell.clockStep);
    if (current(cell) === next) continue;
    // A step no render shows, or none yet, moves without asking for one.
    const shown = cell.value !== null && clockStep() > 0 && cell.clockStep >= clockStep();
    if (!shown) cell.value = next;
    else { page.updates.set(cell, next); moved = true; }
  }
  return moved;
}

/** The finest step the last render read, in ms; 0 when it read none. */
export const clockStep = () => render.clockStep ?? 0;

/** A commit adopts the clock's pending steps with the page's cells. */
export function commitClock() {
  for (const cell of Object.values(cells)) if (page.updates.has(cell)) cell.value = page.updates.get(cell);
}

export function resetClock() {
  now = null;
  for (const cell of Object.values(cells)) cell.value = null;
}

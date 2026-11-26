// The host's wake clock and Escape for a page's top layers.
import { closeTopmost } from './toplayer.js';

let timers = [], nextToken = 1, wakes = new Map();

/** The wake `token` came due; true when it repaints the page. */
export function overlayTimer(token) {
  const wake = wakes.get(token);
  if (!wake) return false;
  wakes.delete(token);
  return wake() === true;
}

/** Escape: the innermost layer goes, as its cancel would. True when one did. */
export function escapeOverlay() {
  return closeTopmost();
}

/** Run `fire` after `ms` on the host's clock; true from it repaints the page. */
export function requestWake(ms, fire) {
  const token = nextToken++;
  wakes.set(token, fire);
  timers.push([token, Math.round(ms)]);
}

export function takeOverlayTimers() { const result = timers; timers = []; return JSON.stringify(result); }

/** The page went away, and its wakes with it. */
export function resetOverlays() {
  timers = []; wakes = new Map();
}

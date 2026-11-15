// The slider a finger holds, with its latest `onPressChange`: a page has one slider gesture at a
// time, and each true gets exactly one false, even when the slider left the View meanwhile.
let held = null;

export function pressChange(identity, callback) {
  if (held?.identity === identity) return;
  held = { identity, callback };
  return callback(true);
}

/** The host saw the press end; false when no slider was held. */
export function releasePress() {
  if (!held) return false;
  const { callback } = held;
  held = null;
  const result = callback(false);
  if (result && typeof result.then === 'function') Promise.resolve(result).catch(() => {});
  return true;
}

/** After a commit: a held slider still in the View answers with its current callback. */
export function refreshPresses(live) {
  if (held && live.has(held.identity)) held.callback = live.get(held.identity);
}

// Sliders under a press, each with the last value it changed to: `onChangeEnd` waits for the release.
let waiting = new Map();

/** A slider changed to `value`: its change ends now, or at the release of the press that holds it. */
export function changeEnd(identity, value, end) {
  if (waiting.has(identity)) waiting.set(identity, { value });
  else end(value);
}

/** A slider's press began or ended; a release ends the change made under it. */
export function pressEnd(identity, pressed, end) {
  if (pressed) { waiting.set(identity, null); return; }
  const last = waiting.get(identity);
  waiting.delete(identity);
  if (last) end(last.value);
}

export const resetPresses = () => { held = null; waiting = new Map(); };

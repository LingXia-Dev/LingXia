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

export const resetPresses = () => { held = null; };

// Eval phase totals in µs, off unless a dev session's clock is opted in with
// `__lingxiaClock.phases = true`, so ordinary dev runs pay one property read per span.
let spans = null;

function clock() {
  const read = globalThis.__lingxiaClock;
  return read && read.phases === true ? read : null;
}

export function now() {
  const read = clock();
  if (!read) return 0;
  spans ??= {};
  return read();
}

export function spent(name, since) {
  if (spans && since) spans[name] = (spans[name] ?? 0) + globalThis.__lingxiaClock() - since;
}

export function counted(name) {
  if (spans) spans[name] = (spans[name] ?? 0) + 1;
}

/** Totals since the last call, as JSON; '' when phases are off. */
export function takeSpans() {
  if (!spans) return '';
  for (const name in spans) spans[name] = Math.round(spans[name]);
  const out = JSON.stringify(spans);
  spans = {};
  return out;
}

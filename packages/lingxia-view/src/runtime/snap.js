// A controlled snap waits for the page's answer. After a settle the page has not taken up, the
// element holds the settled index until the page's index moves (taken, or the page chose another)
// or its decision window closes with the index unchanged; then the page's index is sent once and
// native springs back. The window spans the actions the `onSnapChange` handler started and the
// first data snapshot after they settle, never past CEILING_MS.
import { render, page } from './state.js';
import { actionSerial, actionPending } from './actions.js';
import { requestWake } from './overlay.js';

// About five times the Pixel 3's p95 action round trip (82 ms, PERF-BRIDGE-002).
export const CEILING_MS = 400;

let settled = new Map();

export function resetSnaps() { settled = new Map(); }

const clock = () => Date.now();

// The index to send this frame for a controlled element whose page state says `index`.
export function controlledSnap(key, index) {
  if (settled.size === 0) return index;
  const record = settled.get(key);
  if (!record) return index;
  // Taken, or the page chose another index: that is the answer.
  if (index === record.index || index !== record.from) { settled.delete(key); return index; }
  if (!record.sent) {
    record.sent = true;
    if (!open(record)) render.candidate.invalidateAfterCommit = true;
    return record.index;
  }
  if (open(record)) return record.index;
  settled.delete(key);
  return index;
}

function open(record) {
  if (record.ids.length === 0 || record.rejected || clock() >= record.deadline) return false;
  if (record.ids.some(actionPending)) return true;
  // Settled: Logic's answer is the first data snapshot after the last of them.
  return render.view?.data === record.data;
}

// The element came to rest at `index` while the page holds `current`; `heard` runs the page's
// `onSnapChange`, whose actions extend the window.
export function snapSettled(key, index, current, heard) {
  if (index === current) { settled.delete(key); return; }
  const first = actionSerial();
  try { heard?.(); } finally {
    const ids = [];
    for (let serial = first; serial < actionSerial(); serial++) ids.push(serial);
    const record = { index, from: current, ids, rejected: false, sent: false, data: page.committed?.view?.data, deadline: clock() + CEILING_MS };
    settled.set(key, record);
    page.invalidated = true;
    if (ids.length) requestWake(CEILING_MS, () => {
      if (settled.get(key) !== record) return false;
      page.invalidated = true;
      return true;
    });
  }
}

// A Logic reply settled action `serial`: a waiting element looks again.
export function snapActionSettled(serial, ok) {
  if (settled.size === 0) return;
  for (const record of settled.values()) {
    if (!record.ids.includes(serial)) continue;
    if (!ok) record.rejected = true;
    // The answer is the first snapshot after the last reply, not any since the swipe.
    if (!record.ids.some(actionPending)) record.data = page.committed?.view?.data;
    page.invalidated = true;
  }
}

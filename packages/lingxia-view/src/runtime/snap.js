// A controlled snap index the page refused. Native springs only when the index it is sent
// changes, so after a settle the page did not take, one frame echoes the settled index and the
// next sends the page's own, which springs the element back. Once per settle.
import { render, page } from './state.js';

let settled = new Map();

export function resetSnaps() { settled = new Map(); }

// The index to send this frame for a controlled element whose page state says `index`.
export function controlledSnap(key, index) {
  const record = settled.get(key);
  if (!record || record.index === index) { settled.delete(key); return index; }
  if (!record.echoed) { record.echoed = true; render.candidate.invalidateAfterCommit = true; return record.index; }
  settled.delete(key);
  return index;
}

// The element came to rest at `index` while the page holds `current`.
export function snapSettled(key, index, current) {
  if (index === current) return;
  settled.set(key, { index, echoed: false });
  page.invalidated = true;
}

// Dev builds only: a new page module takes over the committed local state of the
// one it replaces, so an edit keeps what the page was showing.
import { page } from './state.js';
import { immutable } from './data.js';

function exportState() {
  const locals = [];
  // Resource cells (three-part keys) restart with their resource; plain locals carry over.
  // A set not yet committed by a frame is the newest state, so it travels too.
  for (const [key, cell] of page.locals)
    if (JSON.parse(key).length === 2) locals.push([key, page.updates.has(cell) ? page.updates.get(cell) : cell.value]);
  return JSON.stringify(locals);
}

function importState(text) {
  for (const [key, value] of JSON.parse(text)) page.locals.set(key, { value: immutable(value, 0, true) });
}

export function enableHotSwap() {
  Object.assign(globalThis, { __lingxiaExportState: exportState, __lingxiaImportState: importState });
}

import { render, page, outsideRender } from './state.js';
import { h, resetArena, clearArena } from './nodes.js';
import { resolveNode, listTag } from './elements.js';
import { encodeTree, copyTree, retainCells, renderRow } from './encode.js';
import { validInputText } from './data.js';
import { report, resetActions, actionEpochNow, hasActionEpoch } from './actions.js';
import { sited } from './errors.js';
import { overlayNode, decorateOverlay, resetOverlays } from './overlay.js';
import { envOf } from './hooks.js';

// Candidate transactions: prepare renders and encodes without touching the
// committed page; commit adopts the candidate, discard drops it.
export function List({ items, getKey, renderItem, getItemHeight, ...props }) {
  return h(listTag, { ...props, count: items.length,
    itemKeys: items.map(getKey), row: index => renderItem(items[index], index),
    ...(getItemHeight ? { height: index => getItemHeight(items[index], index) } : {}) });
}
List.__lxid = '@lingxia/ui#List';

function runCandidate(view, lookup, build) {
  if (render.active) throw Error('UI_RENDER: recursive render');
  resetArena();
  Object.assign(render, { active: true, view, lookupLocals: lookup });
  render.candidate = { locals: new Map(), declarations: new Set(), callbacks: new Map(), lists: new Map(), resources: new Map(), resourceNames: new Set(), view };
  try { render.scope = 'root'; return build(render.candidate); }
  // A render that throws reverts to the committed page: every pending update is dropped,
  // not only the one that threw, so state that makes render fail cannot wedge each retry.
  catch (error) { discard(); throw error; }
  finally { clearArena(); Object.assign(render, { active: false, scope: null, view: null, lookupLocals: null, collectors: [] }); }
}

export function prepare(renderPage, view, windows = {}) {
  return runCandidate(view, null, candidate => {
    candidate.tree = resolveNode(renderPage(view), 'root');
    render.site = null;
    if (!candidate.tree || candidate.tree.fragment) throw Error('UI_ROOT: page must return one layout node');
    // Logic-requested feedback (toast, modal, action sheet) draws above the page's own children.
    const env = envOf(view), overlay = overlayNode(env);
    if (overlay) candidate.tree.children.push(decorateOverlay(resolveNode(overlay, `root/${candidate.tree.element}/key:"lx-overlay"`, 1,
      { tag: candidate.tree.element, typography: candidate.tree.typography }), env));
    candidate.baseLocals = new Map(candidate.locals);
    return encodeTree(candidate.tree, windows);
  });
}

// Native may refine an uncommitted first window after measuring it. The page render
// and ordinary components remain untouched; a failed candidate never replaces committed rows.
export function prepareWindows(windows) {
  const base = render.candidate ?? page.committed;
  if (!base) throw Error('UI_LIST: no retained description');
  return runCandidate(base.view, base.locals, candidate => {
    candidate.resources = new Map(base.resources);
    candidate.tree = copyTree(base.tree);
    candidate.baseLocals = new Map(base.baseLocals);
    retainCells(candidate.baseLocals);
    return encodeTree(candidate.tree, windows);
  });
}

export function commit() {
  const { candidate } = render;
  if (!candidate) throw Error('UI_COMMIT: no candidate');
  const previous = page.committed;
  for (const cell of candidate.locals.values()) if (page.updates.has(cell)) cell.value = page.updates.get(cell);
  page.locals = candidate.locals; page.callbacks = candidate.callbacks;
  page.committed = candidate; render.candidate = null; page.updates.clear();
  const retained = new Set(page.locals.values()), next = new Map(), definitions = new Map();
  for (const [cell, resource] of page.resources) if (!retained.has(cell)) resource.dispose();
  for (const cell of retained) {
    const entry = candidate.resources.get(cell) ?? previous?.resources.get(cell);
    if (!entry) continue;
    definitions.set(cell, entry);
    next.set(cell, entry.resource);
    entry.resource.commit(entry.definition);
  }
  candidate.resources = definitions;
  page.resources = next;
}

// Native drops the candidate. A frame the target merely declined keeps pending updates
// (the user's taps survive it); any other candidate failure reverts them.
// Automation: which off-screen row of a committed list would hold `testId`. Rows outside the
// window are rendered into a throwaway candidate and searched; nothing is committed.
const probeRows = 2000;
export function probeListRow(testId) {
  const base = page.committed;
  if (!base || render.active || typeof testId !== 'string') return 'null';
  const lists = [];
  const walk = (node, depth) => {
    if (!node || depth > 256) return;
    if (node.list && typeof node.props?.row === 'function') lists.push([node, depth]);
    for (const child of node.children ?? []) walk(child, depth + 1);
  };
  walk(base.tree, 0);
  const holds = node => node?.props?.testId === testId || (node?.children ?? []).some(holds);
  const saved = render.candidate;
  try {
    return JSON.stringify(runCandidate(base.view, base.locals, () => {
      for (const [node, depth] of lists) {
        const { window } = node.list;
        for (let i = 0; i < Math.min(node.props.count, probeRows); i++) {
          if (window && i >= window.start && i < window.end) continue;
          const cells = new Map(); render.collectors.push(cells);
          try { if (holds(renderRow(node, i, cells, depth).row)) return { list: node.identity, index: i }; }
          catch { /* a row that cannot render here is not the match */ }
          finally { render.collectors.pop(); }
        }
      }
      return null;
    }));
  } catch { return 'null'; }
  finally { render.candidate = saved; }
}

export function discard(keepUpdates = false) {
  render.candidate = null;
  if (!keepUpdates) page.updates.clear();
}

export function reset() {
  outsideRender('reset');
  discard(); page.updates.clear(); page.committed = null; page.locals.clear(); page.callbacks.clear();
  for (const resource of page.resources.values()) resource.dispose();
  page.resources.clear();
  resetActions();
  resetOverlays();
}

const accepts = (kind, value) => kind === 'number' ? typeof value === 'number' && Number.isFinite(value) && Math.abs(value) <= 1e9
  : kind === 'text' ? validInputText(value) : value === undefined;

export function dispatch(handle, value) {
  outsideRender('dispatch');
  const entry = page.callbacks.get(handle);
  if (!entry || !accepts(entry.kind, value)) return false;
  const previous = entry.isolated ? new Map(page.updates) : undefined;
  try {
    const epoch = actionEpochNow();
    const result = entry.kind === 'void' ? entry.callback() : entry.callback(value);
    if (result && typeof result.then === 'function') Promise.resolve(result).catch(error => { if (hasActionEpoch(epoch)) report(error); });
    return true;
  } catch (thrown) {
    const error = sited(thrown);
    page.updates.clear();
    if (!entry.isolated) throw error;
    for (const [cell, kept] of previous) page.updates.set(cell, kept);
    report(error);
    return true;
  }
}

export function describe(renderPage, view) {
  const output = prepare(renderPage, view); commit(); return output;
}

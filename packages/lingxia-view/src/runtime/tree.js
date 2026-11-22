import { resetPresses, refreshPresses } from './press.js';
import { render, page, outsideRender } from './state.js';
import { h, resetArena, clearArena } from './nodes.js';
import { resolveNode, listTag } from './elements.js';
import { encodeTree, copyTree, retainCells, renderRow, wireReset } from './encode.js';
import { validInputText } from './data.js';
import { report, resetActions, actionEpochNow, hasActionEpoch } from './actions.js';
import { sited } from './errors.js';
import { overlayNode, decorateOverlay, resetOverlays } from './overlay.js';
import { resetFrameMeasures } from './frames.js';
import { resetReveals } from './reveal.js';
import { pickerNode, pickerAnchor, resetPicker } from './picker.js';
import { envOf, resetEnvReads } from './hooks.js';
import { keysOf } from './keys.js';
import { finishTopLayer, resetTopLayer } from './toplayer.js';
import { resetSnaps } from './snap.js';
import { fullscreenFallback, fullscreenDrawn, resolveBare } from './fullscreen.js';
import { resetVideos } from './video.js';
import { webFullscreenFallback, webFullscreenDrawn, resolveBareWeb, resetWebViews } from './webview.js';
import { now, spent } from './profile.js';

// Candidate transactions: prepare renders and encodes without touching the
// committed page; commit adopts the candidate, discard drops it.
export function List({ items, getKey, renderItem, getItemHeight, ...props }) {
  if (!Object.isFrozen(items) && typeof globalThis.__lingxiaWarn === 'function') warnUnfrozen(props, renderItem);
  const itemKeys = keysOf(items, getKey);
  const row = index => renderItem(items[index], index);
  // The same renderItem over the same frozen items builds the same rows, save those that read state.
  if (typeof renderItem === 'function' && !getItemHeight && Object.isFrozen(items)) row.__lxRows = { items, site: renderItem };
  return h(listTag, { ...props, count: items.length,
    itemKeys, row,
    ...(getItemHeight ? { height: index => getItemHeight(items[index], index) } : {}) });
}
List.__lxid = '@lingxia/view#List';

// Dev sessions only: rows are reused only over a frozen array, so say so once per List.
const warnedLists = new Set();
function warnUnfrozen(props, renderItem) {
  const site = props['data-testid'] ?? String(renderItem).slice(0, 120);
  if (warnedLists.has(site)) return;
  warnedLists.add(site);
  const name = props['data-testid'] ? ` "${props['data-testid']}"` : '';
  globalThis.__lingxiaWarn(`UI_LIST: <List>${name} got an items array that is not frozen, so every row renders again on every render; ` +
    'Object.freeze the array the View builds, or pass the Logic data array through unchanged');
}

function runCandidate(view, lookup, build, recall = null) {
  if (render.active) throw Error('UI_RENDER: recursive render');
  render.thrown = '';
  resetArena();
  Object.assign(render, { active: true, view, lookupLocals: lookup, recall: recall?.memo, recallClosures: recall?.closures });
  render.candidate = { locals: new Map(), declarations: new Set(), callbacks: new Map(), lists: new Map(), resources: new Map(), resourceNames: new Set(), view,
    memo: recall && new Map(), closures: recall && new Map(), closureCounts: new Map() };
  try { render.scope = 'root'; return build(render.candidate); }
  // A render that throws reverts to the committed page: every pending update is dropped,
  // not only the one that threw, so state that makes render fail cannot wedge each retry.
  // Recorded first: an interrupt cannot be caught, so this is the render's own error even if
  // native finds the time limit passed while it unwound.
  catch (error) { render.thrown = error instanceof Error ? error.message : 'JavaScript threw a value'; discard(); throw error; }
  finally { clearArena(); Object.assign(render, { active: false, scope: null, view: null, lookupLocals: null, collectors: [], recall: null, recallClosures: null, frames: [], site: null }); }
}

// With `deferAnchors`, a render whose list anchors moved returns no buffer: native picks their
// destination windows first (`takeMovedAnchors`), then encodes once through `prepareWindows`.
let movedAnchors = '';
/** The error the last render threw itself, once. */
export function takeThrown() { const thrown = render.thrown; render.thrown = ''; return thrown; }

export function takeMovedAnchors() { const moved = movedAnchors; movedAnchors = ''; return moved; }

export function prepare(renderPage, view, windows = {}, deferAnchors = false) {
  return runCandidate(view, null, candidate => {
    if (deferAnchors) candidate.movedAnchors = new Map();
    let since = now();
    const root = renderPage(view);
    spent('render', since);
    since = now();
    candidate.tree = resolveNode(root, 'root');
    spent('resolve', since);
    render.site = null;
    if (!candidate.tree || candidate.tree.fragment) throw Error('UI_ROOT: page must return one layout node');
    if (candidate.tree.lifted) throw Error('UI_ROOT: a <text> page root has no place for a popover it holds; wrap it in <view>');
    // The root may be a recalled subtree the committed page shares; layers go into a copy.
    candidate.tree = { ...candidate.tree, children: [...candidate.tree.children] };
    // Logic-requested feedback (toast, modal, action sheet) draws above the page's own children.
    const env = envOf(view), overlay = overlayNode(env);
    if (overlay) candidate.tree.children.push(decorateOverlay(resolveNode(overlay, `root/${candidate.tree.element}/key:"lx-overlay"`, 1,
      { tag: candidate.tree.element, typography: candidate.tree.typography }), env));
    // A player shown full screen that the page draws no dialog for: the bare picture, in the top layer.
    const fallback = fullscreenFallback();
    const bare = fallback && resolveBare(() => resolveNode(fallback, `root/${candidate.tree.element}/key:"lx-fullscreen"`, 1,
      { tag: candidate.tree.element, typography: candidate.tree.typography }));
    if (bare) candidate.tree.children.push(bare);
    const webFallback = webFullscreenFallback();
    const bareWeb = webFallback && resolveBareWeb(() => resolveNode(webFallback, `root/${candidate.tree.element}/key:"lx-web-fullscreen"`, 1,
      { tag: candidate.tree.element, typography: candidate.tree.typography }));
    if (bareWeb) candidate.tree.children.push(bareWeb);
    // A date or time editor's picker draws above everything else on the page.
    const picker = pickerNode(env);
    if (picker) {
      const layer = resolveNode(picker, `root/${candidate.tree.element}/key:"lx-picker-layer"`, 1,
        { tag: candidate.tree.element, typography: candidate.tree.typography });
      // A popover sits against its editor: native places it from the editor's laid-out box.
      const anchor = pickerAnchor(env);
      const panel = layer.children.find(child => child.props?.testId === 'lx-picker');
      if (anchor && panel) panel.wire.push(['anchorTo', anchor]);
      candidate.tree.children.push(layer);
    }
    finishTopLayer(candidate.tree);
    candidate.baseLocals = new Map(candidate.locals);
    if (candidate.movedAnchors?.size) {
      movedAnchors = JSON.stringify(Object.fromEntries(candidate.movedAnchors));
      return [];
    }
    const encoding = now();
    try {
      const wire = encodeTree(candidate.tree, windows);
      // List rows render while encoding; one that draws the full screen dialog retires the bare one.
      const retireVideo = bare && fullscreenDrawn();
      const retireWeb = bareWeb && webFullscreenDrawn();
      if (!retireVideo && !retireWeb) return wire;
      candidate.tree.children = candidate.tree.children.filter(child =>
        (!retireVideo || child !== bare) && (!retireWeb || child !== bareWeb));
      return encodeTree(candidate.tree, windows);
    } finally { spent('encode', encoding); }
  }, page.committed ?? { memo: new Map(), closures: new Map() });
}

// Native may refine an uncommitted first window after measuring it. The page render
// and ordinary components remain untouched; a failed candidate never replaces committed rows.
export function prepareWindows(windows) {
  const base = render.candidate ?? page.committed;
  if (!base) throw Error('UI_LIST: no retained description');
  return runCandidate(base.view, base.locals, candidate => {
    candidate.resources = new Map(base.resources);
    candidate.topLayer = base.topLayer;
    candidate.fullscreenHandlers = base.fullscreenHandlers;
    candidate.webFullscreenHandlers = base.webFullscreenHandlers;
    candidate.webSources = base.webSources && new Map(base.webSources);
    candidate.tree = copyTree(base.tree);
    candidate.memo = new Map(base.memo);
    candidate.closures = new Map(base.closures);
    candidate.baseLocals = new Map(base.baseLocals);
    retainCells(candidate.baseLocals);
    const encoding = now();
    try { return encodeTree(candidate.tree, windows); } finally { spent('windows', encoding); }
  }, base);
}

export function commit() {
  const since = now();
  try { commitCandidate(); } finally { spent('commit', since); }
}

function commitCandidate() {
  const { candidate } = render;
  if (!candidate) throw Error('UI_COMMIT: no candidate');
  const previous = page.committed;
  for (const cell of candidate.locals.values()) if (page.updates.has(cell)) cell.value = page.updates.get(cell);
  page.locals = candidate.locals; page.callbacks = candidate.callbacks;
  page.committed = candidate; render.candidate = null; page.updates.clear(); page.invalidated = candidate.invalidateAfterCommit === true;
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
  const pressable = new Map();
  for (const entry of candidate.callbacks.values()) if (entry.press) pressable.set(entry.press.identity, entry.press.callback);
  refreshPresses(pressable);
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
  discard(); page.updates.clear(); page.committed = null; page.locals.clear(); page.callbacks.clear(); resetPresses();
  for (const resource of page.resources.values()) resource.dispose();
  page.resources.clear();
  resetActions();
  resetOverlays(); resetPicker(); resetTopLayer(); resetFrameMeasures(); resetReveals(); resetEnvReads(); resetVideos(); resetWebViews(); resetSnaps();
  wireReset();
}

const accepts = (kind, value) => kind === 'number' ? typeof value === 'number' && Number.isFinite(value) && Math.abs(value) <= 1e9
  : kind === 'text' ? validInputText(value)
  : kind === 'check' ? value === undefined || typeof value === 'boolean' : value === undefined;

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

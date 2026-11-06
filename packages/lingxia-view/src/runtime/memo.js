// A component re-runs only when something it was built from changed: its props, its place, the
// cells it declared or read, or the environment. Otherwise the subtree it resolved last time stands.
import { render, page } from './state.js';
import { brand, currentArena } from './nodes.js';
import { envOf } from './hooks.js';
import { handlesIssued, skipHandle } from './elements.js';
import { sliceable } from './encode.js';

// Comparing two prop sets stops after this many objects; past it they count as different.
const compareBudget = 256;

const stamps = new WeakMap();
// The parts of the view every resolution reads besides props: environment and catalog.
function viewStamp(view) {
  let stamp = stamps.get(view);
  if (!stamp) stamps.set(view, stamp = { env: JSON.stringify(envOf(view) ?? null), actions: view?.actions, channels: view?.channels });
  return stamp;
}
const sameView = (a, b) => a.env === b.env && a.actions === b.actions && a.channels === b.channels;

function same(a, b, old, budget) {
  if (Object.is(a, b)) return true;
  if (typeof a !== 'object' || typeof b !== 'object' || a === null || b === null || --budget.left < 0) return false;
  const nodeA = brand in a, nodeB = brand in b;
  if (nodeA || nodeB) {
    if (!nodeA || !nodeB) return false;
    const x = currentArena()[a[brand][1]], y = old[b[brand][1]];
    return Boolean(x && y) && x[0] === y[0] && sameFields(x[1], y[1], old, budget) && same(x[2], y[2], old, budget);
  }
  if (Array.isArray(a)) {
    if (!Array.isArray(b) || a.length !== b.length) return false;
    for (let i = 0; i < a.length; i++) if (!same(a[i], b[i], old, budget)) return false;
    return true;
  }
  if (Object.getPrototypeOf(a) !== Object.prototype || Object.getPrototypeOf(b) !== Object.prototype) return false;
  return sameFields(a, b, old, budget);
}

function sameFields(a, b, old, budget) {
  let count = 0;
  for (const key in a) {
    count++;
    if (!Object.prototype.hasOwnProperty.call(b, key) || !same(a[key], b[key], old, budget)) return false;
  }
  for (const key in b) count--;
  return count === 0;
}

const contextKeys = ['tag', 'direction', 'inControl', 'inText', 'inLink'];
function sameContext(a, b, old) {
  for (let i = 0; i < contextKeys.length; i++) if (a[contextKeys[i]] !== b[contextKeys[i]]) return false;
  const budget = { left: compareBudget };
  return same(a.typography, b.typography, old, budget) && same(a.extra, b.extra, old, budget) &&
    same(a.sibling, b.sibling, old, budget) && same(a.popovers, b.popovers, old, budget);
}

const topLayerSize = () => {
  const top = render.candidate.topLayer;
  return top ? top.entries.length + top.popovers.size + top.invokers.length : 0;
};

/** The subtree `scope` resolved last time, when nothing it was built from changed; else undefined. */
export function recall(scope, tag, props, children, parent) {
  const record = render.recall?.get(scope);
  if (!record || record.tag !== tag || tag.__lxVolatile || !sameView(record.view, viewStamp(render.view))) return undefined;
  for (const cell of record.reads) if (page.updates.has(cell)) return undefined;
  for (const cell of record.cells.values()) if (page.updates.has(cell)) return undefined;
  const budget = { left: compareBudget };
  if (!sameFields(props, record.props, record.arena, budget) || !same(children, record.children, record.arena, budget) ||
      !sameContext(parent, record.parent, record.arena)) return undefined;
  // A list's copy is what this render encodes and commits, so the next recall starts from it.
  const fresh = { ...record, props, children, parent, arena: currentArena(), result: record.lists ? reuse(record.result) : record.result };
  keep(fresh);
  const frame = render.frames[render.frames.length - 1];
  if (frame) { frame.nested.push(fresh); for (const cell of record.reads) frame.reads.add(cell); }
  for (const [key, cell] of record.cells) {
    render.candidate.locals.set(key, cell);
    for (const collector of render.collectors) collector.set(key, cell);
  }
  // Enclosing rows and components see the callbacks and lists this subtree holds.
  if (record.handles) skipHandle();
  if (record.lists) render.lists++;
  return fresh.result;
}

// Encoding writes list state into the node, so a reused list gets its own copy, keeping only the
// rows that read nothing but their item.
function reuse(node) {
  let children = node.children;
  for (let i = 0; i < children.length; i++) {
    const child = reuse(node.children[i]);
    if (child === node.children[i]) continue;
    if (children === node.children) children = children.slice();
    children[i] = child;
  }
  if (!node.list) return children === node.children ? node : { ...node, children };
  const { pure } = node.list, cache = new Map();
  for (const [i, row] of node.list.cache) if (pure.has(i)) cache.set(i, row);
  return { ...node, children, list: { ...node.list, states: new Map(node.list.states), cache, pure: new Set(cache.keys()),
    reusable: null, anchorChanged: false, anchorCache: null } };
}

function keep(record) {
  render.candidate.memo.set(record.scope, record);
  for (const [key, entry] of record.closures) render.candidate.closures?.set(key, entry);
  for (const nested of record.nested) keep(nested);
}

/** Starts tracking what the component at `scope` builds from. */
export function enter(scope) {
  const frame = { scope, cells: new Map(), reads: new Set(), view: false, nested: [], closures: [], handles: handlesIssued(), top: topLayerSize(), lists: render.lists };
  render.frames.push(frame);
  render.collectors.push(frame.cells);
  return frame;
}

/** A cell's value read during render: whoever read it re-runs when it changes. */
export function noteRead(cell) {
  render.cellReads++;
  const frame = render.frames[render.frames.length - 1];
  if (frame) frame.reads.add(cell);
}

/** The View read during render: the reading subtrees re-run on every render. */
export function noteView() {
  const frame = render.frames[render.frames.length - 1];
  if (frame) frame.view = true;
}

/** Ends `frame`, also when its component threw; enclosing frames take what it read. */
export function leave(frame) {
  render.frames.splice(render.frames.lastIndexOf(frame), 1);
  render.collectors.splice(render.collectors.lastIndexOf(frame.cells), 1);
  const outer = render.frames[render.frames.length - 1];
  if (outer) { for (const cell of frame.reads) outer.reads.add(cell); outer.view ||= frame.view; }
}

/** Remembers the subtree `frame` resolved, when it can stand in for a later render. */
export function remember(frame, tag, props, children, parent, result) {
  if (frame.view || result?.fragment || topLayerSize() !== frame.top || !render.candidate.memo) return;
  const record = { scope: frame.scope, tag, props, children, parent, arena: currentArena(), result,
    cells: frame.cells, reads: frame.reads, nested: frame.nested, closures: frame.closures, view: viewStamp(render.view),
    handles: handlesIssued() !== frame.handles, lists: render.lists !== frame.lists };
  render.candidate.memo.set(frame.scope, record);
  render.frames[render.frames.length - 1]?.nested.push(record);
  if (result && !record.lists) sliceable.add(result);
}

/**
 * A closure made during render (wrapped by the compiler): the function an earlier render made at
 * the same place, while everything it captured holds the same values; else `fn`.
 */
export function stableClosure(site, fn, captured) {
  const { candidate } = render;
  if (!render.active || !candidate?.closures) return fn;
  const base = `${render.scope}\u0000${site}`;
  const seen = candidate.closureCounts.get(base) ?? 0;
  candidate.closureCounts.set(base, seen + 1);
  const key = seen ? `${base}\u0000${seen}` : base;
  const old = render.recallClosures?.get(key);
  const entry = old && old.captured.length === captured.length && old.captured.every((value, i) => Object.is(value, captured[i]))
    ? old : { fn, captured };
  candidate.closures.set(key, entry);
  render.frames[render.frames.length - 1]?.closures.push([key, entry]);
  return entry.fn;
}

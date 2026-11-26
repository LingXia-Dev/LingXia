import { applyPatch, immutable, shared } from './data.js';
import { render, page, outsideRender } from './state.js';
import { actionInfo, currentActions, leaveGuards, report } from './actions.js';
import { actionResource, streamResource, requestResource } from './resources.js';
import { channelResource } from './channel-resource.js';
import { noteRead, noteView, noteContext, noteEnv } from './memo.js';
import { classesReadTheme, classesReadSizeClass } from '../style/classes.js';
import { createContext, lookup } from './context.js';
import { clockStep } from './clock.js';
import { h } from './nodes.js';

const SIZE_CLASS_CONTEXT_ID = '@lingxia/view:SizeClassScope';
const SizeClassContext = createContext(null);
Object.defineProperty(SizeClassContext, '__lxid', { value: SIZE_CLASS_CONTEXT_ID });

/** Overrides `env.sizeClass` and `regular:` class matching for its subtree. */
export function SizeClassScope({ value, children }) {
  if (value !== 'compact' && value !== 'regular')
    throw Error('UI_PROP: SizeClassScope value must be "compact" or "regular"');
  return h(SizeClassContext.Provider, { value }, children);
}

/** Active `SizeClassScope` value in the current render context chain, or undefined. */
export function scopedSizeClass(contexts = render.contexts, track = true) {
  const entry = lookup(contexts, SIZE_CLASS_CONTEXT_ID);
  if (!entry) return undefined;
  if (track) noteContext(SIZE_CLASS_CONTEXT_ID, SizeClassContext, entry.value);
  return entry.value;
}

const bindings = new WeakMap();

/** The cell behind a `useLocal` binding, if it is one. */
export function cellOf(binding) { return bindings.get(binding); }

export function local(name, initial, resource = '', order) {
  if (!render.active || render.scope === null || (order === undefined && (typeof name !== 'string' || !name || name.length > 256)))
    throw Error('UI_SCOPE: invalid local state');
  const { candidate, scope } = render;
  // An unnamed local is keyed by its call order in the component, as React's hooks are.
  const key = JSON.stringify(order !== undefined ? [scope, order] : resource ? [scope, resource, name] : [scope, name]);
  if (!resource && candidate.declarations.has(key)) throw Error(`UI_LOCAL: duplicate declaration ${JSON.stringify(name)} in ${scope}`);
  candidate.declarations.add(key);
  const cell = candidate.locals.get(key) ?? render.lookupLocals?.get(key) ?? page.locals.get(key) ?? { value: immutable(initial, 0, true) };
  candidate.locals.set(key, cell);
  for (const collector of render.collectors) collector.set(key, cell);
  // One binding per cell, so a closure that captured it stays equal across renders.
  if (cell.binding) return cell.binding;
  const binding = cell.binding = Object.freeze({
    get value() {
      if (render.active) noteRead(cell);
      return page.updates.has(cell) ? page.updates.get(cell) : cell.value;
    },
    set(value) {
      outsideRender('state mutation');
      // A cell of the prepared candidate takes the value when that candidate commits.
      if (page.locals.get(key) !== cell && render.candidate?.locals.get(key) !== cell)
        throw Error(`UI_LOCAL: ${JSON.stringify(name)} in ${scope} is no longer mounted`);
      if (typeof value === 'function') value = value(page.updates.has(cell) ? page.updates.get(cell) : cell.value);
      page.updates.set(cell, immutable(value, 0, true));
    },
    focus() {
      outsideRender('focus');
      if (page.locals.get(key) !== cell) throw Error(`UI_LOCAL: ${JSON.stringify(name)} in ${scope} is no longer mounted`);
      // The bound editor sends the bumped request; re-rendering with the current value carries it.
      cell.focus = (cell.focus ?? 0) + 1;
      page.updates.set(cell, page.updates.has(cell) ? page.updates.get(cell) : cell.value);
    },
    showPicker() {
      outsideRender('showPicker');
      if (page.locals.get(key) !== cell) throw Error(`UI_LOCAL: ${JSON.stringify(name)} in ${scope} is no longer mounted`);
      cell.picker = (cell.picker ?? 0) + 1;
      page.updates.set(cell, page.updates.has(cell) ? page.updates.get(cell) : cell.value);
    },
    setSelection(start, end) {
      outsideRender('setSelection');
      const value = editorValue(key, cell, name, scope, 'setSelection');
      selectionRange(value, start, end, 'setSelection');
      cell.selection = [(cell.selection?.[0] ?? 0) + 1, start, end];
      page.updates.set(cell, value);
    },
    replaceRange(start, end, text) {
      outsideRender('replaceRange');
      const value = editorValue(key, cell, name, scope, 'replaceRange');
      selectionRange(value, start, end, 'replaceRange');
      if (typeof text !== 'string') throw Error('UI_LOCAL: replaceRange takes (start, end, text) with text a string');
      const at = start + text.length;
      cell.selection = [(cell.selection?.[0] ?? 0) + 1, at, at];
      page.updates.set(cell, immutable(value.slice(0, start) + text + value.slice(end), 0, true));
    },
  });
  bindings.set(binding, { key, cell });
  return binding;
}

// A text cell's latest value, for the selection calls on the editor bound to it.
function editorValue(key, cell, name, scope, call) {
  if (page.locals.get(key) !== cell) throw Error(`UI_LOCAL: ${JSON.stringify(name)} in ${scope} is no longer mounted`);
  const value = page.updates.has(cell) ? page.updates.get(cell) : cell.value;
  if (typeof value !== 'string') throw Error(`UI_LOCAL: ${call} belongs to a text cell bound to an editor`);
  return value;
}

// UTF-16 offsets that keep a surrogate pair whole, as the editor's own selection does.
function selectionRange(value, start, end, call) {
  const splits = at => at > 0 && at < value.length && /[\uD800-\uDBFF]/.test(value[at - 1]) && /[\uDC00-\uDFFF]/.test(value[at]);
  if (!Number.isInteger(start) || !Number.isInteger(end) || start < 0 || start > end || end > value.length || splits(start) || splits(end))
    throw Error(`UI_LOCAL: ${call} takes UTF-16 offsets 0 <= start <= end <= ${value.length} that split no character; got ${start}, ${end}`);
}

/** `useLocal(initial)` keys the cell by call order; `useLocal(name, initial)` by name, which keeps it across reorders and edits. */
export function useLocal(...args) {
  if (args.length === 2) return local(args[0], args[1]);
  if (args.length !== 1) throw Error('UI_LOCAL: useLocal takes (initial) or (name, initial)');
  if (!render.active || render.scope === null) throw Error('UI_SCOPE: invalid local state');
  const counts = render.candidate.localOrder ??= new Map();
  const order = counts.get(render.scope) ?? 0;
  counts.set(render.scope, order + 1);
  return local(`#${order}`, args[0], '', order);
}
// Two 32-bit FNV-1a passes with different seeds: an id collision would fail the page with UI_ID.
function hashId(text) {
  let a = 0x811c9dc5, b = 0x9747b28c;
  for (let i = 0; i < text.length; i++) {
    const c = text.charCodeAt(i);
    a = Math.imul(a ^ c, 0x01000193) >>> 0;
    b = Math.imul(b ^ c, 0x01000193) >>> 0;
  }
  return a.toString(36) + b.toString(36);
}

/** `site` is the compiler's stamp for the call; uncompiled code is keyed by call order instead. */
export function useId(site) {
  if (!render.active || render.scope === null) throw Error('UI_SCOPE: useId outside render');
  const counts = render.candidate.idOrder ??= new Map();
  const base = `${render.scope}\u0000${site ?? '#'}`;
  const seen = counts.get(base) ?? 0;
  counts.set(base, seen + 1);
  return `lx-${hashId(seen ? `${base}\u0000${seen}` : base)}`;
}

export function hasLocalUpdates() { return page.updates.size > 0 || page.invalidated === true; }

export function useView() {
  if (!render.active) throw Error('UI_SCOPE: useView outside render');
  render.readsView = true;
  noteView();
  const scoped = scopedEnv();
  if (!scoped) return render.view;
  const scopedView = Object.freeze({ ...render.view, env: scoped.env });
  rawEnvs.set(scopedView, scoped.base);
  return scopedView;
}

/** The environment alone: a component re-runs only when a field it read changes. */
export function useEnv() {
  if (!render.active) throw Error('UI_SCOPE: useEnv outside render');
  return fieldTracked(scopedEnv()?.env ?? render.view.env);
}

const tracked = new WeakMap();
// Each field read names itself; listing the keys (a spread, `Object.keys`) reads them all.
function fieldTracked(env) {
  if (!env || typeof env !== 'object') return env;
  let proxy = tracked.get(env);
  if (!proxy) tracked.set(env, proxy = new Proxy(env, {
    get(target, key, receiver) {
      if (typeof key === 'string') noteEnv(key);
      return Reflect.get(target, key, receiver);
    },
    has(target, key) {
      if (typeof key === 'string') noteEnv(key);
      return Reflect.has(target, key);
    },
    ownKeys(target) {
      noteEnv('*');
      return Reflect.ownKeys(target);
    },
  }));
  return proxy;
}

// The environment under the nearest SizeClassScope, or undefined when no scope changes it.
function scopedEnv() {
  const scoped = scopedSizeClass();
  const raw = envOf(render.view);
  if (!scoped || !raw || raw.sizeClass === scoped) return undefined;
  const base = Object.freeze({ ...raw, sizeClass: scoped });
  const env = rawEnvs.has(render.view) ? new Proxy(base, {
    get(target, key, receiver) {
      if (key !== 'sizeClass' && hostContextKeys.has(key)) hostContextRead = true;
      if (appearanceKeys.has(key)) appearanceRead = true;
      return Reflect.get(target, key, receiver);
    },
  }) : base;
  return { env, base };
}

// Before the catalog arrives a page renders with no actions; resources wait for it.
function pageAction(action, code, mode) {
  if (action === undefined && !Object.keys(render.view.actions).length) return undefined;
  const info = typeof action === 'function' ? actionInfo(action) : undefined;
  if (!info || info.mode !== mode || info.name.includes('.'))
    throw Error(`${code}: expected a ${mode === 'stream' ? 'streaming' : 'unary'} action from view.actions`);
  return { action, name: info.name };
}

function own(name, initial, kind, code, create, definition) {
  const binding = local(name, initial, kind);
  const { key, cell } = bindings.get(binding);
  // Resources settle asynchronously; once their cell unmounts the update has no reader.
  const state = { get value() { return binding.value; }, set(value) {
    if (page.locals.get(key) === cell || render.candidate?.locals.get(key) === cell) binding.set(value);
  } };
  const { candidate } = render;
  if (candidate.resourceNames.has(cell)) throw Error(`${code}: duplicate resource name`);
  candidate.resourceNames.add(cell);
  const resource = page.resources.get(cell) ?? create(state);
  candidate.resources.set(cell, { resource, definition });
  return resource;
}

const guard = (what, fn) => (...args) => { outsideRender(what); return fn(...args); };

export function useRequest(name, action, ...params) {
  if (params.length > 1) throw Error('UI_REQUEST: expected at most one JSON payload');
  const target = pageAction(action, 'UI_REQUEST', 'call');
  const payload = params[0] === undefined ? undefined : immutable(params[0]);
  const resource = own(name, { pending: false, stale: false, value: null, error: null }, 'request', 'UI_REQUEST', requestResource,
    { action: target?.action, params: payload, key: JSON.stringify(payload) });
  return Object.freeze({
    get pending() { return resource.state.pending; },
    get stale() { return resource.state.stale; },
    get value() { return resource.state.value; },
    get error() { return resource.state.error; },
    reload: guard('request reload', resource.reload),
  });
}

/** `useAction(action)` keys by call order in the component; `useAction(name, action)` by name. */
export function useAction(...args) {
  if (args.length < 1 || args.length > 2) throw Error('UI_ACTION: useAction takes (action) or (name, action)');
  if (!render.active || render.scope === null) throw Error('UI_SCOPE: useAction outside render');
  let name = args[0];
  if (args.length === 1) {
    const counts = render.candidate.actionOrder ??= new Map();
    const order = counts.get(render.scope) ?? 0;
    counts.set(render.scope, order + 1);
    name = `#${order}`;
  }
  const target = pageAction(args[args.length - 1], 'UI_ACTION', 'call');
  const resource = own(name, { pending: false, error: null }, 'action', 'UI_ACTION', actionResource, { action: target?.action });
  return Object.freeze({
    get pending() { return resource.state.pending; },
    get error() { return resource.state.error; },
    run: guard('action run', resource.run),
  });
}

export function useStream(name, action, options = {}) {
  if (!options || typeof options !== 'object' || (options.onData !== undefined && typeof options.onData !== 'function'))
    throw Error('UI_STREAM: onData must be a function');
  const target = pageAction(action, 'UI_STREAM', 'stream');
  const resource = own(name, { status: 'idle', error: null, result: null, data: null }, 'stream', 'UI_STREAM', streamResource,
    { action: target?.action, onData: options.onData });
  return Object.freeze({
    get status() { return resource.state.status; },
    get error() { return resource.state.error; },
    get result() { return resource.state.result; },
    get data() { return resource.state.data; },
    start: guard('stream mutation', resource.start), cancel: guard('stream mutation', resource.cancel),
  });
}

export function useChannel(name, action, options) {
  if (!options || typeof options !== 'object' || !('params' in options) || (options.onData !== undefined && typeof options.onData !== 'function'))
    throw Error('UI_CHANNEL: expected { params, onData? }');
  const target = pageAction(action, 'UI_CHANNEL', 'call');
  const params = options.params === undefined ? undefined : immutable(options.params);
  const resource = own(name, { connecting: false, connected: false, error: null }, 'channel', 'UI_CHANNEL',
    state => channelResource(state, report),
    { topic: target?.name, params, key: JSON.stringify(params), onData: options.onData, channels: render.view.channels });
  return Object.freeze({
    get connecting() { return resource.state.connecting; },
    get connected() { return resource.state.connected; },
    get error() { return resource.state.error; },
    connect: guard('channel resource mutation', resource.connect),
    close: guard('channel resource mutation', resource.close),
    send: guard('channel resource mutation', resource.send),
  });
}

export function useLeaveGuard(name, options) {
  if (typeof options?.enabled !== 'boolean') throw Error('UI_LEAVE: expected { enabled: boolean }');
  const resource = own(name, { pending: null, requests: 0, error: null }, 'leave', 'UI_LEAVE', leaveGuards.resource, { enabled: options.enabled });
  return Object.freeze({
    get pending() { return resource.state.pending; },
    get requests() { return resource.state.requests; },
    get error() { return resource.state.error; },
    answer: guard('leave mutation', resource.answer), request: guard('leave mutation', resource.request),
  });
}

/** LIFO back-navigation handler (intercepts Back only, never Home). */
export function useBackHandler(enabled, onBack) {
  if (!render.active || render.scope === null) throw Error('UI_SCOPE: useBackHandler outside render');
  if (typeof enabled !== 'boolean') throw Error('UI_BACK: enabled must be a boolean');
  if (typeof onBack !== 'function') throw Error('UI_BACK: onBack must be a function');
  const counts = render.candidate.backOrder ??= new Map();
  const order = counts.get(render.scope) ?? 0;
  counts.set(render.scope, order + 1);
  own(`#${order}`, null, 'back', 'UI_BACK', leaveGuards.backHandler, { enabled, onBack });
}

// The host pushes these when the lxapp's surface changes; it repaints only pages whose View read one.
const hostContextKeys = new Set(['sizeClass', 'canDock']);
let hostContextRead = false;
// Whether page code ever read the appearance; kept across renders, since a recalled component does not read again.
const appearanceKeys = new Set(['appearance', 'highContrast']);
let appearanceRead = false;
const rawEnvs = new WeakMap();
// The data the last view froze: a new one shares its unchanged parts.
let lastData;
// The native replica revision `lastData` matches; null until a whole tree arrives.
let lastRevision = null;

/** Brings `lastData` from revision `from` to `revision` by replaying the replica's ops; false when
 *  this isolate is not at `from`, and the host then sends the whole tree. */
export function syncData(from, revision, ops) {
  if (lastData === undefined || lastRevision !== from) return false;
  try {
    lastData = applyPatch(lastData, ops);
  } catch {
    lastRevision = null;
    return false;
  }
  lastRevision = revision;
  return true;
}

/** Adds logical `inlineStart` / `inlineEnd` fields to an edges object based on direction. */
export function withLogicalEdges(edges, rtl) {
  if (!edges || typeof edges !== 'object') return edges;
  const inlineStart = rtl ? edges.right : edges.left;
  const inlineEnd = rtl ? edges.left : edges.right;
  if (edges.inlineStart === inlineStart && edges.inlineEnd === inlineEnd) return edges;
  return Object.freeze({
    ...edges,
    inlineStart,
    inlineEnd,
  });
}

/** Normalizes `insets`, `safeArea`, and `hinge` on an environment snapshot. */
export function normalizeEnv(env) {
  if (!env || typeof env !== 'object') return env;
  const rtl = env.direction === 'rtl';
  const insets = env.insets ? withLogicalEdges(env.insets, rtl) : undefined;
  const safeArea = env.safeArea ? withLogicalEdges(env.safeArea, rtl) : undefined;
  if (insets === env.insets && safeArea === env.safeArea && env.hinge !== undefined) return env;
  return {
    ...env,
    ...(insets ? { insets } : {}),
    ...(safeArea ? { safeArea } : {}),
    ...(env.hinge === undefined ? { hinge: null } : {}),
  };
}

/** Ensures raw `{ env }` test inputs carry normalized logical edges and `hinge: null`. */
export function ensureNormalizedView(view) {
  if (!view || typeof view !== 'object' || rawEnvs.has(view) || !view.env || typeof view.env !== 'object') return view;
  const env = normalizeEnv(view.env);
  return env === view.env ? view : { ...view, env };
}

/** `data` undefined: the isolate's own copy, already synced by `syncData`. */
export function createView(data, env, revision) {
  const { actions, channels } = currentActions();
  hostContextRead = false;
  // A host that does not report the capsule shows none.
  const withCapsule = env && typeof env === 'object' && !Object.hasOwn(env, 'capsule') ? { ...env, capsule: null } : env;
  const frozen = immutable(normalizeEnv(withCapsule));
  const watched = frozen && typeof frozen === 'object' ? new Proxy(frozen, {
    get(target, key, receiver) {
      if (hostContextKeys.has(key)) hostContextRead = true;
      if (appearanceKeys.has(key)) appearanceRead = true;
      return Reflect.get(target, key, receiver);
    },
  }) : frozen;
  const view = Object.freeze({ data: data === undefined ? lastData : shared(data, lastData), env: watched, actions, channels });
  if (data !== undefined) lastRevision = typeof revision === 'number' ? revision : null;
  lastData = view.data;
  rawEnvs.set(view, frozen);
  return view;
}

/** The environment without read tracking, for the runtime's own lowering. */
export const envOf = view => (view && rawEnvs.get(view)) ?? view?.env;
/** Whether the View read a host-context key since its view was created. */
export const readsHostContext = () => hostContextRead;
/** What of the environment the page depends on, for the host: 1 host context, 2 the appearance (page code read `appearance` or `highContrast`, or a class has a theme condition), 4 the clock by the second, 8 by the minute. */
export const envReads = () => ((hostContextRead || classesReadSizeClass()) ? 1 : 0) | (appearanceRead || classesReadTheme() ? 2 : 0)
  | (clockStep() === 1000 ? 4 : clockStep() ? 8 : 0);
export function resetEnvReads() { appearanceRead = false; }

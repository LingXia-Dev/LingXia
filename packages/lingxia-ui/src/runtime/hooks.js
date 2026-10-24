import { immutable } from './data.js';
import { render, page, outsideRender } from './state.js';
import { actionInfo, currentActions, leaveGuards, report } from './actions.js';
import { streamResource, requestResource } from './resources.js';
import { channelResource } from './channel-resource.js';

const bindings = new WeakMap();

/** The cell behind a `useLocal` binding, if it is one. */
export function cellOf(binding) { return bindings.get(binding); }

function local(name, initial, resource = '') {
  if (!render.active || render.scope === null || typeof name !== 'string' || !name || name.length > 256) throw Error('UI_SCOPE: invalid local state');
  const { candidate, scope } = render;
  const key = JSON.stringify(resource ? [scope, resource, name] : [scope, name]);
  if (!resource && candidate.declarations.has(key)) throw Error(`UI_LOCAL: duplicate declaration ${JSON.stringify(name)} in ${scope}`);
  candidate.declarations.add(key);
  const cell = candidate.locals.get(key) ?? render.lookupLocals?.get(key) ?? page.locals.get(key) ?? { value: immutable(initial, 0, true) };
  candidate.locals.set(key, cell);
  for (const collector of render.collectors) collector.set(key, cell);
  const binding = Object.freeze({
    get value() { return page.updates.has(cell) ? page.updates.get(cell) : cell.value; },
    set(value) {
      outsideRender('state mutation');
      // A cell of the prepared candidate takes the value when that candidate commits.
      if (page.locals.get(key) !== cell && render.candidate?.locals.get(key) !== cell)
        throw Error(`UI_LOCAL: ${JSON.stringify(name)} in ${scope} is no longer mounted`);
      page.updates.set(cell, immutable(value, 0, true));
    },
  });
  bindings.set(binding, { key, cell });
  return binding;
}

export function useLocal(name, initial) { return local(name, initial); }
export function hasLocalUpdates() { return page.updates.size > 0; }

export function useView() {
  if (!render.active) throw Error('UI_SCOPE: useView outside render');
  return render.view;
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

export function useStream(name, action, options = {}) {
  if (!options || typeof options !== 'object' || (options.onData !== undefined && typeof options.onData !== 'function'))
    throw Error('UI_STREAM: onData must be a function');
  const target = pageAction(action, 'UI_STREAM', 'stream');
  const resource = own(name, { status: 'idle', error: null, result: null }, 'stream', 'UI_STREAM', streamResource,
    { action: target?.action, onData: options.onData });
  return Object.freeze({
    get status() { return resource.state.status; },
    get error() { return resource.state.error; },
    get result() { return resource.state.result; },
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

export function createView(data, env) {
  const { actions, channels } = currentActions();
  return Object.freeze({ data: immutable(data), env: immutable(env), actions, channels });
}

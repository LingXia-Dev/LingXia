import { streamResource, requestResource } from './runtime/resources.js';
import { acceptsProp } from './authoring/props.js';
import { normalizeShadows } from './shadow.js';
import { normalizeGradient } from './gradient.js';
import { resolveStyle, focusStyle } from './classes.js';
export { configureClasses } from './classes.js';
import { tags, fields, vectorField, dimensionPercentFields, styleOptions, buttonProps, listFields, reachProps, imageProps, inputProps, checkProps, radioProps, sliderProps, gridProps, textProps, plotProps } from './schema.js';
import { createStream } from './stream.js';
import { createChannel } from './channel.js';
import { channelResource } from './channel-resource.js';
import { createLeaveGuards } from './leave-guard.js';

let arena = [], epoch = 0, nextHandle = 1;
const brand = Symbol('ui candidate');
const bindings = new WeakMap();
const componentIds = new WeakMap();
let nextComponent = 1;
let locals = new Map(), callbacks = new Map(), updates = new Map();
let committed = null, lookupLocals = null, localCollectors = [];
let ownedResources = new Map();
let candidate = null, scope = null, currentView = null, rendering = false;
let actions = Object.freeze(Object.create(null)), actionEpoch = 0, nextRequest = 1;
let requests = [], actionErrors = [];
const pendingActions = new Map();
const activeChannels = new Map();
const unaryActions = new WeakSet();
const streamActions = new WeakSet();
const actionNames = new WeakMap();
let channels = Object.freeze({ open() { throw Error('UI_CHANNEL: bridge not ready'); } });
const leaveGuards = createLeaveGuards(callLeaveHost, error => {
  if (actionErrors.length < 32) actionErrors.push(String(error).slice(0, 2048));
});
const has = (object, key) => Object.prototype.hasOwnProperty.call(object, key);

function bridgeError(code, message, data) {
  const error = new Error(message ?? code);
  error.name = 'LxBridgeError'; error.code = code;
  if (data !== undefined) error.data = data;
  return error;
}

function cancelActions() {
  actionEpoch++;
  leaveGuards.reset();
  const pending = [...pendingActions.values()];
  pendingActions.clear(); requests = []; actions = Object.freeze(Object.create(null));
  const oldChannels = [...activeChannels.values()]; activeChannels.clear();
  channels = Object.freeze({ open() { throw Error('UI_CHANNEL: bridge not ready'); } });
  for (const { reject } of pending) reject(bridgeError('BRIDGE_CANCELED', 'Page reset'));
  for (const entry of oldChannels) {
    const error = bridgeError('BRIDGE_CANCELED', 'Page reset');
    entry.reject(error); entry.channel.reject(error);
  }
}

// Native supplies the same compiled action catalog used by WebView pages, after ready.
export function configureActions(metadata) {
  if (rendering) throw Error('UI_RENDER: configure actions during render');
  const names = metadata?.__names, modes = metadata?.__modes;
  if (!Array.isArray(names) || names.length > 256 || !modes || typeof modes !== 'object' ||
      names.some(name => typeof name !== 'string' || !name || name.length > 256 || !has(modes, name) || !['call', 'stream'].includes(modes[name])) ||
      new Set(names).size !== names.length) throw Error('UI_ACTION: invalid action catalog');
  cancelActions();
  const epoch = actionEpoch, built = Object.create(null), actionModes = { ...modes };
  for (const name of names) built[name] = (...args) => {
    if (rendering) throw Error('UI_RENDER: action during render');
    if (epoch !== actionEpoch) throw bridgeError('BRIDGE_CANCELED', 'Stale page action');
    if (args.length > 1) throw Error('UI_ACTION: expected at most one JSON payload');
    if (pendingActions.size + activeChannels.size >= 256 || requests.length >= 256) throw Error('UI_LIMIT: too many pending actions');
    const params = args.length && args[0] !== undefined ? immutable(args[0]) : undefined;
    const id = `ui-action:${nextRequest++}`;
    const frame = { v: 2, kind: 'req', id, method: name, cap: 'page' };
    if (params !== undefined) frame.params = params;
    const encoded = JSON.stringify(frame);
    if (encoded.length > 65536) throw Error('UI_LIMIT: action payload too large');
    if (actionModes[name] === 'stream') {
      const stream = createStream(id, cause => {
        if (!pendingActions.has(id)) return;
        // Reserve a bounded cancellation slot for every admitted request.
        if (requests.length >= 512) throw Error('UI_LIMIT: too many pending cancellations');
        requests.push(JSON.stringify({ v: 2, kind: 'cancel', id }));
        pendingActions.delete(id);
        stream.reject(cause ?? bridgeError('BRIDGE_CANCELED', 'Stream canceled'));
      }, cause => {
        if (epoch === actionEpoch && actionErrors.length < 32) actionErrors.push(String(cause).slice(0, 2048));
      }, bridgeError);
      pendingActions.set(id, { ...stream, sequence: -1 });
      requests.push(encoded);
      return stream.handle;
    }
    const promise = new Promise((resolve, reject) => pendingActions.set(id, { resolve, reject }));
    // A fire-and-forget click must not create an unhandled rejection; callers still receive it.
    promise.catch(() => {});
    requests.push(encoded);
    return promise;
  };
  for (const name of names) {
    if (actionModes[name] === 'call') unaryActions.add(built[name]);
    else streamActions.add(built[name]);
    actionNames.set(built[name], name);
  }
  actions = Object.freeze(built);
  channels = Object.freeze({
    open(topic, params) {
      if (rendering) throw Error('UI_RENDER: channel during render');
      if (epoch !== actionEpoch) throw bridgeError('BRIDGE_CANCELED', 'Stale page channel');
      if (typeof topic !== 'string' || !has(built, topic) || topic.includes('.') || actionModes[topic] !== 'call')
        throw Error('UI_CHANNEL: expected a page channel handler');
      if (pendingActions.size + activeChannels.size >= 256 || requests.length >= 256)
        throw Error('UI_LIMIT: too many pending channels');
      const id = `ui-channel:${nextRequest++}`;
      const frame = { v: 2, kind: 'ch.open', id, topic, cap: 'page' };
      if (params !== undefined) frame.params = immutable(params);
      const encoded = JSON.stringify(frame);
      if (encoded.length > 65536) throw Error('UI_LIMIT: channel payload too large');
      let outgoing = 0;
      const channel = createChannel(id, payload => {
        if (rendering) throw Error('UI_RENDER: channel send during render');
        const entry = activeChannels.get(id);
        if (epoch !== actionEpoch || !entry?.open) throw bridgeError('STREAM_CLOSED', 'Channel is closed');
        if (requests.length >= 256) throw Error('UI_LIMIT: too many pending channel frames');
        const message = JSON.stringify({ v: 2, kind: 'ch.data', id, seq: outgoing, payload: immutable(payload) });
        if (message.length > 65536) throw Error('UI_LIMIT: channel payload too large');
        requests.push(message); outgoing++;
      }, (code, reason, cause) => {
        if (rendering) throw Error('UI_RENDER: channel close during render');
        if (!activeChannels.has(id)) return;
        if (typeof code !== 'string' || code.length > 256 || typeof reason !== 'string' || reason.length > 2048)
          throw Error('UI_CHANNEL: invalid close reason');
        if (requests.length >= 512) throw Error('UI_LIMIT: too many pending cancellations');
        requests.push(JSON.stringify({ v: 2, kind: 'ch.close', id, code, reason }));
        activeChannels.delete(id);
        if (cause) channel.reject(cause); else channel.finish(code, reason, true);
      }, cause => {
        if (epoch === actionEpoch && actionErrors.length < 32) actionErrors.push(String(cause).slice(0, 2048));
      }, bridgeError);
      const promise = new Promise((resolve, reject) => activeChannels.set(id, { channel, resolve, reject, open: false, sequence: -1 }));
      promise.catch(() => {});
      requests.push(encoded);
      return promise;
    },
  });
}

function callLeaveHost(method, params) {
  if (rendering) throw Error('UI_RENDER: leave control during render');
  if (!['navigation.setLeaveGuard', 'navigation.leave'].includes(method)) throw Error('UI_LEAVE: invalid control');
  if (pendingActions.size + activeChannels.size >= 256 || requests.length >= 256) throw Error('UI_LIMIT: too many pending actions');
  const id = `ui-action:${nextRequest++}`;
  const promise = new Promise((resolve, reject) => pendingActions.set(id, { resolve, reject }));
  promise.catch(() => {});
  requests.push(JSON.stringify({ v: 2, kind: 'req', id, method: `host.${method}`, cap: 'host', params }));
  return promise;
}

export function requestLeave(reason) {
  if (rendering) throw Error('UI_RENDER: leave request during render');
  leaveGuards.request(reason);
}

export function takeActionRequests() { const result = requests; requests = []; return result; }
export function takeActionErrors() { const result = actionErrors; actionErrors = []; return result; }
export function receiveActionResponse(frame) {
  if (rendering) throw Error('UI_RENDER: response during render');
  if (frame?.v === 2 && ['ch.ack', 'ch.data', 'ch.close'].includes(frame.kind)) return receiveChannelFrame(frame);
  if (frame?.v !== 2 || !['res', 'event'].includes(frame.kind) || typeof frame.id !== 'string' ||
      (frame.kind === 'res' && typeof frame.ok !== 'boolean') ||
      (frame.kind === 'event' && (!Number.isSafeInteger(frame.seq) || frame.seq < 0)))
    throw Error('UI_ACTION: invalid response');
  const pending = pendingActions.get(frame.id);
  if (!pending) return false;
  if (frame.kind === 'event') {
    if (!pending.push) throw Error('UI_STREAM: event for unary action');
    if (frame.seq <= pending.sequence || (pending.sequence === -1 && frame.seq !== 0))
      throw Error('UI_STREAM: invalid sequence');
    const value = immutable(frame.payload ?? null);
    pending.sequence = frame.seq;
    pending.push(value);
    return true;
  }
  const value = frame.ok ? immutable(frame.result ?? null) : bridgeError(
    typeof frame.error?.code === 'string' ? frame.error.code : 'BRIDGE_INTERNAL_ERROR',
    typeof frame.error?.message === 'string' ? frame.error.message : undefined,
    frame.error?.data === undefined ? undefined : immutable(frame.error.data));
  pendingActions.delete(frame.id);
  if (frame.ok) pending.resolve(value); else pending.reject(value);
  return true;
}

function receiveChannelFrame(frame) {
  if (typeof frame.id !== 'string' ||
      (frame.kind === 'ch.ack' && typeof frame.ok !== 'boolean') ||
      (frame.kind === 'ch.data' && (!Number.isSafeInteger(frame.seq) || frame.seq < 0)) ||
      (frame.kind === 'ch.close' && ((frame.code !== undefined && typeof frame.code !== 'string') ||
        (frame.reason !== undefined && typeof frame.reason !== 'string'))))
    throw Error('UI_CHANNEL: invalid frame');
  const entry = activeChannels.get(frame.id);
  if (!entry) return false;
  if (frame.kind === 'ch.ack') {
    if (entry.open) throw Error('UI_CHANNEL: duplicate acknowledgement');
    if (frame.ok) { entry.open = true; entry.resolve(entry.channel.handle); }
    else {
      const cause = bridgeError(frame.error?.code ?? 'BRIDGE_INTERNAL_ERROR', frame.error?.message,
        frame.error?.data === undefined ? undefined : immutable(frame.error.data));
      activeChannels.delete(frame.id); entry.reject(cause); entry.channel.reject(cause);
    }
    return true;
  }
  if (!entry.open) throw Error('UI_CHANNEL: frame before acknowledgement');
  if (frame.kind === 'ch.close') {
    activeChannels.delete(frame.id); entry.channel.finish(frame.code, frame.reason);
  } else {
    if (frame.seq <= entry.sequence || (entry.sequence === -1 && frame.seq !== 0))
      throw Error('UI_CHANNEL: invalid sequence');
    const value = immutable(frame.payload ?? null);
    entry.sequence = frame.seq; entry.channel.push(value);
  }
  return true;
}

function immutable(value, depth = 0, omitUndefined = false) {
  if (depth > 64) throw Error('UI_LOCAL: value too deep');
  if (value === null || typeof value === 'string' || typeof value === 'boolean') return value;
  if (typeof value === 'number' && Number.isFinite(value)) return value;
  if (Array.isArray(value)) return Object.freeze(value.map(v => immutable(v, depth + 1, omitUndefined)));
  if (value && Object.getPrototypeOf(value) === Object.prototype) {
    return Object.freeze(Object.fromEntries(Object.entries(value)
      .filter(([,v]) => !omitUndefined || v !== undefined)
      .map(([k,v]) => [k, immutable(v, depth + 1, omitUndefined)])));
  }
  throw Error('UI_LOCAL: expected JSON value');
}

export function useView() {
  if (!rendering) throw Error('UI_SCOPE: useView outside render');
  return currentView;
}

export function createView(data, env) {
  return Object.freeze({ data: immutable(data), env: immutable(env), actions, channels });
}

function local(name, initial, resource = '') {
  if (!rendering || scope === null || typeof name !== 'string' || !name || name.length > 256) throw Error('UI_SCOPE: invalid local state');
  const key = JSON.stringify(resource ? [scope, resource, name] : [scope, name]);
  if (!resource && candidate.declarations.has(key)) throw Error(`UI_LOCAL: duplicate declaration ${JSON.stringify(name)} in ${scope}`);
  candidate.declarations.add(key);
  let cell = candidate.locals.get(key) ?? lookupLocals?.get(key) ?? locals.get(key);
  if (!cell) cell = { value: immutable(initial, 0, true) };
  candidate.locals.set(key, cell);
  for (const collector of localCollectors) collector.set(key, cell);
  const binding = Object.freeze({
    get value() { return updates.has(cell) ? updates.get(cell) : cell.value; },
    set(value) {
      if (rendering) throw Error('UI_RENDER: state mutation during render');
      if (locals.get(key) !== cell) return;
      updates.set(cell, immutable(value, 0, true));
    },
  });
  bindings.set(binding, {key,cell}); return binding;
}

export function useLocal(name, initial) { return local(name, initial); }
export function hasLocalUpdates() { return updates.size > 0; }

function resourceAction(method, kind, registered) {
  const catalog = currentView.actions, names = Object.keys(catalog);
  const label = kind === 'UI_STREAM' ? 'streaming' : 'unary';
  if (method === undefined && !names.length) return undefined;
  if (typeof method !== 'function' && (typeof method !== 'string' || !method || method.includes('.')))
    throw Error(`${kind}: expected a page action name or action reference`);
  const action = typeof method === 'function' ? method : catalog[method];
  if (action !== undefined && (!registered.has(action) || !has(catalog, actionNames.get(action)) || catalog[actionNames.get(action)] !== action || actionNames.get(action).includes('.')))
    throw Error(`${kind}: expected a ${label} page action`);
  if (action === undefined && names.length) throw Error(`${kind}: unknown page action`);
  return action;
}

export function useStream(name, method, options) {
  const state = local(name, { status: 'idle', error: null, result: null }, 'stream');
  if (typeof options?.onData !== 'function')
    throw Error('UI_STREAM: expected a page stream name and onData');
  const action = resourceAction(method, 'UI_STREAM', streamActions);
  const { cell } = bindings.get(state);
  if (candidate.resourceNames.has(cell)) throw Error('UI_STREAM: duplicate resource name');
  candidate.resourceNames.add(cell);
  const resource = ownedResources.get(cell) ?? streamResource(state);
  candidate.resources.set(cell, { resource, definition: { method, action, onData: options.onData } });
  const guarded = fn => (...args) => {
    if (rendering) throw Error('UI_RENDER: stream mutation during render');
    return fn(...args);
  };
  return Object.freeze({
    get status() { return resource.state.status; },
    get error() { return resource.state.error; },
    get result() { return resource.state.result; },
    start: guarded(resource.start), cancel: guarded(resource.cancel),
  });
}

export function useRequest(name, method, params) {
  const state = local(name, { pending: false, value: null, error: null }, 'request');
  // A page can render before its action catalog arrives; the next committed
  // catalog starts the request. Streams have a different ownership contract.
  const action = resourceAction(method, 'UI_REQUEST', unaryActions);
  const payload = params === undefined ? undefined : immutable(params);
  const { cell } = bindings.get(state);
  if (candidate.resourceNames.has(cell)) throw Error('UI_REQUEST: duplicate resource name');
  candidate.resourceNames.add(cell);
  const resource = ownedResources.get(cell) ?? requestResource(state);
  candidate.resources.set(cell, { resource, definition: { method, action, params: payload, key: JSON.stringify(payload) } });
  return Object.freeze({
    get pending() { return resource.state.pending; },
    get value() { return resource.state.value; },
    get error() { return resource.state.error; },
    reload() {
      if (rendering) throw Error('UI_RENDER: request reload during render');
      return resource.reload();
    },
  });
}

export function useLeaveGuard(name, enabled) {
  if (typeof enabled !== 'boolean') throw Error('UI_LEAVE: expected boolean enabled');
  const state = local(name, { pending: null, requests: 0, error: null }, 'leave');
  const { cell } = bindings.get(state);
  if (candidate.resourceNames.has(cell)) throw Error('UI_LEAVE: duplicate resource name');
  candidate.resourceNames.add(cell);
  const resource = ownedResources.get(cell) ?? leaveGuards.resource(state);
  candidate.resources.set(cell, { resource, definition: { enabled } });
  const guarded = fn => (...args) => {
    if (rendering) throw Error('UI_RENDER: leave mutation during render');
    return fn(...args);
  };
  return Object.freeze({
    get pending() { return resource.state.pending; },
    get requests() { return resource.state.requests; },
    get error() { return resource.state.error; },
    answer: guarded(resource.answer), request: guarded(resource.request),
  });
}

export function useChannel(name, topic, options) {
  const state = local(name, { connecting: false, connected: false, error: null }, 'channel');
  if (typeof topic !== 'string' || !topic || typeof options?.onData !== 'function')
    throw Error('UI_CHANNEL: expected topic and onData');
  const { cell } = bindings.get(state);
  if (candidate.resourceNames.has(cell)) throw Error('UI_CHANNEL: duplicate resource name');
  candidate.resourceNames.add(cell);
  const params = options.params === undefined ? undefined : immutable(options.params);
  const definition = { topic, params, key: JSON.stringify(params), onData: options.onData, channels: currentView.channels };
  const resource = ownedResources.get(cell) ?? channelResource(state, error => {
    if (actionErrors.length < 32) actionErrors.push(String(error).slice(0, 2048));
  });
  candidate.resources.set(cell, { definition, resource });
  const guarded = callback => (...args) => {
    if (rendering) throw Error('UI_RENDER: channel resource mutation during render');
    return callback(...args);
  };
  return Object.freeze({
    get connecting() { return resource.state.connecting; },
    get connected() { return resource.state.connected; },
    get error() { return resource.state.error; },
    connect: guarded(resource.connect), close: guarded(resource.close), send: guarded(resource.send),
  });
}

export function childrenToArray(children) {
  return flatten([children]);
}

export function List({ items, getKey, renderItem, getItemHeight, ...props }) {
  return h('list', { ...props, count: items.length,
    itemKeys: items.map(getKey), row: index => renderItem(items[index], index),
    ...(getItemHeight ? { height: index => getItemHeight(items[index], index) } : {}) });
}

export function h(tag, props, ...children) {
  if (!rendering) throw Error('UI_SCOPE: JSX outside render');
  if (tag === 'view') tag = 'row';
  const handle = { [brand]: [epoch, arena.length] };
  if (arena.length >= 16384) throw Error('UI_LIMIT: candidate arena too large');
  const { children: suppliedChildren, ...attributes } = props ?? {};
  arena.push([tag, attributes, children.length ? children : suppliedChildren === undefined ? [] : [suppliedChildren]]);
  return handle;
}

function flatten(children, depth = 0, result = []) {
  if (depth > 128) throw Error('UI_LIMIT: children too deep');
  for (const child of children) {
    if (result.length >= 16384) throw Error('UI_LIMIT: too many children');
    if (Array.isArray(child)) flatten(child, depth + 1, result);
    else if (child !== null && child !== undefined && typeof child !== 'boolean') result.push(child);
  }
  return result;
}

function sliderValue({min,max,step},value) {
  value=Math.max(min,Math.min(max,value));
  const snapped=Math.max(min,Math.min(max,min+Math.floor((value-min)/step+0.5)*step));
  return Math.abs(max-value)<Math.abs(snapped-value)?max:snapped;
}

function validInputText(value) {
  return typeof value === 'string' && utf8Bytes(value, 65536) <= 65536;
}

// View isolates do not require the browser TextEncoder API.
function utf8Bytes(value, limit) {
  if (value.length > limit) return Infinity;
  if (/^[\x00-\x7f]*$/.test(value)) return value.length;
  let bytes = 0;
  for (const char of value) {
    const c = char.codePointAt(0);
    if (c >= 0xD800 && c <= 0xDFFF) return Infinity;
    bytes += c < 0x80 ? 1 : c < 0x800 ? 2 : c < 0x10000 ? 3 : 4;
    if (bytes > limit) return Infinity;
  }
  return bytes;
}

function resolveNode(node, identity, depth = 0, parentTag = null, inherited, buttonAncestor = false) {
  if (depth > 128) throw Error('UI_LIMIT: children too deep');
  if (node == null || typeof node === 'boolean') return null;
  if (typeof node === 'string' || typeof node === 'number') node = h('text', {}, node);
  if (!node || typeof node !== 'object' || !(brand in node)) throw Error('UI_ROOT: expected one node');
  const [owner, offset] = node[brand];
  if (owner !== epoch || !arena[offset]) throw Error('UI_STALE: candidate from another render');
  const [tag, original, children] = arena[offset];
  if (typeof tag === 'function') {
    if (!componentIds.has(tag)) componentIds.set(tag, nextComponent++);
    const previous = scope;
    scope = `${identity}/component:${componentIds.get(tag)}`;
    const componentChildren = children.length === 0 ? undefined : children.length === 1 ? children[0] : children;
    try { return resolveNode(tag({ ...original, children: componentChildren }, currentView), scope, depth + 1, parentTag, inherited, buttonAncestor); }
    catch (error) { throw new Error(`${error?.message ?? error}\n  at component ${tag.name || '<anonymous>'} (${identity})`, { cause: error }); }
    finally { scope = previous; }
  }
  if (!has(tags, tag) || tag === 'radio-option') throw Error(`UI_TAG: ${tag}`);
  const { style, typography } = resolveStyle(original.className, original.style, currentView?.env, inherited);
  const props = { ...original, style };
  const input = tag === 'input' || tag === 'textarea' || tag === 'search';
  const focusedStyle = focusStyle(original.className, original.style, currentView?.env, inherited);
  if (focusedStyle && !input) throw Error('UI_STYLE: focus styles require an editor');
  if (focusedStyle?.boxShadow !== undefined) focusedStyle.boxShadow = normalizeShadows(focusedStyle.boxShadow);
  if (focusedStyle?.backgroundGradient !== undefined) focusedStyle.backgroundGradient = normalizeGradient(focusedStyle.backgroundGradient);
  const check = tag === 'checkbox'  || tag === 'switch';
  const radio = tag === 'radio';
  const slider = tag === 'slider';
  if (tag === 'search' && (props.secure !== undefined || props.keyboard !== undefined))
    throw Error('UI_INPUT: search owns its keyboard and cannot be secure');
  for (const key of Object.keys(props)) {
    if (!acceptsProp(tag, key)) throw Error(`UI_PROP: ${key}`);
  }
  for (const key of ['key', 'testId']) if (props[key] !== undefined && typeof props[key] !== 'string') throw Error(`UI_PROP: ${key}`);
  if (buttonAncestor && !['column','row','text','spacer','image','grid','stack','vector'].includes(tag)) throw Error('UI_BUTTON: descendants must be noninteractive content');
  const flat = flatten(children), composed = (tag === 'button' || tag === 'checkbox') && flat.some(child => !['string','number'].includes(typeof child));
  if ((tag === 'button' || tag === 'checkbox') && ((props.accessibilityLabel !== undefined && (typeof props.accessibilityLabel !== 'string' || !props.accessibilityLabel.trim() || props.accessibilityLabel.length > 1024 || props.accessibilityLabel.includes('\0'))) || (composed && (!props.accessibilityLabel || flat.some(child => ['string','number'].includes(typeof child)))))) throw Error('UI_BUTTON: composed button requires an explicit label and no inline text');
  const leaf = tag === 'text' || (tag === 'button' && !composed) || (check && !composed);
  if (leaf && flat.some(x => !['string', 'number'].includes(typeof x))) throw Error(`UI_CHILD: ${tag}`);
  if ((tag === 'vector' || tag === 'spacer' || tag === 'list' || tag === 'image' || tag === 'plot' || input || radio || slider) && flat.length) throw Error(`UI_CHILD: ${tag}`);
  if (tag === 'vector') validateVector(props);
  if (tag === 'plot') {
    if (['smooth','area'].some(name => props[name] !== undefined && typeof props[name] !== 'boolean') ||
        (props.kind !== 'line' && (props.smooth || props.area)))
      throw Error('UI_PLOT: smooth and area must be booleans on line plots');
    if (typeof props.accessibilityLabel !== 'string' || !props.accessibilityLabel.trim() ||
        props.accessibilityLabel.length > 1024 || props.accessibilityLabel.includes('\0'))
      throw Error('UI_PLOT: accessibilityLabel is required');
    if (!['bar','line','pie'].includes(props.kind) || !Array.isArray(props.values) ||
        props.values.length < 1 || props.values.length > 128 ||
        props.values.some(v => typeof v !== 'number' || !Number.isFinite(v) || Math.abs(v)>1e9 || (props.kind==='pie' && v<0)))
      throw Error('UI_PLOT: invalid kind or values');
    if (props.markerEmphasisScale !== undefined && (props.kind !== 'line' || typeof props.markerEmphasisScale !== 'number' || !Number.isFinite(props.markerEmphasisScale) || props.markerEmphasisScale < 1 || props.markerEmphasisScale > 4))
      throw Error('UI_PLOT: invalid marker emphasis scale');
    if (props.pieLayout !== undefined) {
      const layout = props.pieLayout;
      if (props.kind !== 'pie' || !layout || typeof layout !== 'object' || Array.isArray(layout) ||
          Object.keys(layout).some(key => !['center','innerRadius','outerRadius','emphasisScale'].includes(key)) ||
          !Array.isArray(layout.center) || layout.center.length !== 2 ||
          [...layout.center, layout.innerRadius, layout.outerRadius].some(value => typeof value !== 'number' || !Number.isFinite(value) || value < 0 || value > 1) ||
          layout.innerRadius >= layout.outerRadius ||
          (layout.emphasisScale !== undefined && (typeof layout.emphasisScale !== 'number' || !Number.isFinite(layout.emphasisScale) || layout.emphasisScale < 0 || layout.emphasisScale > 64)))
        throw Error('UI_PLOT: invalid pie layout');
    }
    if (props.domainMin !== undefined || props.domainMax !== undefined) {
      const lo = props.domainMin, hi = props.domainMax;
      if (props.kind === 'pie' || !Number.isFinite(lo) || !Number.isFinite(hi) ||
          lo < -1e9 || hi > 1e9 || lo >= hi || lo > 0 || hi < 0 ||
          props.values.some(value => value < lo || value > hi))
        throw Error('UI_PLOT: domain must contain zero and all values');
    }
    if (props.onSelect !== undefined && typeof props.onSelect !== 'function')
      throw Error('UI_PLOT: onSelect must be a function');
    if (props.selectedIndex !== undefined && (!Number.isInteger(props.selectedIndex) ||
        props.selectedIndex < -1 || props.selectedIndex >= props.values.length))
      throw Error('UI_PLOT: invalid selected index');
    if (!Array.isArray(props.colors) || !props.colors.length || props.colors.length>16 ||
        props.colors.some(c => typeof c!=='string' || !/^#[0-9a-fA-F]{6}([0-9a-fA-F]{2})?$/.test(c)))
      throw Error('UI_PLOT: expected 1..16 hex colors');
  }
  for (const [name, value] of Object.entries(props.style)) {
    if (name === 'animation') {
      if (value !== null) {
        if (!value || typeof value !== 'object' || Array.isArray(value)) throw Error('UI_ANIMATION: expected definition');
        const encoded = JSON.stringify(value, (_, item) => {
          if (typeof item === 'number' && !Number.isFinite(item) || ['undefined','function','symbol','bigint'].includes(typeof item)) throw Error('UI_ANIMATION: invalid payload value');
          return item;
        });
        if (utf8Bytes(encoded, 32768) > 32768) throw Error('UI_ANIMATION: wire budget exceeded');
        props.style.animation = JSON.parse(encoded);
      }
      continue;
    }
    if (name === 'scrollAxis' && tag !== 'scroll') throw Error('UI_SCROLL: scrollAxis requires a scroll node');
    if (name === 'flexBasis') {
      const basis = typeof value === 'string' && /^(?:\d+(?:\.\d+)?|\.\d+)%$/.test(value) ? Number(value.slice(0,-1)) : value;
      if (value !== 'auto' && (typeof basis !== 'number' || !Number.isFinite(basis) || basis < 0 || basis > 16384)) throw Error('UI_STYLE_VALUE: flexBasis');
      continue;
    }
    if (name === 'height' && value === 'auto') continue;
    if (has(dimensionPercentFields, name)) {
      const length = typeof value === 'string' && /^(?:\d+(?:\.\d+)?|\.\d+)%$/.test(value) ? Number(value.slice(0,-1)) : value;
      if (typeof length !== 'number' || !Number.isFinite(length) || length < 0 || length > 16384) throw Error(`UI_STYLE_VALUE: ${name}`);
      continue;
    }
    if (name === 'boxShadow') {
      props.style.boxShadow = normalizeShadows(value);
      continue;
    }
    if (name === 'backgroundGradient') {
      props.style.backgroundGradient = normalizeGradient(value);
      continue;
    }
    if (name === 'offset') {
      if (parentTag !== 'stack' || !value || typeof value !== 'object' || Array.isArray(value) ||
          Object.keys(value).some(key => key !== 'x' && key !== 'y')) throw Error('UI_STACK: offset requires a stack parent and x/y object');
      const x = value.x ?? 0, y = value.y ?? 0;
      if ([x,y].some(v => typeof v !== 'number' || !Number.isFinite(v) || Math.abs(v) > 16384) || value.x === null || value.y === null) throw Error('UI_STACK: invalid offset');
      props.style.offset = {x,y};
      continue;
    }
    if (name === 'offsetX' || name === 'offsetY') throw Error(`UI_STYLE: ${name}`);
    if (!has(fields, name)) throw Error(`UI_STYLE: ${name}`);
    if (['radiusTopLeft','radiusTopRight','radiusBottomRight','radiusBottomLeft','padding', 'paddingTop', 'paddingRight', 'paddingBottom', 'paddingLeft', 'borderWidth', 'borderTopWidth', 'borderRightWidth', 'borderBottomWidth', 'borderLeftWidth'].includes(name) &&
        (typeof value !== 'number' || !Number.isFinite(value) || value < 0 || value > 16384)) throw Error(`UI_STYLE_VALUE: ${name}`);
    if ((name === 'backdropBlur' || name === 'backdropSaturation') && (typeof value !== 'number' || !Number.isFinite(value) || value < 0 || value > (name === 'backdropBlur' ? 64 : 16))) throw Error(`UI_STYLE_VALUE: ${name}`);
    if (name === 'placeholderColor' && typeof value !== 'string') throw Error('UI_STYLE_VALUE: placeholderColor');
    if (name === 'opacity' && (typeof value !== 'number' || !Number.isFinite(value) || value < 0 || value > 1)) throw Error('UI_STYLE_VALUE: opacity');
    if (name === 'fontWeight' && (typeof value !== 'number' || value < 1 || value > 1000)) throw Error('UI_STYLE_VALUE: fontWeight');
    if (name === 'letterSpacing' && (typeof value !== 'number' || !Number.isFinite(value) || Math.abs(value) > 1024)) throw Error('UI_STYLE_VALUE: letterSpacing');
    if (name === 'lineHeight' && (typeof value !== 'number' || value <= 0 || value > 16384)) throw Error('UI_STYLE_VALUE: lineHeight');
    if (has(styleOptions, name) && !styleOptions[name].includes(value)) throw Error(`UI_STYLE_VALUE: ${name}`);
    if (['flexDirection', 'display'].includes(name) && !['row', 'column'].includes(tag))
      throw Error('UI_LAYOUT: flex direction/display requires a flex container');
    if (tag === 'list' && ((name === 'align' && value !== 'stretch') || (name === 'justify' && value !== 'start'))) throw Error('UI_LIST: rows require stretch alignment and start justification');
    if (typeof value !== 'string' && (typeof value !== 'number' || !Number.isFinite(value))) throw Error(`UI_STYLE_VALUE: ${name}`);
  }
  if (tag === 'text' && props.lines !== undefined && (!Number.isInteger(props.lines) || props.lines < 1 || props.lines > 1024)) throw Error('UI_TEXT: lines must be an integer between 1 and 1024');
  if (tag === 'grid' && (!Number.isInteger(props.columns) || props.columns < 1 || props.columns > 64)) throw Error('UI_GRID: columns must be an integer between 1 and 64');
  if (props.enabled !== undefined && typeof props.enabled !== 'boolean') throw Error('UI_PROP: enabled');
  if (props.onClick !== undefined && typeof props.onClick !== 'function') throw Error('UI_PROP: onClick');
  if (check) {
    if (props.bind !== undefined) {
      const local = bindings.get(props.bind);
      if (!local || candidate.locals.get(local.key) !== local.cell || typeof props.bind.value !== 'boolean' || props.value !== undefined || props.onChange !== undefined)
        throw Error('UI_CHECK: bind requires a local boolean cell without value/onChange');
      props.value = props.bind.value; props.onChange = value => props.bind.set(value);
    }
    if (props.value !== undefined && typeof props.value !== 'boolean') throw Error('UI_CHECK: value must be boolean');
    if (props.onChange !== undefined && typeof props.onChange !== 'function') throw Error('UI_CHECK: onChange');
  }
  if (slider) {
    if (props.bind !== undefined) {
      const local=bindings.get(props.bind);
      if (!local || candidate.locals.get(local.key)!==local.cell || typeof props.bind.value!=='number' || props.value!==undefined || props.onChange!==undefined) throw Error('UI_SLIDER: bind requires a local number cell without value/onChange');
      props.value=props.bind.value;props.onChange=value=>props.bind.set(value);
    }
    for (const [name,fallback] of [['min',0],['max',100],['step',1]]) if(props[name]===undefined)props[name]=fallback;
    if(props.value===undefined)props.value=props.min;
    const {min,max,step,value}=props;
    if (![min,max,step,value].every(v=>typeof v==='number' && Number.isFinite(v) && Math.abs(v)<=1e9) || max<=min || step<=0 || step>max-min || (max-min)/step>1e6 || min+step===min || max-step===max) throw Error('UI_SLIDER: invalid range or precision');
    if (typeof props.accessibilityLabel!=='string' || !props.accessibilityLabel.trim() || props.accessibilityLabel.length>1024 || props.accessibilityLabel.includes('\0')) throw Error('UI_SLIDER: accessibilityLabel is required');
    if (props.onChange!==undefined && typeof props.onChange!=='function') throw Error('UI_SLIDER: onChange');
    props.value=sliderValue(props,value);
  }
  if (input) {
    if (props.submitOnEnter !== undefined && (tag !== 'textarea' || typeof props.submitOnEnter !== 'boolean')) throw Error('UI_INPUT: submitOnEnter requires textarea boolean');
    if (tag === 'search') {
      if (props.secure !== undefined || props.keyboard !== undefined) throw Error('UI_INPUT: search owns its keyboard and cannot be secure');
      props.keyboard = 'search';
    }
    if (props.bind !== undefined) {
      const local = bindings.get(props.bind);
      if (!local || candidate.locals.get(local.key) !== local.cell || props.value !== undefined || props.onInput !== undefined) throw Error('UI_INPUT: bind requires a local string cell without value/onInput');
      props.value = props.bind.value; props.onInput = value => props.bind.set(value);
    }
    if (!validInputText(props.value === undefined ? '' : props.value) || !validInputText(props.placeholder === undefined ? '' : props.placeholder)) throw Error('UI_INPUT: invalid text');
    for (const name of ['secure', 'autoFocus']) if (props[name] !== undefined && typeof props[name] !== 'boolean') throw Error(`UI_INPUT: ${name}`);
    if (props.keyboard !== undefined && !['text','number','email','phone','search'].includes(props.keyboard)) throw Error('UI_INPUT: keyboard');
    for (const name of ['onInput','onSubmit']) if (props[name] !== undefined && typeof props[name] !== 'function') throw Error(`UI_INPUT: ${name}`);
  }
  const result = { tag, props, typography, focusedStyle, identity, text: leaf ? flat.join('') : '', children: [] };
  if (tag === 'plot' && props.onSelect !== undefined) result.plotSelect = [nextHandle++, index => {
    if (Number.isInteger(index) && index >= -1 && index < props.values.length) props.onSelect(index);
  }];
  if (props.onClick !== undefined && props.enabled !== false) result.callback = [nextHandle++, props.onClick];
  if (check && props.onChange !== undefined && props.enabled !== false) result.change = [nextHandle++, () => props.onChange(!(props.value ?? false))];
  if (tag === 'image' && (typeof props.src !== 'string' || !props.src.length || props.src.length > 8192 || props.src.includes('\0') ||
      (props.fit !== undefined && !['cover', 'contain', 'fill'].includes(props.fit)))) throw Error('UI_IMAGE: invalid source or fit');
  if (tag === 'image' && props.alt !== undefined && (typeof props.alt !== 'string'
      || utf8Bytes(props.alt, 1024) > 1024 || props.alt.includes('\0')
      || (props.alt.length > 0 && !props.alt.trim()))) throw Error('UI_IMAGE: invalid alternative text');
  if (slider && props.onChange!==undefined && props.enabled!==false) result.numberChange=[nextHandle++,value=>{const next=sliderValue(props,value);if(next!==props.value)props.onChange(next);}];
  result.inputs = [];
  if (input && props.enabled !== false) for (const name of ['onInput','onSubmit']) {
    if (props[name] !== undefined) result.inputs.push([inputProps[name],nextHandle++,props[name]]);
  }
  result.reaches = [];
  if (tag === 'image') {
    if (props.retryKey !== undefined && (typeof props.retryKey !== 'string' || utf8Bytes(props.retryKey,257) > 256 || props.retryKey.includes('\0')))
      throw Error('UI_IMAGE: invalid retry key');
    for (const name of ['onLoad','onError']) if (props[name] !== undefined) {
      if (typeof props[name] !== 'function') throw Error('UI_IMAGE: invalid callback');
      result.reaches.push([imageProps[name], nextHandle++, props[name]]);
    }
  }
  for (const name of Object.keys(reachProps)) {
    if (props[name] === undefined) continue;
    if (typeof props[name] !== 'function') throw Error(`UI_PROP: ${name}`);
    result.reaches.push([reachProps[name], nextHandle++, props[name]]);
  }
  if (radio) {
    if (props.bind !== undefined) {
      const local=bindings.get(props.bind);
      if (!local || candidate.locals.get(local.key)!==local.cell || typeof props.bind.value!=='string' || props.value!==undefined || props.onChange!==undefined) throw Error('UI_RADIO: bind requires a live string cell without value/onChange');
      props.value=props.bind.value; props.onChange=value=>props.bind.set(value);
    }
    if ((props.value!==undefined && (typeof props.value!=='string' || props.value.length>256)) || (props.onChange!==undefined && typeof props.onChange!=='function')) throw Error('UI_RADIO: invalid value/onChange');
    if (props.accessibilityLabel!==undefined && (typeof props.accessibilityLabel!=='string' || props.accessibilityLabel.length>1024 || props.accessibilityLabel.includes('\0'))) throw Error('UI_RADIO: invalid accessibility label');
    if (!Array.isArray(props.options) || !props.options.length || props.options.length>64) throw Error('UI_RADIO: expected 1..64 options');
    const values=new Set();
    result.children=props.options.map(option=>{
      if (!option || typeof option!=='object' || Object.keys(option).some(k=>!['value','label','enabled','testId'].includes(k)) || typeof option.value!=='string' || !option.value.length || option.value.length>256 || values.has(option.value) || typeof option.label!=='string' || (option.enabled!==undefined && typeof option.enabled!=='boolean') || (option.testId!==undefined && typeof option.testId!=='string')) throw Error('UI_RADIO: invalid or duplicate option');
      values.add(option.value);
      const value=option.value, selected=props.value===value, enabled=props.enabled!==false && option.enabled!==false;
      const child={tag:'radio-option',identity:`${identity}/radio/key:${JSON.stringify(value)}`,props:{key:value,testId:option.testId,value:selected,enabled,style:Object.fromEntries(Object.entries(props.style).filter(([name])=>['fontSize','fontWeight','fontFamily','lineHeight','color','background'].includes(name)))},text:option.label,children:[]};
      if (enabled && props.onChange) child.change=[nextHandle++,()=>{if (!selected) return props.onChange(value);}];
      return child;
    });
  } else if (tag === 'list') {
    if (!Number.isSafeInteger(props.count) || props.count < 0 || props.count > 100000 || typeof props.row !== 'function' ||
        (props.height !== undefined && typeof props.height !== 'function')) throw Error('UI_LIST: invalid row description');
    if (props.itemKeys !== undefined) {
      if (!Array.isArray(props.itemKeys) || props.itemKeys.length !== props.count) throw Error('UI_LIST: itemKeys must match count');
      const seen = new Set();
      props.itemKeys = Array.from(props.itemKeys, key => {
        if (typeof key !== 'string' || utf8Bytes(key,256) > 256 || utf8Bytes(`key:${JSON.stringify(key)}`,256) > 256 || seen.has(key)) throw Error('UI_LIST: invalid or duplicate itemKey');
        seen.add(key); return key;
      });
      if (utf8Bytes(JSON.stringify(props.itemKeys),262144) > 262144) throw Error('UI_LIST: itemKeys budget exceeded');
    }
    if (props.anchor !== undefined && props.anchor !== 'end') {
      const a = props.anchor;
      if (!a || typeof a !== 'object' || Object.keys(a).some(k => !['index', 'align', 'seq'].includes(k)) ||
          !Number.isSafeInteger(a.index) || a.index < 0 || !['start', 'end'].includes(a.align) ||
          (a.seq !== undefined && (!Number.isSafeInteger(a.seq) || a.seq < 0))) throw Error('UI_LIST: invalid anchor');
      props.anchor = { index: a.index, align: a.align, ...(a.seq === undefined ? {} : { seq: a.seq }) };
    }
    const old = committed?.lists.get(identity);
    const anchor = props.anchor === undefined ? null : JSON.stringify(props.anchor);
    const positions = props.itemKeys && new Map(props.itemKeys.map((key,i)=>[`key:${JSON.stringify(key)}`,i]));
    const states = positions
      ? new Map([...(old?.states ?? [])].flatMap(([,state])=>positions.has(state.key) ? [[positions.get(state.key),state]] : []))
      : new Map([...(old?.states ?? [])].filter(([i])=>i<props.count));
    result.list = { count: props.count, previousCount: old?.count ?? props.count, anchor,
      anchorChanged: Boolean(old && old.count === props.count && anchor !== null && anchor !== old.anchor),
      anchorCache: old?.cache ?? null,
      states, cache: new Map(), window: old?.window ? { ...old.window, start: Math.min(old.window.start, props.count), end: Math.min(old.window.end, props.count) } : null };
  } else if (!leaf) {
    const keys = new Set();
    result.children = flat.map((child, position) => {
      const key = child?.[brand] && arena[child[brand][1]]?.[1]?.key;
      if (key !== undefined && typeof key !== 'string') throw Error('UI_KEY: expected string');
      if (key !== undefined && keys.has(key)) throw Error('UI_KEY: duplicate sibling');
      if (key !== undefined) keys.add(key);
      const part = key === undefined ? `position:${position}` : `key:${JSON.stringify(key)}`;
      return resolveNode(child, `${identity}/${tag}/${part}`, depth + 1, tag, typography, buttonAncestor || tag === 'button' || tag === 'checkbox');
    }).filter(child => child !== null);
  }
  return result;
}

function copyTree(node) {
  // Encoding mutates list windows/maps only; ordinary resolved nodes can be shared.
  let children = node.children;
  for (let i = 0; i < children.length; i++) {
    const child = copyTree(node.children[i]);
    if (child === node.children[i]) continue;
    if (children === node.children) children = children.slice();
    children[i] = child;
  }
  if (node.list) return { ...node, children,
    list: { ...node.list, states: new Map(node.list.states), cache: new Map(node.list.cache) } };
  return children === node.children ? node : { ...node, children };
}

function retainCells(cells) {
  for (const [key, cell] of cells) {
    candidate.locals.set(key, cell);
    for (const collector of localCollectors) collector.set(key, cell);
  }
}

function encodeTree(tree, windows) {
  const output = []; let count = 0, plotWork = 0, animationBytes = 0, itemKeyBytes = 0;
  const write = (node, parent, depth) => {
    if (depth > 128 || ++count > 4096) throw Error('UI_LIMIT: candidate too large');
    const { tag, props, identity } = node, encoded = [];
    for (const [name, value] of Object.entries(props.style ?? {})) {
      if (name === 'animation') {
        if (value !== null) {
          const payload = JSON.stringify(value);
          animationBytes += utf8Bytes(payload, 32768);
          if (animationBytes > 262144) throw Error('UI_ANIMATION: page payload budget exceeded');
          encoded.push(fields.animation,payload);
        }
      }
      else if (name === 'height' && value === 'auto') encoded.push(82,1);
      else if (has(dimensionPercentFields,name) && typeof value === 'string') encoded.push(dimensionPercentFields[name],Number(value.slice(0,-1)));
      else if (name === 'boxShadow') encoded.push(fields.boxShadow, value === 'none' ? value : JSON.stringify(value));
      else if (name === 'backgroundGradient') encoded.push(fields.backgroundGradient, value === 'none' ? value : JSON.stringify(value));
      else if (name === 'offset') encoded.push(fields.offsetX, value.x, fields.offsetY, value.y);
      else encoded.push(fields[name], value);
    }
    if (node.focusedStyle) {
      const focused = [];
      for (const [name, value] of Object.entries(node.focusedStyle)) focused.push(fields[name], ['boxShadow','backgroundGradient'].includes(name) && value !== 'none' ? JSON.stringify(value) : value);
      encoded.push(98, JSON.stringify(focused));
    }
    if (props.enabled !== undefined) encoded.push(buttonProps.enabled, +props.enabled);
    if (node.callback) { candidate.callbacks.set(node.callback[0], {callback:node.callback[1],text:false}); encoded.push(buttonProps.onClick, node.callback[0]); }
    if ((tag === 'button' || tag === 'checkbox') && props.accessibilityLabel !== undefined) encoded.push(buttonProps.accessibilityLabel,props.accessibilityLabel);
    if (tag === 'vector') encoded.push(vectorField,JSON.stringify({viewBox:props.viewBox ?? [0,0,24,24],paths:props.paths,...(props.fit === undefined ? {} : {fit:props.fit})}));
    if (tag === 'grid') encoded.push(gridProps.columns,props.columns);
    if (tag === 'plot') {
      const count = props.values.length;
      const points = (count - 1) * (props.smooth ? 8 : 1) + 1;
      plotWork += props.kind === 'pie' ? 256 + count : props.kind === 'line'
        ? points * (props.area ? 2 : 1) + 2 * count + (props.area ? 2 : 0) : 8 * count;
      if (plotWork > 16384) throw Error('UI_LIMIT: plot geometry budget exceeded');
      encoded.push(plotProps.values,JSON.stringify(props.values),plotProps.kind,props.kind,plotProps.colors,JSON.stringify(props.colors),plotProps.accessibilityLabel,props.accessibilityLabel);
      for (const name of ['smooth','area']) if (props[name] !== undefined) encoded.push(plotProps[name],+props[name]);
      for (const name of ['domainMin','domainMax','markerEmphasisScale']) if (props[name] !== undefined) encoded.push(plotProps[name],props[name]);
      if (props.pieLayout !== undefined) encoded.push(plotProps.pieLayout, JSON.stringify(props.pieLayout));
      if (props.selectedIndex !== undefined) encoded.push(plotProps.selectedIndex, props.selectedIndex);
      if (node.plotSelect) {
        candidate.callbacks.set(node.plotSelect[0], {callback:node.plotSelect[1],number:true});
        encoded.push(plotProps.onSelect,node.plotSelect[0]);
      }
    }
    if (tag === 'text' && props.lines !== undefined) encoded.push(textProps.lines,props.lines);
    if (tag === 'slider') {
      for (const name of ['value','min','max','step','accessibilityLabel']) encoded.push(sliderProps[name],props[name]);
      if (node.numberChange) { candidate.callbacks.set(node.numberChange[0],{callback:node.numberChange[1],number:true});encoded.push(sliderProps.onChange,node.numberChange[0]); }
    }
    if (tag === 'radio' && props.accessibilityLabel !== undefined) encoded.push(radioProps.accessibilityLabel,props.accessibilityLabel);
    if (tag === 'checkbox' || tag === 'switch' || tag === 'radio-option') {
      encoded.push(checkProps.value, +(props.value ?? false));
      if (node.change) { candidate.callbacks.set(node.change[0], {callback:node.change[1],text:false}); encoded.push(checkProps.onChange,node.change[0]); }
    }
    for (const [field, handle, callback] of node.reaches ?? []) {
      candidate.callbacks.set(handle, {callback,text:false,isolated:field===imageProps.onLoad || field===imageProps.onError}); encoded.push(field, handle);
    }
    if (tag === 'input' || tag === 'textarea' || tag === 'search') {
      if (props.accessibilityLabel !== undefined) {
        if (typeof props.accessibilityLabel !== 'string' || !props.accessibilityLabel.trim() || utf8Bytes(props.accessibilityLabel,1025)>1024 || props.accessibilityLabel.includes('\0')) throw Error('UI_INPUT: invalid accessibility label');
        encoded.push(buttonProps.accessibilityLabel,props.accessibilityLabel);
      }
      encoded.push(inputProps.value, props.value ?? '');
      for (const name of ['placeholder','secure','keyboard','autoFocus','submitOnEnter']) if (props[name] !== undefined) encoded.push(inputProps[name],typeof props[name] === 'boolean' ? +props[name] : props[name]);
      for (const [field,handle,callback] of node.inputs) {
        candidate.callbacks.set(handle,{callback,text:true}); encoded.push(field,handle);
      }
    }
    if (tag === 'image') {
      encoded.push(imageProps.src, props.src);
      if (props.fit !== undefined) encoded.push(imageProps.fit, props.fit);
      if (props.alt !== undefined) encoded.push(imageProps.alt, props.alt);
      if (props.retryKey !== undefined) encoded.push(imageProps.retryKey, props.retryKey);
    }
    if (node.rowIndex !== undefined) encoded.push(listFields.rowIndex, node.rowIndex);
    let window, seededAnchor = false;
    if (node.list) {
      window = windows[identity] ?? node.list.window ?? { start: 0, end: Math.min(1, props.count), leading: 0, trailing: Math.max(0, props.count - 1) * 64 };
      if (!Number.isSafeInteger(window.start) || !Number.isSafeInteger(window.end) || window.start < 0 ||
          window.end < window.start || window.end > Math.max(props.count, node.list.previousCount) ||
          ![window.leading, window.trailing].every(v => Number.isFinite(v) && v >= 0 && v <= 1638400000)) throw Error('UI_LIST: invalid window');
      // A full render may shrink the list before native can reconcile its old window.
      if (window.end > props.count) window = { start: 0, end: Math.min(1, props.count), leading: 0, trailing: Math.max(0, props.count - 1) * 64 };
      const target = props.anchor === 'end' ? props.count - 1 : Math.min(props.anchor?.index ?? 0, props.count - 1);
      if (node.list.anchorChanged && props.count > 0 && (target < window.start || target >= window.end)) {
        // Keep old geometry only until native chooses the destination window.
        // These rows cannot be committed for an anchor outside this window.
        node.list.cache = new Map([...node.list.anchorCache].filter(([i,row]) =>
          props.itemKeys === undefined || row.props.key === `key:${JSON.stringify(props.itemKeys[i])}`));
        seededAnchor = true;
      }
      node.list.anchorChanged = false;
      node.list.anchorCache = null;
      node.list.previousCount = props.count;
      node.list.window = { ...window };
      encoded.push(listFields.listId, identity, listFields.listCount, props.count,
        listFields.leading, window.leading, listFields.trailing, window.trailing);
      if (props.itemKeys !== undefined) {
        const payload = JSON.stringify(props.itemKeys);
        itemKeyBytes += utf8Bytes(payload,262144);
        if (itemKeyBytes > 262144) throw Error('UI_LIST: itemKeys page budget exceeded');
        encoded.push(listFields.itemKeys,payload);
      }
      if (props.anchor !== undefined) encoded.push(listFields.anchor, JSON.stringify(props.anchor));
      candidate.lists.set(identity, node.list);
    }
    const index = output.length;
    output.push(tags[tag], parent, props.key ?? '', props.testId ?? '', node.text, encoded.length / 2, ...encoded);
    if (!node.list) { for (const child of node.children) write(child, index, depth + 1); return; }
    const visibleKeys = new Set();
    for (const i of node.list.cache.keys()) if (i < window.start || i >= window.end) node.list.cache.delete(i);
    for (let i = window.start; i < window.end; i++) {
      const cells = new Map(); localCollectors.push(cells);
      try {
        let row = node.list.cache.get(i), key = node.list.states.get(i)?.key;
        if (!row) {
          const suppliedKey = props.itemKeys?.[i];
          const previous = scope; scope = suppliedKey === undefined ? `${identity}/row:${i}` : `${identity}/row/key:${JSON.stringify(suppliedKey)}`;
          try {
            const value = props.row(i);
            const declared = value?.[brand] && arena[value[brand][1]]?.[1]?.key;
            if (declared !== undefined && typeof declared !== 'string') throw Error('UI_KEY: expected string');
            if (suppliedKey === undefined && declared !== undefined && cells.size > 0)
              throw Error('UI_LIST: keyed row factory hooks require itemKeys; alternatively move hooks into a keyed component');
            if (suppliedKey !== undefined && declared !== undefined && suppliedKey !== declared) throw Error('UI_LIST: row key does not match itemKeys');
            const rowKey = suppliedKey ?? declared;
            key = rowKey === undefined ? `index:${i}` : `key:${JSON.stringify(rowKey)}`;
            scope = `${identity}/row/${key}`;
            const style = {};
            if (props.height) {
              const height = props.height(i);
              if (!Number.isFinite(height) || height < 0 || height > 16384) throw Error('UI_LIST: invalid row height');
              style.height = height;
            }
            row = { tag: 'column', props: { key, style }, identity: scope,
              rowIndex: i, text: '', children: [resolveNode(value, scope, depth + 2, null, node.typography)].filter(child => child !== null) };
          } finally { scope = previous; }
        } else {
          row = copyTree(row);
          retainCells(node.list.states.get(i).locals);
        }
        if (visibleKeys.has(key)) throw Error('UI_KEY: duplicate list row');
        visibleKeys.add(key);
        write(row, index, depth + 1);
        node.list.cache.set(i, row);
        node.list.states.set(i, { key, locals: cells });
      } finally { localCollectors.pop(); }
    }
    for (const state of node.list.states.values()) retainCells(state.locals);
    // Even overlapping destination rows must resolve against this render's props.
    if (seededAnchor) node.list.cache.clear();
  };
  write(tree, -1, 0);
  return output;
}

function runCandidate(view, lookup, build) {
  if (rendering) throw Error('UI_RENDER: recursive render');
  arena = []; epoch++; rendering = true; currentView = view; lookupLocals = lookup;
  candidate = { locals: new Map(), declarations: new Set(), callbacks: new Map(), lists: new Map(), resources: new Map(), resourceNames: new Set(), view };
  try { scope = 'root'; return build(); }
  catch (error) { discard(); throw error; }
  finally { arena = []; scope = null; currentView = null; lookupLocals = null; localCollectors = []; rendering = false; }
}

export function prepare(render, view, windows = {}) {
  return runCandidate(view, null, () => {
    candidate.tree = resolveNode(render(view), 'root');
    if (!candidate.tree) throw Error('UI_ROOT: page must return one layout node');
    candidate.baseLocals = new Map(candidate.locals);
    return encodeTree(candidate.tree, windows);
  });
}

// Native may refine an uncommitted first window after measuring it. The page render
// and ordinary components remain untouched; a failed candidate never replaces committed rows.
export function prepareWindows(windows) {
  const base = candidate ?? committed;
  if (!base) throw Error('UI_LIST: no retained description');
  return runCandidate(base.view, base.locals, () => {
    candidate.resources = new Map(base.resources);
    candidate.tree = copyTree(base.tree);
    candidate.baseLocals = new Map(base.baseLocals);
    retainCells(candidate.baseLocals);
    return encodeTree(candidate.tree, windows);
  });
}

export function commit() {
  if (!candidate) throw Error('UI_COMMIT: no candidate');
  const previous = committed;
  for (const cell of candidate.locals.values()) if (updates.has(cell)) cell.value = updates.get(cell);
  locals = candidate.locals; callbacks = candidate.callbacks;
  committed = candidate; candidate = null; updates.clear();
  const retained = new Set(locals.values()), next = new Map(), definitions = new Map();
  for (const [cell, resource] of ownedResources) if (!retained.has(cell)) resource.dispose();
  for (const cell of retained) {
    const entry = committed.resources.get(cell) ?? previous?.resources.get(cell);
    if (!entry) continue;
    definitions.set(cell, entry);
    next.set(cell, entry.resource);
    entry.resource.commit(entry.definition);
  }
  committed.resources = definitions;
  ownedResources = next;
}
export function discard() { candidate = null; updates.clear(); }
export function reset() {
  if (rendering) throw Error('UI_RENDER: reset during render');
  discard(); committed = null; locals.clear(); callbacks.clear();
  for (const resource of ownedResources.values()) resource.dispose();
  ownedResources.clear();
  cancelActions(); actionErrors = [];
}
export function dispatch(handle, value) {
  if (rendering) throw Error('UI_RENDER: dispatch during render');
  const entry = callbacks.get(handle);
  if (!entry) return false;
  if (entry.number ? typeof value!=='number' || !Number.isFinite(value) || Math.abs(value)>1e9 : entry.text ? !validInputText(value) : value !== undefined) return false;
  const previous = entry.isolated ? new Map(updates) : undefined;
  try {
    const epoch = actionEpoch;
    const result = entry.text || entry.number ? entry.callback(value) : entry.callback();
    if (result && typeof result.then === 'function') Promise.resolve(result).catch(error => {
      if (epoch === actionEpoch && actionErrors.length < 32) actionErrors.push(String(error).slice(0, 2048));
    });
    return true;
  }
  catch (error) {
    updates.clear();
    if (!entry.isolated) throw error;
    for (const [cell, value] of previous) updates.set(cell, value);
    if (actionErrors.length < 32) actionErrors.push(String(error).slice(0, 2048));
    return true;
  }
}
export function describe(render, view) {
  const output = prepare(render, view); commit(); return output;
}

function validateVector(props) {
  if (props.fit !== undefined && !['contain','fill'].includes(props.fit)) throw Error('UI_VECTOR: invalid fit');
  const box = props.viewBox ?? [0,0,24,24];
  if (!Array.isArray(box) || box.length !== 4 || box.some(v => typeof v !== 'number' || !Number.isFinite(v) || Math.abs(v)>16384) || box[2]<1/64 || box[3]<1/64) throw Error('UI_VECTOR: invalid viewBox');
  if (!Array.isArray(props.paths) || !props.paths.length || props.paths.length>32) throw Error('UI_VECTOR: invalid paths');
  for (const path of props.paths) {
    if (!path || typeof path !== 'object' || Array.isArray(path) || Object.keys(path).some(k => !['d','fill','stroke','strokeWidth','strokeDasharray','vectorEffect'].includes(k)) || typeof path.d !== 'string' || !path.d.length || path.d.length>16384) throw Error('UI_VECTOR: invalid path');
    for (const key of ['fill','stroke']) if (path[key] !== undefined && (typeof path[key] !== 'string' || (!['none','currentColor'].includes(path[key]) && !/^#(?:[0-9a-f]{3}|[0-9a-f]{4}|[0-9a-f]{6}|[0-9a-f]{8})$/i.test(path[key])))) throw Error('UI_VECTOR: invalid color');
    if (path.vectorEffect !== undefined && !['none','non-scaling-stroke'].includes(path.vectorEffect)) throw Error('UI_VECTOR: invalid vector effect');
    if (path.strokeDasharray !== undefined && (!Array.isArray(path.strokeDasharray) || path.strokeDasharray.length>16 || path.strokeDasharray.some(v=>typeof v!=='number' || !Number.isFinite(v) || v<1/64 || v>1024))) throw Error('UI_VECTOR: invalid dash pattern');
    if (path.strokeWidth !== undefined && (typeof path.strokeWidth !== 'number' || !Number.isFinite(path.strokeWidth) || path.strokeWidth<0 || path.strokeWidth>1024)) throw Error('UI_VECTOR: invalid stroke width');
  }
  if (JSON.stringify({viewBox:box,paths:props.paths,...(props.fit === undefined ? {} : {fit:props.fit})}).length>32768) throw Error('UI_VECTOR: source too large');
}

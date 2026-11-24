import { immutable, has } from './data.js';
import { takeOverlayReplies } from './overlay.js';
import { bridgeError } from './errors.js';
import { outsideRender } from './state.js';
import { createStream } from './stream.js';
import { createChannel } from './channel.js';
import { createLeaveGuards } from './leave-guard.js';
import { snapActionSettled } from './snap.js';

// Page actions, streams and channels share one bounded queue of plain messages that the
// native session walks into typed Logic events; every reset invalidates what the old page issued.
let actionPage = '';
// Names tooling probes on any object (awaiting, JSON, inspectors, test matchers): unknown, not a failure.
const probes = new Set(['then', 'toJSON', 'constructor', 'prototype', '__proto__', 'toString', 'valueOf', 'inspect', 'nodeType', 'tagName', '$$typeof', 'asymmetricMatch', '_isMockFunction', '@@__IMMUTABLE_ITERABLE__@@', '@@__IMMUTABLE_RECORD__@@']);
/** A frozen catalog that names an action the build did not see; before the catalog arrives a read waits as undefined. */
const catalog = (built, arrived) => new Proxy(Object.freeze(built), {
  get(target, key) {
    if (!arrived || typeof key === 'symbol' || has(target, key) || probes.has(key)) return target[key];
    throw Error(`UI_ACTION: \`${key}\` is not an action of ${actionPage || 'this page'}: the build did not see it`);
  },
});
/** The page the catalog belongs to, for the unknown-action message. */
export function setActionPage(page) { if (typeof page === 'string') actionPage = page; }
let actions = catalog(Object.create(null), false), actionEpoch = 0, nextRequest = 1;
let requests = [], actionErrors = [];
const pendingActions = new Map();
const activeChannels = new Map();
const actionModes = new WeakMap();
const actionNames = new WeakMap();
const closedChannels = Object.freeze({ open() { throw Error('UI_CHANNEL: bridge not ready'); } });
let channels = closedChannels;

export function report(error) {
  if (actionErrors.length < 32) actionErrors.push(String(error).slice(0, 2048));
}

// Loud (a dev build or under automation): the first handler that failed after it returned
// fails the page, as a throw from it would. Quiet: it is logged and the page stays.
let handlerFailure = '', loud = true;
export function setLoudFailures(on) { loud = on === true; }
export function loudFailures() { return loud; }
export function failHandler(message) { if (loud) handlerFailure ||= message.slice(0, 2048); else report(message); }
export function takeHandlerFailure() { const result = handlerFailure; handlerFailure = ''; return result; }

export const leaveGuards = createLeaveGuards(callLeaveHost, report);

/** The current catalog; Views capture it when created. */
export function currentActions() { return { actions, channels }; }

/** The catalog name and mode of an action reference, if it belongs to the current catalog. */
export function actionInfo(action) {
  const name = actionNames.get(action);
  return name !== undefined && has(actions, name) && actions[name] === action ? { name, mode: actionModes.get(action) } : undefined;
}

export function cancelActions() {
  actionEpoch++;
  leaveGuards.reset();
  sentTitle = undefined;
  const pending = [...pendingActions.values()];
  pendingActions.clear(); requests = []; actions = catalog(Object.create(null), false);
  const oldChannels = [...activeChannels.values()]; activeChannels.clear();
  channels = closedChannels;
  for (const { reject } of pending) reject(bridgeError('BRIDGE_CANCELED', 'Page reset'));
  for (const entry of oldChannels) {
    const error = bridgeError('BRIDGE_CANCELED', 'Page reset');
    entry.reject(error); entry.channel.reject(error);
  }
}

export function resetActions() { cancelActions(); actionErrors = []; handlerFailure = ''; }

function admit(what) {
  if (pendingActions.size + activeChannels.size >= 256 || requests.length >= 256) throw Error(`UI_LIMIT: too many pending ${what}`);
}

// An action's promise knows whether its caller observed it (await, then, catch, finally).
class ActionPromise extends Promise {
  static get [Symbol.species]() { return Promise; }
  then(onFulfilled, onRejected) {
    this.handled = true;
    return super.then(onFulfilled, onRejected);
  }
}

const kindOf = value => value === null ? 'null' : typeof value === 'number' ? String(value)
  : typeof value !== 'object' ? typeof value : Array.isArray(value) ? 'array' : value.constructor?.name ?? 'object';

// JSON's rule: an `undefined` field is left out and an `undefined` element is `null`.
function payload(value, refuse, path, depth, budget, ancestors) {
  if (depth > 64) throw Error('UI_LOCAL: value too deep');
  if ((budget.left -= typeof value === 'string' ? value.length + 2 : 8) < 0) throw Error(`UI_LIMIT: ${budget.what} payload too large`);
  if (value === null || typeof value === 'string' || typeof value === 'boolean') return value;
  if (typeof value === 'number' && Number.isFinite(value)) return value;
  const array = Array.isArray(value);
  const plain = !array && value && typeof value === 'object' && Object.getPrototypeOf(value) === Object.prototype;
  if (array || plain) {
    const here = path ? (path.startsWith('[') ? `argument${path}` : `argument.${path}`) : 'argument';
    if (ancestors.has(value)) {
      const code = budget.what === 'action' ? 'UI_ACTION' : 'UI_CHANNEL';
      throw Error(`${code}: ${budget.who} payload field ${here} refers back to ${ancestors.get(value)}; send a value without cycles`);
    }
    ancestors.set(value, here);
    try {
      if (array) return Object.freeze(Array.from(value, (v, i) =>
        v === undefined ? null : payload(v, refuse, `${path}[${i}]`, depth + 1, budget, ancestors)));
      const out = {};
      for (const [k, v] of Object.entries(value)) {
        if (v === undefined) continue;
        budget.left -= k.length + 3;
        out[k] = payload(v, refuse, path ? `${path}.${k}` : k, depth + 1, budget, ancestors);
      }
      return Object.freeze(out);
    } finally { ancestors.delete(value); }
  }
  throw refuse(path, kindOf(value));
}
const bounded = (value, what, who) => payload(value, (path, kind) =>
  Error(`${what === 'action' ? 'UI_ACTION' : 'UI_CHANNEL'}: ${who} payload ${path ? `field ${path}` : 'value'} is not JSON (${kind})`),
  '', 0, { left: 65536, what, who }, new Map());

// Native supplies the same compiled action catalog used by WebView pages, after ready.
export function configureActions(metadata) {
  outsideRender('configure actions');
  const names = metadata?.__names, modes = metadata?.__modes;
  if (!Array.isArray(names) || names.length > 256 || !modes || typeof modes !== 'object' ||
      names.some(name => typeof name !== 'string' || !name || name.length > 256 || !has(modes, name) || !['call', 'stream'].includes(modes[name])) ||
      new Set(names).size !== names.length) throw Error('UI_ACTION: invalid action catalog');
  cancelActions();
  const epoch = actionEpoch, built = Object.create(null);
  const reportLive = cause => { if (epoch === actionEpoch) report(cause); };
  for (const name of names) built[name] = (...args) => {
    outsideRender('action');
    if (epoch !== actionEpoch) throw bridgeError('BRIDGE_CANCELED', 'Stale page action');
    if (args.length > 1) throw Error('UI_ACTION: expected at most one JSON payload');
    admit('actions');
    const params = args.length && args[0] !== undefined ? bounded(args[0], 'action', name) : undefined;
    const id = `ui-action:${nextRequest++}`;
    const encoded = { kind: 'req', id, method: name, params };
    if (modes[name] === 'stream') {
      const stream = createStream(id, cause => {
        if (!pendingActions.has(id)) return;
        // Reserve a bounded cancellation slot for every admitted request.
        if (requests.length >= 512) throw Error('UI_LIMIT: too many pending cancellations');
        requests.push({ kind: 'cancel', id });
        pendingActions.delete(id);
        stream.reject(cause ?? bridgeError('BRIDGE_CANCELED', 'Stream canceled'));
      }, reportLive, bridgeError);
      pendingActions.set(id, { ...stream, sequence: -1 });
      requests.push(encoded);
      return stream.handle;
    }
    let settle;
    const promise = new ActionPromise((resolve, reject) => { settle = { resolve, reject }; });
    pendingActions.set(id, {
      resolve: settle.resolve,
      reject(cause) {
        settle.reject(cause);
        // Nobody awaited or caught it: the page log shows it instead of the failure vanishing.
        Promise.resolve().then(() => {
          if (!promise.handled && cause?.code !== 'BRIDGE_CANCELED') {
            reportLive(`Unhandled rejection of action '${name}': ${cause?.message ?? String(cause)}`);
          }
        });
      },
    });
    Promise.prototype.then.call(promise, undefined, () => {});
    requests.push(encoded);
    return promise;
  };
  for (const name of names) { actionModes.set(built[name], modes[name]); actionNames.set(built[name], name); }
  actions = catalog(built, true);
  channels = Object.freeze({
    open(topic, params) {
      outsideRender('channel');
      if (epoch !== actionEpoch) throw bridgeError('BRIDGE_CANCELED', 'Stale page channel');
      if (typeof topic !== 'string' || !has(built, topic) || topic.includes('.') || modes[topic] !== 'call')
        throw Error('UI_CHANNEL: expected a page channel handler');
      admit('channels');
      const id = `ui-channel:${nextRequest++}`;
      const encoded = { kind: 'ch.open', id, topic, params: params === undefined ? undefined : bounded(params, 'channel', topic) };
      let outgoing = 0;
      const channel = createChannel(id, payload => {
        outsideRender('channel send');
        const entry = activeChannels.get(id);
        if (epoch !== actionEpoch || !entry?.open) throw bridgeError('STREAM_CLOSED', 'Channel is closed');
        if (requests.length >= 256) throw Error('UI_LIMIT: too many pending channel frames');
        requests.push({ kind: 'ch.data', id, seq: outgoing, payload: bounded(payload, 'channel', topic) });
        outgoing++;
      }, (code, reason, cause) => {
        outsideRender('channel close');
        if (!activeChannels.has(id)) return;
        if (typeof code !== 'string' || code.length > 256 || typeof reason !== 'string' || reason.length > 2048)
          throw Error('UI_CHANNEL: invalid close reason');
        if (requests.length >= 512) throw Error('UI_LIMIT: too many pending cancellations');
        requests.push({ kind: 'ch.close', id, code, reason });
        activeChannels.delete(id);
        if (cause) channel.reject(cause); else channel.finish(code, reason, true);
      }, reportLive, bridgeError);
      const promise = new Promise((resolve, reject) => activeChannels.set(id, { channel, resolve, reject, open: false, sequence: -1 }));
      promise.catch(() => {});
      requests.push(encoded);
      return promise;
    },
  });
}

function callLeaveHost(method, params) {
  outsideRender('leave control');
  if (!['navigation.setLeaveGuard', 'navigation.leave', 'navigation.setTitle'].includes(method)) throw Error('UI_LEAVE: invalid control');
  admit('actions');
  const id = `ui-action:${nextRequest++}`;
  const promise = new Promise((resolve, reject) => pendingActions.set(id, { resolve, reject }));
  promise.catch(() => {});
  requests.push({ kind: 'req', id, method: `host.${method}`, params });
  return promise;
}

/** A `<link to>` press: Logic navigates; a failure reports like a failed action. */
export function requestNavigation(to) {
  outsideRender('link');
  admit('actions');
  const id = `ui-action:${nextRequest++}`;
  const promise = new Promise((resolve, reject) => pendingActions.set(id, { resolve, reject }));
  promise.catch(report);
  requests.push({ kind: 'req', id, method: 'nav.go', params: bounded(to, 'link') });
}

// The `<page title>` last sent to the host; `undefined` until the page first declares one.
let sentTitle;
/** Sends a changed `<page title>` to the host navigation bar; `null` restores the configured title. */
export function syncPageTitle(title) {
  const next = typeof title === 'string' ? title : null;
  if (next === sentTitle || (sentTitle === undefined && next === null)) return;
  sentTitle = next;
  try { callLeaveHost('navigation.setTitle', { title: next }).catch(report); } catch (error) { report(error); }
}

export function requestLeave(reason) {
  outsideRender('leave request');
  leaveGuards.request(reason);
}

/** What the View sends Logic since the last call, in order: plain objects the native session walks. */
export function takeViewEvents() { const result = requests.concat(takeOverlayReplies()); requests = []; return result; }

// The same messages as frame text, for the package's JS tests.
const frameCaps = { req: m => m.method.startsWith('host.') ? 'host' : 'page', 'ch.open': () => 'page' };
export function takeActionRequests() {
  return takeViewEvents().map(message => {
    const frame = { v: 2, ...message };
    if (frameCaps[message.kind]) frame.cap = frameCaps[message.kind](message);
    return JSON.stringify(frame);
  });
}

// Test automation calls a page action exactly as `view.actions[name](payload)` would.
let automationResults = [];
const automationError = (call, error) => automationResults.push({ call, error: {
  code: typeof error?.code === 'string' ? error.code : 'UI_ACTION', message: String(error?.message ?? error) } });
export function automationAction(call, name, payload) {
  const action = typeof name === 'string' && has(actions, name) ? actions[name] : undefined;
  if (!action) return automationError(call, { code: 'UI_ACTION', message: `no page action ${JSON.stringify(name)}` });
  if (actionModes.get(action) !== 'call') return automationError(call, { code: 'UI_ACTION', message: `${name} streams; automation calls only call actions` });
  try {
    (payload === undefined ? action() : action(payload)).then(
      value => automationResults.push({ call, value: value === undefined ? null : value }),
      error => automationError(call, error));
  } catch (error) { automationError(call, error); }
}
export function takeAutomationResults() { const result = automationResults; automationResults = []; return JSON.stringify(result); }
export function takeActionErrors() { const result = actionErrors; actionErrors = []; return result; }
/** The serial the next action or request takes; `actionPending(serial)` while it awaits Logic. */
export function actionSerial() { return nextRequest; }
export function actionPending(serial) { return pendingActions.has(`ui-action:${serial}`); }
export function hasActionEpoch(epoch) { return epoch === actionEpoch; }
export function actionEpochNow() { return actionEpoch; }

const failure = frame => bridgeError(
  typeof frame.error?.code === 'string' ? frame.error.code : 'BRIDGE_INTERNAL_ERROR',
  typeof frame.error?.message === 'string' ? frame.error.message : undefined,
  frame.error?.data === undefined ? undefined : immutable(frame.error.data));

export function receiveActionResponse(frame) {
  outsideRender('response');
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
  // Validate before settling so a malformed reply cannot consume the live request.
  const value = frame.ok ? immutable(frame.result ?? null) : failure(frame);
  pendingActions.delete(frame.id);
  if (frame.ok) pending.resolve(value); else pending.reject(value);
  snapActionSettled(Number(frame.id.slice('ui-action:'.length)), frame.ok);
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
      const cause = failure(frame);
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

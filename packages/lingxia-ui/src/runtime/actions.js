import { immutable, has } from './data.js';
import { takeOverlayReplies } from './overlay.js';
import { bridgeError } from './errors.js';
import { outsideRender } from './state.js';
import { createStream } from './stream.js';
import { createChannel } from './channel.js';
import { createLeaveGuards } from './leave-guard.js';

// Page actions, streams and channels share one bounded request queue that the
// native session drains; every reset invalidates what the old page issued.
let actions = Object.freeze(Object.create(null)), actionEpoch = 0, nextRequest = 1;
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

export const leaveGuards = createLeaveGuards(callLeaveHost, report);

/** The current catalog; Views capture it when created. */
export function currentActions() { return { actions, channels }; }

/** The catalog name and mode of an action reference, if it belongs to the current catalog. */
export function actionInfo(action) {
  const name = actionNames.get(action);
  return name !== undefined && actions[name] === action ? { name, mode: actionModes.get(action) } : undefined;
}

export function cancelActions() {
  actionEpoch++;
  leaveGuards.reset();
  const pending = [...pendingActions.values()];
  pendingActions.clear(); requests = []; actions = Object.freeze(Object.create(null));
  const oldChannels = [...activeChannels.values()]; activeChannels.clear();
  channels = closedChannels;
  for (const { reject } of pending) reject(bridgeError('BRIDGE_CANCELED', 'Page reset'));
  for (const entry of oldChannels) {
    const error = bridgeError('BRIDGE_CANCELED', 'Page reset');
    entry.reject(error); entry.channel.reject(error);
  }
}

export function resetActions() { cancelActions(); actionErrors = []; }

function admit(what) {
  if (pendingActions.size + activeChannels.size >= 256 || requests.length >= 256) throw Error(`UI_LIMIT: too many pending ${what}`);
}

function encodeFrame(frame, what) {
  const encoded = JSON.stringify(frame);
  if (encoded.length > 65536) throw Error(`UI_LIMIT: ${what} payload too large`);
  return encoded;
}

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
    const params = args.length && args[0] !== undefined ? immutable(args[0]) : undefined;
    const id = `ui-action:${nextRequest++}`;
    const encoded = encodeFrame({ v: 2, kind: 'req', id, method: name, cap: 'page', ...(params === undefined ? {} : { params }) }, 'action');
    if (modes[name] === 'stream') {
      const stream = createStream(id, cause => {
        if (!pendingActions.has(id)) return;
        // Reserve a bounded cancellation slot for every admitted request.
        if (requests.length >= 512) throw Error('UI_LIMIT: too many pending cancellations');
        requests.push(JSON.stringify({ v: 2, kind: 'cancel', id }));
        pendingActions.delete(id);
        stream.reject(cause ?? bridgeError('BRIDGE_CANCELED', 'Stream canceled'));
      }, reportLive, bridgeError);
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
  for (const name of names) { actionModes.set(built[name], modes[name]); actionNames.set(built[name], name); }
  actions = Object.freeze(built);
  channels = Object.freeze({
    open(topic, params) {
      outsideRender('channel');
      if (epoch !== actionEpoch) throw bridgeError('BRIDGE_CANCELED', 'Stale page channel');
      if (typeof topic !== 'string' || !has(built, topic) || topic.includes('.') || modes[topic] !== 'call')
        throw Error('UI_CHANNEL: expected a page channel handler');
      admit('channels');
      const id = `ui-channel:${nextRequest++}`;
      const encoded = encodeFrame({ v: 2, kind: 'ch.open', id, topic, cap: 'page', ...(params === undefined ? {} : { params: immutable(params) }) }, 'channel');
      let outgoing = 0;
      const channel = createChannel(id, payload => {
        outsideRender('channel send');
        const entry = activeChannels.get(id);
        if (epoch !== actionEpoch || !entry?.open) throw bridgeError('STREAM_CLOSED', 'Channel is closed');
        if (requests.length >= 256) throw Error('UI_LIMIT: too many pending channel frames');
        requests.push(encodeFrame({ v: 2, kind: 'ch.data', id, seq: outgoing, payload: immutable(payload) }, 'channel'));
        outgoing++;
      }, (code, reason, cause) => {
        outsideRender('channel close');
        if (!activeChannels.has(id)) return;
        if (typeof code !== 'string' || code.length > 256 || typeof reason !== 'string' || reason.length > 2048)
          throw Error('UI_CHANNEL: invalid close reason');
        if (requests.length >= 512) throw Error('UI_LIMIT: too many pending cancellations');
        requests.push(JSON.stringify({ v: 2, kind: 'ch.close', id, code, reason }));
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
  if (!['navigation.setLeaveGuard', 'navigation.leave'].includes(method)) throw Error('UI_LEAVE: invalid control');
  admit('actions');
  const id = `ui-action:${nextRequest++}`;
  const promise = new Promise((resolve, reject) => pendingActions.set(id, { resolve, reject }));
  promise.catch(() => {});
  requests.push(JSON.stringify({ v: 2, kind: 'req', id, method: `host.${method}`, cap: 'host', params }));
  return promise;
}

export function requestLeave(reason) {
  outsideRender('leave request');
  leaveGuards.request(reason);
}

export function takeActionRequests() { const result = requests.concat(takeOverlayReplies()); requests = []; return result; }

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

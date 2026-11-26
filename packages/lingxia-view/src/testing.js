// Renders a compiled page in Node through the globals the native session calls, so a test runs the
// runtime copy a device runs. No layout or semantics: roles and names come from the native projection.
import { existsSync, readFileSync, statSync } from 'node:fs';
import { mkdtemp, readFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { dirname, join, relative, resolve } from 'node:path';
import { createContext, runInContext } from 'node:vm';
import { tags, field, maxField } from './generated/wire.js';
import { scales, scaleDefaults, scaledFields } from './generated/contract.js';

const tagNames = Object.fromEntries(Object.entries(tags).map(([name, id]) => [id, name]));
const fieldNames = Object.fromEntries(Object.entries(field).map(([name, id]) => [id, name]));
const states = new Set(['focusStyle', 'focusVisibleStyle', 'pressedStyle', 'hoverStyle', 'checkedStyle', 'hasCheckedStyle', 'sideTopStyle', 'sideBottomStyle', 'sideStartStyle', 'sideEndStyle']);

// A `[slot, times, plus]` step reference, as theme-scaled lengths, durations and easings reach the wire.
const isStep = value => Array.isArray(value) && value.length === 3 && Number.isInteger(value[0]) && scales[value[0]] !== undefined
  && scaleDefaults[value[0]] !== null && value.every(Number.isFinite);

// A list on a wire field that takes no step stays as it is, so three numbers there are never read as one.
const scaled = new Set(Object.values(scaledFields).flat());
const fixed = key => Object.hasOwn(field, key) && !scaled.has(key);

/** `value` with every theme step resolved as a host does under the built-in pack, at any depth. */
export function points(value) {
  if (isStep(value)) {
    const base = scaleDefaults[value[0]];
    return Array.isArray(base) ? base : value[1] * base + value[2];
  }
  if (Array.isArray(value)) return value.map(points);
  if (value && typeof value === 'object' && Object.getPrototypeOf(value) === Object.prototype)
    return Object.fromEntries(Object.entries(value).map(([key, item]) => [key, fixed(key) && Array.isArray(item) ? item : points(item)]));
  return value;
}

/** Loads a compiled page module into its own context, announcing what a current native host does. */
export function installHost(source) {
  const host = createContext({
    __lingxiaViewHost: { uiAbi: 1, maxField, features: ['candidate-transactions', 'bounded-jobs'] },
    console, setTimeout, clearTimeout, queueMicrotask,
  });
  // The module's one export list is for the native loader; a script context cannot hold it.
  runInContext(source.replace(/^export\s*\{[^}]*\};?[ \t]*$/m, ''), host);
  if (typeof host.__lingxiaRender !== 'function') throw Error('installHost: the module is not a compiled GPU page');
  return host;
}

/** One describe-and-commit; returns the wire buffer. */
export const describePage = (host, view = {}) =>
  host.__lingxiaDescribe(host.__lingxiaRender, { data: {}, env: {}, ...view });

/** False when the handle is stale or the value is the wrong kind. */
export const dispatch = (host, handle, value, timeStamp) => host.__lingxiaDispatch(handle, value, timeStamp);

export function decode(buffer) {
  const nodes = [], offsets = new Map();
  for (let cursor = 0; cursor < buffer.length;) {
    const [tag, parent, key, testId, text, count] = buffer.slice(cursor, cursor + 6);
    // The buffer's serial number heads it; tests never acknowledge, so no reference follows.
    if (tag === -3) { cursor += 6; continue; }
    const props = {};
    for (let i = 0; i < count; i++) {
      const name = fieldNames[buffer[cursor + 6 + i * 2]], value = buffer[cursor + 7 + i * 2];
      props[name] = states.has(name) ? pairs(JSON.parse(value)) : value;
    }
    const node = { tag: tagNames[tag], key, testId, text, props, children: [], parent: null };
    if (parent >= 0) { node.parent = offsets.get(parent); node.parent.children.push(node); }
    Object.defineProperty(node, 'parent', { enumerable: false });
    offsets.set(cursor, node);
    nodes.push(node);
    cursor += 6 + count * 2;
  }
  return nodes;
}

function pairs(flat) {
  const result = {};
  for (let i = 0; i < flat.length; i += 2) result[fieldNames[flat[i]]] = flat[i + 1];
  return result;
}

/** The first node, depth first, that passes `test`. */
export function find(root, test) {
  if (test(root)) return root;
  for (const child of root.children) { const found = find(child, test); if (found) return found; }
}

/** A node's text with its descendants'. */
export const textOf = node => (node.text ?? '') + node.children.map(textOf).join('');

/**
 * Builds `page` with the View compiler and renders it. `root` is the project the page belongs to,
 * by default the nearest directory above it with a package.json or lxapp.json. `messages` is what
 * the page's `t` reads (relative to `root`): a compiled `index.js`, or a build's `.lingxia/i18n/build`,
 * whose `current` names its copy. Without it `t` has no catalogue.
 */
export async function render(page, view = {}, { root, points: resolved = false, messages = null } = {}) {
  const { buildPages } = await import('../compiler/index.mjs');
  const file = resolve(page);
  root = resolve(root ?? projectOf(file));
  const entry = relative(root, file);
  const out = await mkdtemp(join(tmpdir(), 'lingxia-view-testing-'));
  try {
    // One self-contained module: the test host evaluates it without a chunk loader.
    await buildPages(root, [entry], out, {}, { chunks: false, messages: compiledIndex(root, messages), inputsRecord: join(out, 'gpu-inputs.json') });
    return screen(installHost(await readFile(join(out, entry.replace(/\.[jt]sx?$/, '.js')), 'utf8')), view, resolved);
  } finally { await rm(out, { recursive: true, force: true }); }
}

function compiledIndex(root, messages) {
  if (messages === null) return null;
  const path = resolve(root, messages);
  if (!statSync(path).isDirectory()) return path;
  return join(path, readFileSync(join(path, 'current'), 'utf8').trim(), 'index.js');
}

function projectOf(file) {
  for (let dir = dirname(file); ; dir = dirname(dir)) {
    if (existsSync(join(dir, 'package.json')) || existsSync(join(dir, 'lxapp.json'))) return dir;
    if (dirname(dir) === dir) return dirname(file);
  }
}

/** What a phone reports by default; a test overrides any field through `view.env`. */
export function defaultEnv(overrides = {}) {
  const viewport = overrides.viewport ?? { width: 390, height: 844 };
  const direction = overrides.direction ?? 'ltr';
  const rtl = direction === 'rtl';
  const toLogical = raw => ({
    ...raw,
    inlineStart: rtl ? raw.right : raw.left,
    inlineEnd: rtl ? raw.left : raw.right,
  });
  const baseEdges = { top: 0, right: 0, bottom: 0, left: 0 };
  const insets = toLogical(overrides.insets ?? baseEdges);
  const safeArea = toLogical(overrides.safeArea ?? baseEdges);
  return {
    appearance: 'light', viewport, capsule: null, keyboard: { height: 0 }, pointer: 'coarse', os: 'unknown',
    sizeClass: viewport.width < 600 ? 'compact' : 'regular', language: 'en-US',
    reducedMotion: false, highContrast: false, animatorDurationScale: 1, hourCycle: 'h12', firstWeekday: 7,
    direction, canDock: false, hinge: null, ...overrides, insets, safeArea,
  };
}

function screen(host, view, resolved) {
  let given = view.env ?? {};
  const { channels: served, now: pinned, ...rest } = view;
  let current = { data: {}, ...rest, env: defaultEnv(given) };
  // The test's clock: pinned by `now`, still until `advance`.
  let clock = typeof pinned === 'number' ? pinned : Date.now();
  const bridge = served ? serveChannels(host, served) : null;
  if (bridge) current = { ...current, actions: { ...current.actions, ...bridge.actions }, channels: bridge.channels };
  // With `points`, every node's props read in points, as the built-in pack resolves them.
  const read = () => {
    host.__lingxiaSetNow?.(clock);
    const decoded = decode(describePage(host, current));
    if (resolved) for (const node of decoded) node.props = points(node.props);
    return decoded;
  };
  let nodes = read();
  const refresh = () => { nodes = read(); bridge?.pump(); };
  bridge?.pump();
  const single = (matches, what, onScreen) => {
    if (matches.length === 1) return matches[0];
    if (matches.length === 0) throw Error(`no node has ${what}; on screen: ${onScreen() || 'none'}`);
    throw Error(`${matches.length} nodes have ${what}:\n${matches.map(node => `  ${label(node)} at ${path(node)}`).join('\n')}`);
  };
  // The same element after a re-render, found by its position.
  const again = node => {
    const steps = [];
    for (let at = node; at.parent; at = at.parent) steps.unshift(at.parent.children.indexOf(at));
    return steps.reduce((at, index) => at?.children[index], nodes[0]);
  };
  const fire = (node, prop, value) => {
    const handle = node.props[prop];
    if (handle === undefined || handle === 0) throw Error(`${label(node)} has no ${prop}`);
    if (!dispatch(host, handle, value, clock)) throw Error(`${label(node)} refused ${prop}(${value === undefined ? '' : JSON.stringify(value)})`);
    bridge?.pump();
    refresh();
  };
  const json = (node, prop, payload) => fire(node, prop, JSON.stringify(payload));
  return {
    get root() { return nodes[0]; },
    get nodes() { return nodes; },
    /** The page's runtime, for `dispatch` and the other low-level calls. */
    host,
    wire: () => describePage(host, current),
    getByTestId: id => single(nodes.filter(node => node.testId === id), `data-testid="${id}"`,
      () => list(nodes.filter(node => node.testId).map(node => node.testId))),
    // The innermost node whose text is `text`, so a wrapper is not a second match.
    getByText: text => single(nodes.filter(node => textOf(node) === text && !node.children.some(child => textOf(child) === text)),
      `text ${JSON.stringify(text)}`, () => list(nodes.filter(node => node.text).map(node => JSON.stringify(node.text)))),
    queryAll: test => nodes.filter(test),
    fire,
    press: node => fire(node, 'onClick'),
    type: (node, text) => fire(node, 'onInput', text),
    /** A paste: `onPaste` hears it first; unless it called `preventDefault()`, the plain text goes in as `onInput`, as the host inserts it. */
    paste(node, { text, html, plain = false, selection } = {}) {
      if (typeof text !== 'string') throw Error('paste takes { text, html?, plain?, selection? }');
      const value = node.props.inputValue ?? '';
      const [start, end] = selection ? [selection.start, selection.end] : [value.length, value.length];
      let prevented = false;
      if (node.props.onPaste) {
        const outcome = dispatch(host, node.props.onPaste, JSON.stringify({ text, ...(html !== undefined && !plain ? { html } : {}), selection: [start, end], plain, omitted: [] }), clock);
        if (!outcome) throw Error(`${label(node)} refused onPaste`);
        prevented = outcome === 'prevented';
        bridge?.pump();
        refresh();
      }
      if (!prevented) {
        const inserted = node.tag === 'textarea' ? text.replace(/\r\n?/g, '\n') : text.replace(/\r\n|[\r\n]/g, ' ');
        const target = again(node) ?? node;
        if (target.props.onInput) fire(target, 'onInput', value.slice(0, start) + inserted + value.slice(end));
      }
      return { prevented };
    },
    submit: (node, text = node.props.inputValue ?? '') => fire(node, 'onSubmit', text),
    toggle(node, checked) {
      const was = node.props.checked === 1;
      if (checked !== undefined && was === checked) throw Error(`${label(node)} is already ${checked ? 'checked' : 'unchecked'}`);
      fire(node, 'onCheckedChange');
      const now = again(node);
      if (checked !== undefined && (now?.props.checked === 1) !== checked)
        throw Error(`${label(node)} stayed ${was ? 'checked' : 'unchecked'}: its onChange did not update value`);
    },
    slide: (node, value) => fire(node, 'onSliderChange', value),
    select: (node, value) => fire(node, 'onSelect', value),
    selectPoint: (node, index) => fire(node, 'onPlotSelect', index),
    visible: (node, items) => json(node, 'onVisibleChange', { items, timeStamp: clock }),
    reach: (node, edge) => fire(node, edge === 'start' ? 'onReachStart' : 'onReachEnd'),
    image: (node, outcome) => fire(node, outcome === 'error' ? 'onImageError' : 'onImageLoad'),
    snap: (node, index) => fire(node, 'onSnapChange', String(index)),
    key: (node, key) => fire(node, 'onKeyDown', key),
    selectText: (node, start, end = start) => json(node, 'onSelectionChange', { start, end, caret: null, rects: [] }),
    focus: node => fire(node, 'onFocus'),
    blur: node => fire(node, 'onBlur'),
    layout: (node, { width, height, minContentWidth, maxContentWidth }) => json(node, 'onLayout', { width, height, minContentWidth, maxContentWidth }),
    pointer: (node, { phase, x, y, width, height, pointer = 'touch' }) => json(node, 'onPointer', { phase, x, y, width, height, pointer }),
    drag: (node, event) => json(node, 'onDragEvent', event),
    video: (node, status, events = []) => json(node, 'onVideoEvent', { status, events }),
    webview: (node, events = []) => json(node, 'onWebEvent', { events }),
    /** The Back or Escape a device sends: closes the topmost overlay; false when none was open. */
    escape() { const closed = host.__lingxiaEscape(); refresh(); return closed; },
    /** Moves the clock on by `ms`; a page showing `useNow` re-renders when its step changes. */
    advance(ms) {
      if (!Number.isFinite(ms) || ms < 0) throw Error('advance takes a duration in ms, 0 or more');
      clock += ms;
      if (host.__lingxiaSetNow?.(clock)) refresh();
    },
    get now() { return clock; },
    rerender(next = {}) {
      if (next.channels) throw Error('rerender: channels are served from the first render');
      const { now: moved, ...view } = next;
      if (typeof moved === 'number') clock = moved;
      next = view;
      given = { ...given, ...next.env };
      current = { ...current, ...next, data: { ...current.data, ...next.data }, env: defaultEnv(given) };
      refresh();
    },
    /** With `channels`, the other messages the page sent (`nav.go`, …), unanswered, oldest first. */
    takeRequests: () => bridge?.take() ?? [],
    async settle() {
      for (let round = 0; round < 50; round++) {
        await new Promise(done => setImmediate(done));
        const before = bridge?.count() ?? 0;
        refresh();
        if ((bridge?.count() ?? 0) === before) { await new Promise(done => setImmediate(done)); refresh(); return; }
      }
      throw Error('settle: the page kept sending channel frames after 50 rounds');
    },
  };
}

// Plays Logic's side of the page's channels: each name opens through the page's own bridge, as on a device.
function serveChannels(host, served) {
  const names = Object.keys(served);
  for (const name of names) if (typeof served[name] !== 'function') throw Error(`render: channels.${name} must be a function (channel, params) => void`);
  host.__lingxiaConfigureActions({ __names: names, __modes: Object.fromEntries(names.map(name => [name, 'call'])) });
  const made = runInContext('__lingxiaCreateView({}, {})', host);
  // Frames are parsed inside the page's realm, as the bridge hands them over.
  const receive = frame => {
    host.__lingxiaFrame = JSON.stringify(frame);
    try { runInContext('__lingxiaReceiveActionResponse(JSON.parse(__lingxiaFrame))', host); } finally { delete host.__lingxiaFrame; }
  };
  const open = new Map(), unanswered = [];
  let handled = 0;
  return {
    actions: Object.fromEntries(names.map(name => [name, made.actions[name]])),
    channels: made.channels,
    count: () => handled,
    take: () => unanswered.splice(0),
    pump() {
      for (let messages; (messages = host.__lingxiaTakeViewEvents()).length;) for (const message of messages) {
        handled++;
        const copy = value => value === undefined ? undefined : JSON.parse(JSON.stringify(value));
        if (message.kind === 'ch.open') {
          const listeners = { data: new Set(), close: new Set() };
          let seq = 0;
          open.set(message.id, listeners);
          receive({ v: 2, kind: 'ch.ack', id: message.id, ok: true });
          served[message.topic]({
            send(payload) { if (open.has(message.id)) receive({ v: 2, kind: 'ch.data', id: message.id, seq: seq++, payload: copy(payload) }); },
            close(error) {
              if (!open.delete(message.id)) return;
              receive(error === undefined ? { v: 2, kind: 'ch.close', id: message.id } : { v: 2, kind: 'ch.close', id: message.id, error: copy(error) });
            },
            on(event, listener) {
              if (!listeners[event]) throw Error(`channel.on: "data" or "close", not ${JSON.stringify(event)}`);
              listeners[event].add(listener);
              return () => listeners[event].delete(listener);
            },
          }, copy(message.params));
        } else if (message.kind === 'ch.data') {
          for (const listener of [...open.get(message.id)?.data ?? []]) listener(copy(message.payload));
        } else if (message.kind === 'ch.close') {
          const listeners = open.get(message.id);
          open.delete(message.id);
          const event = message.error === undefined ? {} : { error: copy(message.error) };
          for (const listener of [...listeners?.close ?? []]) listener(event);
        } else unanswered.push(copy(message));
      }
    },
  };
}

// Messages use the element an author wrote: `<view>` travels as the wire's `column`.
const authored = node => node.tag === 'column' ? 'view' : node.tag;
const list = items => [...new Set(items)].slice(0, 20).join(', ') + (new Set(items).size > 20 ? ', …' : '');
const label = node => `<${authored(node)}${node.testId ? ` data-testid="${node.testId}"` : ''}>` +
  (textOf(node) ? ` ${JSON.stringify(textOf(node).slice(0, 40))}` : '');
function path(node) {
  const steps = [];
  for (let at = node; at; at = at.parent) steps.unshift(at.parent ? `${authored(at)}[${at.parent.children.indexOf(at)}]` : authored(at));
  return steps.join(' > ');
}

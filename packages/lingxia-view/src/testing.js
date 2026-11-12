// Renders a compiled page in Node through the globals the native session calls, so a test runs the
// runtime copy a device runs. No layout or semantics: roles and names come from the native projection.
import { existsSync } from 'node:fs';
import { mkdtemp, readFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { dirname, join, relative, resolve } from 'node:path';
import { createContext, runInContext } from 'node:vm';
import { tags, field, maxField } from './generated/wire.js';

const tagNames = Object.fromEntries(Object.entries(tags).map(([name, id]) => [id, name]));
const fieldNames = Object.fromEntries(Object.entries(field).map(([name, id]) => [id, name]));
const states = new Set(['focusStyle', 'pressedStyle', 'hoverStyle']);

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
export const dispatch = (host, handle, value) => host.__lingxiaDispatch(handle, value);

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
 * by default the nearest directory above it with a package.json or lxapp.json.
 */
export async function render(page, view = {}, { root } = {}) {
  const { buildPages } = await import('../compiler/index.mjs');
  const file = resolve(page);
  root = resolve(root ?? projectOf(file));
  const entry = relative(root, file);
  const out = await mkdtemp(join(tmpdir(), 'lingxia-view-testing-'));
  try {
    await buildPages(root, [entry], out);
    return screen(installHost(await readFile(join(out, entry.replace(/\.[jt]sx?$/, '.js')), 'utf8')), view);
  } finally { await rm(out, { recursive: true, force: true }); }
}

function projectOf(file) {
  for (let dir = dirname(file); ; dir = dirname(dir)) {
    if (existsSync(join(dir, 'package.json')) || existsSync(join(dir, 'lxapp.json'))) return dir;
    if (dirname(dir) === dir) return dirname(file);
  }
}

function screen(host, view) {
  let current = { data: {}, env: {}, ...view };
  let nodes = decode(describePage(host, current));
  const refresh = () => { nodes = decode(describePage(host, current)); };
  const single = (matches, what, onScreen) => {
    if (matches.length === 1) return matches[0];
    if (matches.length === 0) throw Error(`no node has ${what}; on screen: ${onScreen() || 'none'}`);
    throw Error(`${matches.length} nodes have ${what}:\n${matches.map(node => `  ${label(node)} at ${path(node)}`).join('\n')}`);
  };
  const fire = (node, prop, value) => {
    const handle = node.props[prop];
    if (handle === undefined || handle === 0) throw Error(`${label(node)} has no ${prop}`);
    if (!dispatch(host, handle, value)) throw Error(`${label(node)} refused ${prop}(${JSON.stringify(value)})`);
    refresh();
  };
  return {
    get root() { return nodes[0]; },
    get nodes() { return nodes; },
    wire: () => describePage(host, current),
    getByTestId: id => single(nodes.filter(node => node.testId === id), `data-testid="${id}"`,
      () => list(nodes.filter(node => node.testId).map(node => node.testId))),
    // The innermost node whose text is `text`, so a wrapper is not a second match.
    getByText: text => single(nodes.filter(node => textOf(node) === text && !node.children.some(child => textOf(child) === text)),
      `text ${JSON.stringify(text)}`, () => list(nodes.filter(node => node.text).map(node => JSON.stringify(node.text)))),
    queryAll: test => nodes.filter(test),
    press: node => fire(node, 'onClick'),
    type: (node, text) => fire(node, 'onInput', text),
    toggle: (node, checked) => fire(node, 'onCheckedChange', checked),
    rerender(next) {
      current = { ...current, ...next, data: { ...current.data, ...next?.data }, env: { ...current.env, ...next?.env } };
      refresh();
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

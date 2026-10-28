import { render } from './state.js';

// JSX produces handles into a per-render arena; a handle from another render is stale.
let arena = [], epoch = 0;
export const brand = Symbol('ui candidate');

export function resetArena() { arena = []; epoch++; }
export function clearArena() { arena = []; }

/** `[tag, props, children, location]` of a handle from the current render. */
export function readNode(node) {
  const [owner, offset] = node[brand];
  if (owner !== epoch || !arena[offset]) throw Error('UI_STALE: candidate from another render');
  return arena[offset];
}

export const isNode = value => Boolean(value && typeof value === 'object' && brand in value);
export const nodeKey = value => isNode(value) ? readNode(value)[1].key : undefined;

export function h(tag, props, ...children) {
  if (!render.active) throw Error('UI_SCOPE: JSX outside render');
  if (arena.length >= 16384) throw Error('UI_LIMIT: candidate arena too large');
  const handle = { [brand]: [epoch, arena.length] };
  const { children: supplied, __loc: location, ...attributes } = props ?? {};
  arena.push([tag, attributes, children.length ? children : supplied === undefined ? [] : [supplied], location]);
  return handle;
}

/** `<>…</>` and `<Fragment key>`: its children stand in its place among its parent's. */
export function Fragment() { throw Error('UI_FRAGMENT: a fragment is not a component'); }
export const isFragment = value => isNode(value) && readNode(value)[0] === Fragment;

const kept = child => child !== null && child !== undefined && typeof child !== 'boolean' && child !== '';

/**
 * The children a parent lays out, with arrays and fragments opened, each with its identity part:
 * `slot` is its index among the authored children, holes (`cond && <X/>` while false, null)
 * included, and `i.j` inside an array or fragment, so a child that appears or disappears leaves
 * its siblings' identity, local state and semantics alone. Inside a keyed fragment a child is
 * named by the fragment's key and its own key or position instead, so the group moves together.
 */
export function entries(children, prefix = '', scope, depth = 0, result = []) {
  if (depth > 128) throw Error('UI_LIMIT: children too deep');
  children.forEach((child, index) => {
    if (result.length >= 16384) throw Error('UI_LIMIT: too many children');
    const slot = prefix ? `${prefix}.${index}` : String(index);
    if (Array.isArray(child)) return entries(child, slot, scope, depth + 1, result);
    if (isFragment(child)) {
      const [, props, inner] = readNode(child);
      if (props.key !== undefined && typeof props.key !== 'string') throw Error('UI_KEY: expected string');
      const named = props.key === undefined ? scope : grouped(scope, slot, props.key);
      return entries(inner, props.key === undefined ? slot : '', named, depth + 1, result);
    }
    if (!kept(child)) return;
    const key = nodeKey(child);
    if (!scope) result.push({ child, slot, key });
    else result.push({ child, key: JSON.stringify([scope, key ?? `#${slot}`]) });
  });
  return result;
}
// A keyed fragment inside another one is named by both keys.
const grouped = (scope, slot, key) => scope === undefined ? key : JSON.stringify([scope, key ?? `#${slot}`]);

// Booleans, nullish values and empty strings render nothing, so `{text && <x/>}` is safe.
export function flatten(children) { return entries(children).map(entry => entry.child); }

export function childrenToArray(children) { return flatten([children]); }

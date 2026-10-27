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

// Booleans, nullish values and empty strings render nothing, so `{text && <x/>}` is safe.
export function flatten(children, depth = 0, result = []) {
  if (depth > 128) throw Error('UI_LIMIT: children too deep');
  for (const child of children) {
    if (result.length >= 16384) throw Error('UI_LIMIT: too many children');
    if (Array.isArray(child)) flatten(child, depth + 1, result);
    else if (child !== null && child !== undefined && typeof child !== 'boolean' && child !== '') result.push(child);
  }
  return result;
}

export function childrenToArray(children) { return flatten([children]); }

// Each kept child's slot, parallel to `flatten(children)`: its index among the authored children,
// holes (`cond && <X/>` while false, null) included, and `i.j` inside nested arrays. A child that
// appears or disappears therefore leaves its siblings' identity, local state and semantics alone.
export function slots(children, prefix = '', result = []) {
  children.forEach((child, index) => {
    const slot = prefix ? `${prefix}.${index}` : String(index);
    if (Array.isArray(child)) slots(child, slot, result);
    else if (child !== null && child !== undefined && typeof child !== 'boolean' && child !== '') result.push(slot);
  });
  return result;
}

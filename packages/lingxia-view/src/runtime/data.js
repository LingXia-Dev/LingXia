// JSON-shaped values cross the View boundary frozen; nothing else crosses it.
export function immutable(value, depth = 0, omitUndefined = false) {
  if (depth > 64) throw Error('UI_LOCAL: value too deep');
  if (value === null || typeof value === 'string' || typeof value === 'boolean') return value;
  if (typeof value === 'number' && Number.isFinite(value)) return value;
  if (Array.isArray(value)) return Object.freeze(value.map(v => immutable(v, depth + 1, omitUndefined)));
  if (value && Object.getPrototypeOf(value) === Object.prototype) {
    return Object.freeze(Object.fromEntries(Object.entries(value)
      .filter(([, v]) => !omitUndefined || v !== undefined)
      .map(([k, v]) => [k, immutable(v, depth + 1, omitUndefined)])));
  }
  throw Error('UI_LOCAL: expected JSON value');
}

/** `immutable(value)` that hands back `previous`'s frozen parts wherever they hold the same value,
 *  so a patched page's unchanged data keeps its identity and what was built from it can stand. */
export function shared(value, previous, depth = 0) {
  if (depth > 64) throw Error('UI_LOCAL: value too deep');
  if (previous === undefined || value === null || typeof value !== 'object') return immutable(value, depth);
  if (Array.isArray(value)) {
    const old = Array.isArray(previous) ? previous : [];
    let same = old.length === value.length;
    const next = value.map((v, i) => { const item = shared(v, old[i], depth + 1); same &&= item === old[i]; return item; });
    return same ? previous : Object.freeze(next);
  }
  if (Object.getPrototypeOf(value) !== Object.prototype) return immutable(value, depth);
  const old = previous && typeof previous === 'object' && Object.getPrototypeOf(previous) === Object.prototype ? previous : null;
  const entries = [];
  let same = old !== null;
  for (const key of Object.keys(value)) {
    const had = old !== null && Object.prototype.hasOwnProperty.call(old, key);
    const item = shared(value[key], had ? old[key] : undefined, depth + 1);
    entries.push([key, item]);
    same &&= had && item === old[key];
  }
  // fromEntries defines own keys, so a `__proto__` key stays data.
  return same && entries.length === Object.keys(old).length ? old : Object.freeze(Object.fromEntries(entries));
}

/** Replays JSON Patch ops (add/replace/remove) the native replica already applied, copying only
 *  the containers on each op's path; everything else keeps its frozen identity. */
export function applyPatch(root, ops) {
  const fresh = new Set();
  for (const op of ops) {
    if (op.path === '') {
      root = op.op === 'remove' ? Object.freeze({}) : shared(op.value, root);
      continue;
    }
    const tokens = op.path.slice(1).split('/').map(t => t.replace(/~1/g, '/').replace(/~0/g, '~'));
    root = patchAt(root, tokens, 0, op, fresh);
  }
  for (const container of fresh) Object.freeze(container);
  return root;
}

function patchAt(node, tokens, i, op, fresh) {
  const key = tokens[i];
  const array = Array.isArray(node);
  if (i === tokens.length - 1) return patchLeaf(node, array, key, op, fresh);
  const slot = array ? Number(key) : key;
  const had = array || has(node, key);
  let child = had ? node[slot] : undefined;
  // The replica creates a missing container on the way, an array before a numeric token.
  if (child === undefined && !array) child = /^\d+$/.test(tokens[i + 1]) ? [] : {};
  const next = patchAt(child, tokens, i + 1, op, fresh);
  if (had && next === node[slot]) return node;
  const copy = mutable(node, fresh);
  define(copy, slot, next);
  if (!had) sortKeys(copy);
  return copy;
}

function patchLeaf(node, array, key, op, fresh) {
  if (array) {
    const index = key === '-' ? node.length : Number(key);
    if (op.op === 'remove' && index >= node.length) return node;
    const copy = mutable(node, fresh);
    if (op.op === 'add') copy.splice(Math.min(index, copy.length), 0, immutable(op.value));
    else if (op.op === 'remove') copy.splice(index, 1);
    else {
      while (copy.length < index) copy.push(null);
      copy[index] = shared(op.value, node[index]);
    }
    return copy;
  }
  const had = has(node, key);
  if (op.op === 'remove') {
    if (!had) return node;
    const copy = mutable(node, fresh);
    delete copy[key];
    return copy;
  }
  const value = shared(op.value, had ? node[key] : undefined);
  if (had && value === node[key]) return node;
  const copy = mutable(node, fresh);
  define(copy, key, value);
  if (!had) sortKeys(copy);
  return copy;
}

// The replica's objects iterate in code point order (as its whole tree arrives); a replayed new
// key takes the same place.
function sortKeys(object) {
  const entries = Object.entries(object).sort(([a], [b]) => compareKeys(a, b));
  for (const [key] of entries) delete object[key];
  for (const [key, value] of entries) define(object, key, value);
}

function compareKeys(a, b) {
  for (let i = 0; i < a.length && i < b.length; i++) {
    const x = a.codePointAt(i), y = b.codePointAt(i);
    if (x !== y) return x - y;
    if (x > 0xffff) i++;
  }
  return a.length - b.length;
}

function mutable(node, fresh) {
  if (fresh.has(node)) return node;
  // fromEntries defines own keys, so a `__proto__` key stays data.
  const copy = Array.isArray(node) ? node.slice() : Object.fromEntries(Object.entries(node));
  fresh.add(copy);
  return copy;
}

function define(target, key, value) {
  Object.defineProperty(target, key, { value, writable: true, enumerable: true, configurable: true });
}

// View isolates do not require the browser TextEncoder API.
export function utf8Bytes(value, limit) {
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

export function validInputText(value) {
  return typeof value === 'string' && utf8Bytes(value, 65536) <= 65536;
}

export const has = (object, key) => Object.prototype.hasOwnProperty.call(object, key);

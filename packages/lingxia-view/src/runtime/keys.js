// List keys are checked, encoded and indexed once per key array. An array that extends an
// earlier one (a list growing at its end) costs only its new keys, and one that replaces some
// items under the same keys (a patched data array) reuses the earlier keys.
import { utf8Bytes, isFrozen, extended } from './data.js';
import { limits } from '../generated/contract.js';

/** Key arrays this module made: never changed once made, and never frozen, which QuickJS pays for by length. */
const made = new WeakSet();
/** The key array each new one extends. */
const parents = new WeakMap();
/** Keys of a checked array, for the check of arrays extending it. */
const checked = new WeakMap();
const payloads = new WeakMap();
const positions = new WeakMap();

/** The latest keyed items, for an items array that extends it. */
let latest = null;
const itemKeys = new WeakMap();

/** Whether `keys` stays as it is: made here, or frozen. */
export const settled = keys => made.has(keys) || isFrozen(keys);

/** Keys of `items` by `getKey`; reused while `items` is the same frozen array. */
export function keysOf(items, getKey) {
  const known = itemKeys.get(items);
  if (known) return known;
  const frozen = isFrozen(items);
  const base = frozen && latest && extends_(items, latest.items) ? latest : null;
  // A patched data array: same length, its keys unchanged. The old key array stands.
  if (!base && frozen && latest && latest.getKey === getKey && sameKeys(items, latest, getKey)) {
    itemKeys.set(items, latest.keys);
    latest = { ...latest, items };
    return latest.keys;
  }
  let keys;
  if (base) {
    keys = base.keys.slice();
    for (let i = base.items.length; i < items.length; i++) keys.push(getKey(items[i], i));
  } else keys = items.map(getKey);
  if (frozen) {
    made.add(keys);
    if (base && base.keys.length) parents.set(keys, base.keys);
    itemKeys.set(items, keys);
    latest = { items, keys, getKey };
  }
  return keys;
}

function sameKeys(items, { items: before, keys }, getKey) {
  if (items.length !== before.length || !isFrozen(before)) return false;
  for (let i = 0; i < items.length; i++) {
    if (items[i] !== before[i] && getKey(items[i], i) !== keys[i]) return false;
  }
  return true;
}

// A data array the patch replay grew by appends names what it grew from; any other is compared.
function extends_(items, prefix) {
  if (items.length < prefix.length || !isFrozen(prefix)) return false;
  for (let from = extended(items); from; from = extended(from)) if (from === prefix) return true;
  for (let i = 0; i < prefix.length; i++) if (items[i] !== prefix[i]) return false;
  return true;
}

// A parent's set or map grows into its first extension's, so a chain of appends shares one; a
// second extension of the same parent finds it grown past the parent and rebuilds its own.
function inherited(parent, table, rebuild) {
  const entry = parent && table.get(parent);
  if (!entry) return null;
  return entry.size === parent.length ? entry : rebuild(parent);
}

/** Whether `keys` passed the check; `check(key, seen)` validates one key. */
export function checkKeys(keys, check) {
  if (checked.has(keys)) return;
  const parent = parents.get(keys);
  const parentSeen = inherited(parent, checked, keys => new Set(keys));
  const seen = parentSeen ?? new Set();
  for (let i = parentSeen ? parent.length : 0; i < keys.length; i++) {
    check(keys[i], seen);
    seen.add(keys[i]);
  }
  if (settled(keys)) checked.set(keys, seen);
}

/** `{payload, bytes}`: the JSON array of `keys` and its UTF-8 size. */
export function keyPayload(keys) {
  let encoded = payloads.get(keys);
  if (encoded) return encoded;
  const parent = parents.get(keys);
  const base = parent && payloads.get(parent);
  if (base && keys.length > parent.length) {
    // "[a]" extended by "[b]" is "[a,b]": one byte fewer than both.
    const tail = JSON.stringify(keys.slice(parent.length));
    encoded = { payload: `${base.payload.slice(0, -1)},${tail.slice(1)}`, bytes: base.bytes + utf8Bytes(tail, limits.itemKeyBytes) - 1 };
  } else {
    const payload = JSON.stringify(keys);
    encoded = { payload, bytes: utf8Bytes(payload, limits.itemKeyBytes) };
  }
  if (settled(keys)) payloads.set(keys, encoded);
  return encoded;
}

const identity = key => `key:${JSON.stringify(key)}`;

/** Each key's index, by its `key:"…"` identity. */
export function keyPositions(keys) {
  let map = positions.get(keys);
  if (map) return map;
  const parent = parents.get(keys);
  const base = inherited(parent, positions, keys => new Map(keys.map((key, i) => [identity(key), i])));
  map = base ?? new Map();
  for (let i = base ? parent.length : 0; i < keys.length; i++) map.set(identity(keys[i]), i);
  if (settled(keys)) positions.set(keys, map);
  return map;
}

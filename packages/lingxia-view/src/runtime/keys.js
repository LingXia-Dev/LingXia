// List keys are checked, encoded and indexed once per frozen key array. An array that
// extends an earlier one (a list growing at its end) costs only its new keys, and one that
// replaces some items under the same keys (a patched data array) reuses the earlier keys.
import { utf8Bytes } from './data.js';
import { limits } from '../generated/contract.js';

/** The frozen key array each new one extends. */
const parents = new WeakMap();
/** Keys of a checked array, for the check of arrays extending it. */
const checked = new WeakMap();
const payloads = new WeakMap();
const positions = new WeakMap();

/** The latest keyed items, for an items array that extends it. */
let latest = null;
const itemKeys = new WeakMap();

/** Frozen keys of `items` by `getKey`; reused while `items` is the same frozen array. */
export function keysOf(items, getKey) {
  const known = itemKeys.get(items);
  if (known) return known;
  const frozen = Object.isFrozen(items);
  // A patched data array: same length, its keys unchanged. The old key array stands.
  if (frozen && latest && latest.getKey === getKey && sameKeys(items, latest, getKey)) {
    itemKeys.set(items, latest.keys);
    latest = { ...latest, items };
    return latest.keys;
  }
  const base = frozen && latest && extendsArray(items, latest.items) ? latest : null;
  const keys = base
    ? Object.freeze(base.keys.concat(items.slice(base.items.length).map((item, i) => getKey(item, base.items.length + i))))
    : Object.freeze(items.map(getKey));
  if (frozen) {
    if (base && base.keys.length) parents.set(keys, base.keys);
    itemKeys.set(items, keys);
    latest = { items, keys, getKey };
  }
  return keys;
}

function sameKeys(items, { items: before, keys }, getKey) {
  if (items.length !== before.length || !Object.isFrozen(before)) return false;
  for (let i = 0; i < items.length; i++) {
    if (items[i] !== before[i] && getKey(items[i], i) !== keys[i]) return false;
  }
  return true;
}

function extendsArray(items, prefix) {
  if (items.length < prefix.length || !Object.isFrozen(prefix)) return false;
  for (let i = 0; i < prefix.length; i++) if (items[i] !== prefix[i]) return false;
  return true;
}

/** Whether `keys` passed the check; `check(key, seen)` validates one key. */
export function checkKeys(keys, check) {
  if (checked.has(keys)) return;
  const parent = parents.get(keys);
  const parentSeen = parent && checked.get(parent);
  const seen = parentSeen ? new Set(parentSeen) : new Set();
  for (let i = parentSeen ? parent.length : 0; i < keys.length; i++) {
    check(keys[i], seen);
    seen.add(keys[i]);
  }
  if (Object.isFrozen(keys)) checked.set(keys, seen);
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
  if (Object.isFrozen(keys)) payloads.set(keys, encoded);
  return encoded;
}

/** Each key's index, by its `key:"…"` identity. */
export function keyPositions(keys) {
  let map = positions.get(keys);
  if (map) return map;
  const parent = parents.get(keys);
  const base = parent && positions.get(parent);
  map = base ? new Map(base) : new Map();
  for (let i = base ? parent.length : 0; i < keys.length; i++) map.set(`key:${JSON.stringify(keys[i])}`, i);
  if (Object.isFrozen(keys)) positions.set(keys, map);
  return map;
}

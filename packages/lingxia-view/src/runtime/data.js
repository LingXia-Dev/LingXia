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

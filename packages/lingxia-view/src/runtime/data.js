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

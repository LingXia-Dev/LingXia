import { isRole } from './color.js';
export function normalizeShadows(value) {
  if (value === 'none') return value;
  const invalid = () => { throw Error('UI_SHADOW: invalid box shadow'); };
  if (!Array.isArray(value) || value.length > 8) invalid();
  return value.map(shadow => {
    if (!shadow || typeof shadow !== 'object' || Array.isArray(shadow) ||
        Object.keys(shadow).some(key => !['offsetX', 'offsetY', 'blur', 'spread', 'color'].includes(key))) invalid();
    const result = {};
    for (const key of ['offsetX', 'offsetY', 'blur', 'spread']) {
      const number = shadow[key] === undefined ? 0 : shadow[key];
      if (typeof number !== 'number' || !Number.isFinite(number) ||
          (key === 'blur' ? number < 0 || number > 256 : Math.abs(number) > 1024)) invalid();
      result[key] = number;
    }
    result.color = shadow.color === undefined ? 'currentColor' : shadow.color;
    if (result.color !== 'currentColor' && (typeof result.color !== 'string' || !(/^#[0-9a-fA-F]{6}([0-9a-fA-F]{2})?$/.test(result.color) || isRole(result.color)))) invalid();
    return result;
  });
}

const ringColor = (value, invalid) => {
  const color = value === undefined ? 'currentColor' : value;
  if (color !== 'currentColor' && (typeof color !== 'string' || !(/^#[0-9a-fA-F]{6}([0-9a-fA-F]{2})?$/.test(color) || isRole(color)))) invalid();
  return color;
};

/** A ring as `{ outer?: { width, offset, color, offsetColor? }, inner?: { width, color } }`, or `'none'`. */
export function normalizeRing(value) {
  if (value === 'none') return value;
  const invalid = () => { throw Error('UI_SHADOW: invalid ring'); };
  const plain = (object, keys) => object && typeof object === 'object' && !Array.isArray(object) && Object.keys(object).every(key => keys.includes(key));
  const size = number => typeof number === 'number' && Number.isFinite(number) && number >= 0 && number <= 1024;
  if (!plain(value, ['outer', 'inner'])) invalid();
  const result = {};
  if (value.outer !== undefined) {
    const { width, offset = 0, color, offsetColor } = value.outer;
    if (!plain(value.outer, ['width', 'offset', 'color', 'offsetColor']) || !size(width) || !size(offset)) invalid();
    result.outer = { width, offset, color: ringColor(color, invalid) };
    if (offsetColor !== undefined) result.outer.offsetColor = ringColor(offsetColor, invalid);
  }
  if (value.inner !== undefined) {
    if (!plain(value.inner, ['width', 'color']) || !size(value.inner.width)) invalid();
    result.inner = { width: value.inner.width, color: ringColor(value.inner.color, invalid) };
  }
  return result;
}

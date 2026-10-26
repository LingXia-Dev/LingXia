import { elements, style as table } from '../generated/contract.js';
import { fieldOptions } from '../generated/wire.js';
import { normalizeShadows } from './shadow.js';
import { normalizeGradient } from './gradient.js';
import { utf8Bytes, has } from '../runtime/data.js';

const finite = value => typeof value === 'number' && Number.isFinite(value);
const percent = value => typeof value === 'string' && /^(?:\d+(?:\.\d+)?|\.\d+)%$/.test(value) ? Number(value.slice(0, -1)) : undefined;
const shorthand = name => Array.isArray(table[name]?.wire);
const longhands = Object.fromEntries(Object.entries(table).filter(([name]) => shorthand(name)).map(([name, entry]) =>
  [name, Object.keys(table).filter(other => !shorthand(other) && entry.wire.includes(table[other].wire))]));

/**
 * Layers styles the way the CSS cascade does: a shorthand in a later layer
 * resets the longhands earlier layers set; within one layer longhands win.
 */
export function mergeStyle(...layers) {
  const result = {};
  for (const layer of layers) {
    if (!layer) continue;
    for (const name of Object.keys(layer)) for (const longhand of longhands[name] ?? []) if (!(longhand in layer)) delete result[longhand];
    Object.assign(result, layer);
  }
  return result;
}

/** Throws unless every property is a known style that `tag` renders. */
export function admitStyle(tag, style) {
  const accepted = elements[tag].style;
  for (const name of Object.keys(style)) {
    if (!has(table, name)) throw Error(`UI_STYLE: unknown style property ${name}`);
    if (!accepted.includes(name)) throw Error(`UI_STYLE: ${name} is not supported on <${tag}>`);
  }
}

function invalid(name) { throw Error(`UI_STYLE_VALUE: ${name}`); }

function length(name, value, { signed = false, max = 16384 } = {}) {
  if (!finite(value) || value > max || value < (signed ? -max : 0)) invalid(name);
  return value;
}

function dimension(name, value, env, out, key) {
  const ch = typeof value === 'string' && value.match(/^(\d+(?:\.\d+)?)ch$/);
  if (ch) {
    if (key !== 'maxWidth') invalid(name);
    out.maxWidthCh = length(name, Number(ch[1]), { max: 4096 });
    return;
  }
  const viewport = typeof value === 'string' && value.match(/^(\d+(?:\.\d+)?)(vw|vh)$/);
  if (viewport) {
    const extent = env?.viewport?.[viewport[2] === 'vw' ? 'width' : 'height'];
    if (!finite(extent)) invalid(name);
    out[key] = length(name, extent * Number(viewport[1]) / 100);
  } else if (percent(value) !== undefined) out[`${key}Percent`] = length(name, percent(value));
  else if (value === 'auto' && key === 'height') out.heightAuto = 1;
  else if (value === 'auto' && key === 'width') { /* CSS default */ }
  else if (['fit-content', 'max-content', 'min-content'].includes(value) && (key === 'width' || key === 'height')) out[`${key}Keyword`] = value;
  else out[key] = length(name, value);
}

const alignment = { 'flex-start': 'start', 'flex-end': 'end' };
const justification = { 'flex-start': 'start', 'flex-end': 'end', 'space-between': 'between', 'space-around': 'around', 'space-evenly': 'evenly' };

function option(name, key, value) {
  if (!fieldOptions[key]?.includes(value)) invalid(name);
  return value;
}

function backdrop(name, value) {
  if (value === 'none') return [0, 1];
  const match = typeof value === 'string' && value.trim().match(/^(?:blur\((\d+(?:\.\d+)?)px\))?\s*(?:saturate\((\d+(?:\.\d+)?)(%?)\))?$/);
  if (!match || (match[1] === undefined && match[2] === undefined)) invalid(name);
  const blur = match[1] === undefined ? 0 : Number(match[1]);
  const saturation = match[2] === undefined ? 1 : Number(match[2]) / (match[3] ? 100 : 1);
  if (blur > 64 || saturation > 16) invalid(name);
  return [blur, saturation];
}

const translateLength = value => finite(value) ? Math.abs(value) <= 16384
  : typeof value === 'string' && /^-?(?:\d+(?:\.\d+)?|\.\d+)%$/.test(value) && Math.abs(parseFloat(value)) <= 1000;

// CSS individual transform properties, composed natively as translate, rotate, scale about the origin.
function transformPart(name, value, out) {
  const pair = check => {
    const parts = Array.isArray(value) ? value : [value, name === 'scale' ? value : 0];
    if (parts.length !== 2 || !parts.every(check)) invalid(name);
    return [...parts];
  };
  if (name === 'translate') out.translate = pair(translateLength);
  else if (name === 'scale') out.scale = pair(v => finite(v) && Math.abs(v) <= 1000);
  else if (name === 'rotate') { if (!finite(value) || Math.abs(value) > 36000) invalid(name); out.rotate = value; }
  else {
    if (!Array.isArray(value) || value.length !== 2 || !value.every(v => finite(v) && Math.abs(v) <= 16)) invalid(name);
    out.origin = [...value];
  }
}

// One keyword per CSS font-variant-numeric group, in CSS order.
const numericGroups = [['ordinal'], ['slashed-zero'], ['lining-nums', 'oldstyle-nums'], ['proportional-nums', 'tabular-nums'], ['diagonal-fractions', 'stacked-fractions']];
function numericVariant(name, value) {
  if (value === 'normal') return value;
  const tokens = typeof value === 'string' ? value.trim().split(/\s+/) : [];
  const picked = numericGroups.map(group => tokens.filter(token => group.includes(token)));
  if (!tokens.length || picked.some(group => group.length > 1) || picked.flat().length !== tokens.length) invalid(name);
  return picked.flat().join(' ');
}

function animation(value) {
  if (value === null) return undefined;
  if (!value || typeof value !== 'object' || Array.isArray(value)) throw Error('UI_ANIMATION: expected definition');
  const encoded = JSON.stringify(value, (_, item) => {
    if ((typeof item === 'number' && !Number.isFinite(item)) || ['undefined', 'function', 'symbol', 'bigint'].includes(typeof item))
      throw Error('UI_ANIMATION: invalid payload value');
    return item;
  });
  if (utf8Bytes(encoded, 32768) > 32768) throw Error('UI_ANIMATION: wire budget exceeded');
  return JSON.parse(encoded);
}

/**
 * Lowers a CSS-named style to wire fields keyed by field name. Longhands win
 * over shorthands regardless of order; `inherited` carries font size and the
 * line-height multiplier the way CSS inherits them.
 */
export function lowerStyle(tag, style, env, inherited = { fontSize: 14 }) {
  admitStyle(tag, style);
  const out = {};
  const fontSize = style.fontSize === undefined ? inherited.fontSize : length('fontSize', style.fontSize);
  if (style.fontSize !== undefined && style.fontSize <= 0) invalid('fontSize');
  let lineHeight = inherited.lineHeight;
  const transform = {};
  const names = Object.keys(style).sort((a, b) => Number(shorthand(b)) - Number(shorthand(a)));
  for (const name of names) {
    const value = style[name], entry = table[name], key = entry.wire;
    switch (entry.lower) {
      case 'dimension': dimension(name, value, env, out, key); break;
      case 'margin':
        if (value !== 'auto') length(name, value, { signed: true });
        for (const side of [key].flat()) out[side] = value;
        break;
      case 'inset': {
        const amount = percent(value) !== undefined ? percent(value) : value;
        length(name, amount, { signed: true });
        for (const side of [key].flat()) out[side] = value;
        break;
      }
      case 'length':
        length(name, value);
        for (const side of [key].flat()) out[side] = value;
        break;
      case 'z-index':
        if (!Number.isInteger(value) || Math.abs(value) > 10000) invalid(name);
        out[key] = value; break;
      case 'columns': {
        const count = typeof value === 'string' ? Number(value.match(/^repeat\((\d+),\s*minmax\(0,\s*1fr\)\)$/)?.[1]) : value;
        if (!Number.isInteger(count) || count < 1 || count > 64) invalid(name);
        out[key] = count; break;
      }
      case 'line-clamp':
        if (!Number.isInteger(value) || value < 1 || value > 1024) invalid(name);
        out[key] = value; break;
      case 'number': out[key] = length(name, value); break;
      case 'opacity': if (!finite(value) || value < 0 || value > 1) invalid(name); out[key] = value; break;
      case 'font-size': out[key] = fontSize; break;
      case 'font-weight': if (!finite(value) || value < 1 || value > 1000) invalid(name); out[key] = value; break;
      case 'color': if (typeof value !== 'string' || !value) invalid(name); out[key] = value; break;
      case 'gradient': out[key] = normalizeGradient(value); break;
      case 'shadow': out[key] = normalizeShadows(value); break;
      case 'backdrop': [out[key[0]], out[key[1]]] = backdrop(name, value); break;
      case 'animation': { const lowered = animation(value); if (lowered !== undefined) out[key] = lowered; break; }
      case 'transform': transformPart(name, value, transform); break;
      case 'aspect-ratio':
        if (value === 'auto') break;
        if (!finite(value) || value <= 0 || value > 10000) invalid(name);
        out[key] = value; break;
      case 'font-variant-numeric': out[key] = numericVariant(name, value); break;
      case 'basis':
        if (value !== 'auto') length(name, percent(value) ?? value);
        out[key] = value; break;
      case 'option': out[key] = option(name, key, value); break;
      case 'alignment': out[key] = option(name, key, alignment[value] ?? value); break;
      case 'justification': out[key] = option(name, key, justification[value] ?? value); break;
      case 'family': out[key] = option(name, key, { 'sans-serif': 'sans', monospace: 'mono' }[value]); break;
      case 'line-height':
        if (typeof value === 'string' && /^\d+(?:\.\d+)?px$/.test(value)) lineHeight = length(name, Number(value.slice(0, -2)));
        else if (finite(value) && value > 0 && value <= 64) lineHeight = { multiplier: value };
        else invalid(name);
        break;
      case 'letter-spacing': {
        const em = typeof value === 'string' && value.match(/^(-?\d+(?:\.\d+)?)em$/);
        const pixels = em ? fontSize * Number(em[1]) : value;
        out[key] = length(name, pixels, { signed: true, max: 1024 });
        break;
      }
      case 'scroll-axis':
        if (value !== 'auto' || (style.overflowX !== undefined && style.overflowY !== undefined)) invalid(name);
        out[key] = name === 'overflowX' ? 'horizontal' : 'vertical';
        break;
      case 'fit':
        if (!(tag === 'vector' ? ['contain', 'fill'] : ['cover', 'contain', 'fill']).includes(value)) invalid(name);
        out[key] = value; break;
      default: throw Error(`UI_STYLE: ${name} has no native lowering`);
    }
  }
  if (Object.keys(transform).length) out.transform = JSON.stringify(transform);
  // A multiplier recomputes against each node's font size; pixels inherit natively.
  if (style.lineHeight !== undefined || typeof lineHeight === 'object') {
    const pixels = typeof lineHeight === 'object' ? fontSize * lineHeight.multiplier : lineHeight;
    if (!(pixels > 0 && pixels <= 16384)) invalid('lineHeight');
    out.lineHeight = pixels;
  }
  return { fields: out, typography: { fontSize, lineHeight } };
}

/** The paint subset of a style; state styles may carry nothing else. */
export function paintOnly(state, tag, style) {
  admitStyle(tag, style);
  for (const name of Object.keys(style))
    if (!table[name].paint) throw Error(`UI_STYLE: ${state}: supports paint properties only, not ${name}`);
}

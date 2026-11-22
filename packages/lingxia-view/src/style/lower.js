import { declaredFamily } from '../runtime/fonts.js';
import { kindOf, style as table } from '../generated/contract.js';
import { fieldOptions, field } from '../generated/wire.js';
import { limits, options, scales, scaledFields, scaleDefaults } from '../generated/contract.js';
import { normalizeRing, normalizeShadows } from './shadow.js';
import { appliesTo } from './hints.js';
import { normalizeGradient } from './gradient.js';
import { wireColorProblem } from './color.js';
import { utf8Bytes, has } from '../runtime/data.js';

const finite = value => typeof value === 'number' && Number.isFinite(value);
const percent = value => typeof value === 'string' && /^(?:\d+(?:\.\d+)?|\.\d+)%$/.test(value) ? Number(value.slice(0, -1)) : undefined;
const shorthand = name => Array.isArray(table[name]?.wire);
// Physical shorthands also reset the flow-relative longhands of the same box edge.
const logical = {
  margin: ['marginInlineStart', 'marginInlineEnd'], padding: ['paddingInlineStart', 'paddingInlineEnd'],
  inset: ['insetInlineStart', 'insetInlineEnd'], borderWidth: ['borderInlineStartWidth', 'borderInlineEndWidth'],
  borderRadius: ['borderStartStartRadius', 'borderStartEndRadius', 'borderEndStartRadius', 'borderEndEndRadius'],
};
const longhands = Object.fromEntries(Object.entries(table).filter(([name]) => shorthand(name)).map(([name, entry]) =>
  [name, [...Object.keys(table).filter(other => !shorthand(other) && entry.wire.includes(table[other].wire)), ...(logical[name] ?? [])]]));
// Flow-relative keys and the physical key they map to under each direction.
const sides = [['marginInlineStart', 'marginLeft', 'marginRight'], ['marginInlineEnd', 'marginRight', 'marginLeft'],
  ['paddingInlineStart', 'paddingLeft', 'paddingRight'], ['paddingInlineEnd', 'paddingRight', 'paddingLeft'],
  ['insetInlineStart', 'left', 'right'], ['insetInlineEnd', 'right', 'left'],
  ['borderInlineStartWidth', 'borderLeftWidth', 'borderRightWidth'], ['borderInlineEndWidth', 'borderRightWidth', 'borderLeftWidth'],
  ['borderStartStartRadius', 'borderTopLeftRadius', 'borderTopRightRadius'], ['borderStartEndRadius', 'borderTopRightRadius', 'borderTopLeftRadius'],
  ['borderEndStartRadius', 'borderBottomLeftRadius', 'borderBottomRightRadius'], ['borderEndEndRadius', 'borderBottomRightRadius', 'borderBottomLeftRadius']];
const aliases = direction => {
  const map = {};
  for (const [flow, ltr, rtl] of sides) {
    const physical = direction === 'rtl' ? rtl : ltr;
    (map[flow] ??= []).push(physical); (map[physical] ??= []).push(flow);
  }
  return map;
};
const aliasesByDirection = { ltr: aliases('ltr'), rtl: aliases('rtl') };

/** Clear what a later `layer` resets in `result`: the longhands of each shorthand it sets. */
export function resetLonghands(result, layer) {
  for (const name of Object.keys(layer))
    for (const longhand of longhands[name] ?? []) if (!(longhand in layer)) delete result[longhand];
}

/**
 * Layers styles the way the CSS cascade does: a shorthand in a later layer
 * resets the longhands earlier layers set; within one layer longhands win.
 */
export function mergeStyle(direction, ...layers) {
  const result = {};
  const alias = aliasesByDirection[direction] ?? aliasesByDirection.ltr;
  for (const layer of layers) {
    if (!layer) continue;
    for (const name of Object.keys(layer)) {
      for (const longhand of longhands[name] ?? []) if (!(longhand in layer)) delete result[longhand];
      // A later layer's physical edge replaces an earlier flow-relative one on the same side, and back.
      for (const other of alias[name] ?? []) if (!(other in layer)) delete result[other];
    }
    Object.assign(result, layer);
  }
  return result;
}

/** Throws unless every property is a known style that `tag` renders. */
export function admitStyle(tag, style) {
  const accepted = kindOf(tag).style;
  for (const name of Object.keys(style)) {
    if (!has(table, name)) throw Error(`UI_STYLE: unknown style property ${name}`);
    if (!accepted.includes(name)) throw Error(`UI_STYLE: ${name} is not supported on <${tag}>${appliesTo(name)}`);
  }
}

// Names what the property takes and what it got: a bad value usually comes from data, far from the style.
function invalid(name, value, takes) {
  const got = typeof value === 'number' ? String(value) : (JSON.stringify(value) ?? typeof value).slice(0, 40);
  throw Error(`UI_STYLE_VALUE: ${name} takes ${takes}; got ${got}`);
}

// A theme step the host resolves: `[slot, times, plus]`.
const isStep = value => Array.isArray(value) && value.length === 3 && Number.isInteger(value[0]) && scales[value[0]] !== undefined && finite(value[1]) && finite(value[2]);
// A step where the host cannot resolve one (a slider part) takes the built-in pack's value.
const fold = value => isStep(value) && finite(scaleDefaults[value[0]]) ? value[1] * scaleDefaults[value[0]] + value[2] : value;
// `value` scaled by `factor`, a step staying a step.
const times = (value, factor) => isStep(value) ? [value[0], value[1] * factor, value[2] * factor] : value * factor;

function length(name, value, { signed = false, max = 16384 } = {}) {
  if (isStep(value)) {
    // Steps are positive: a negative multiple with no positive offset can only resolve below 0.
    if (!signed && value[1] < 0 && value[2] <= 0) invalid(name, value, 'a step that resolves to 0 or more');
    return value;
  }
  const min = signed ? -max : 0;
  if (!finite(value) || value > max || value < min) invalid(name, value, `a number from ${min} to ${max}`);
  return value;
}

function dimension(name, value, env, out, key) {
  const ch = typeof value === 'string' && value.match(/^(\d+(?:\.\d+)?)ch$/);
  if (ch) {
    // Widths only: a `ch` is the advance of the element's own `0`, which only the host can shape.
    if (!['width', 'minWidth', 'maxWidth'].includes(key)) invalid(name, value, 'points, a percentage or vw/vh; ch sizes widths only');
    out[`${key}Ch`] = length(name, Number(ch[1]), { max: 4096 });
    return;
  }
  const viewport = typeof value === 'string' && value.match(/^(\d+(?:\.\d+)?)(vw|vh)$/);
  if (viewport) {
    // Viewport units measure the page's safe content box: inside the insets, above the keyboard.
    const insets = env?.insets ?? {}, keyboard = env?.keyboard?.height ?? 0;
    const extent = viewport[2] === 'vw'
      ? env?.viewport?.width - (insets.left ?? 0) - (insets.right ?? 0)
      : env?.viewport?.height - (insets.top ?? 0) - Math.max(insets.bottom ?? 0, keyboard);
    if (!finite(extent)) invalid(name, value, 'vw/vh only once the page has a viewport');
    out[key] = length(name, extent * Number(viewport[1]) / 100);
  } else if (percent(value) !== undefined) out[`${key}Percent`] = length(name, percent(value));
  else if (value === 'auto' && key === 'height') out.heightAuto = 1;
  else if (value === 'auto' && key === 'width') { /* CSS default */ }
  else if (['fit-content', 'max-content', 'min-content'].includes(value) && ['width', 'height', 'minWidth', 'minHeight', 'maxWidth', 'maxHeight'].includes(key)) out[`${key}Keyword`] = value;
  else out[key] = length(name, value);
}

const alignment = { 'flex-start': 'start', 'flex-end': 'end' };
const justification = { 'flex-start': 'start', 'flex-end': 'end', 'space-between': 'between', 'space-around': 'around', 'space-evenly': 'evenly' };
// `alignSafe` bits per wire field: `safe center` keeps an overflowing item at the start, as in CSS.
const safeBits = { justify: 1, align: 2, alignSelf: 4, alignContent: 8, justifyItems: 16, justifySelf: 32 };
function aligned(name, key, value, names, safe) {
  if (typeof value === 'string' && value.startsWith('safe ')) {
    value = value.slice(5);
    if (!['center', 'end', 'flex-end'].includes(value)) invalid(name, `safe ${value}`, 'safe center, safe end or safe flex-end');
    safe.bits |= safeBits[key];
  }
  return option(name, key, names[value] ?? value);
}

function option(name, key, value) {
  if (!fieldOptions[key]?.includes(value)) invalid(name, value, `one of ${fieldOptions[key]?.join(', ')}`);
  return value;
}

// A generic, or a face the theme declared with `@font-face` before its generic fallback.
const generics = { 'sans-serif': 'sans', monospace: 'mono', serif: 'serif' };
function family(name, value) {
  if (typeof value !== 'string') invalid(name, value, 'a family name');
  const parts = value.split(',').map(part => part.trim().replace(/^['"]|['"]$/g, '')).filter(Boolean);
  if (parts.length === 1 && generics[parts[0]]) return generics[parts[0]];
  const [face, fallback = 'sans-serif'] = parts;
  if (parts.length > 2 || !generics[fallback] || Object.values(generics).includes(face)) invalid(name, value, 'sans-serif, serif, monospace, or one @font-face family before one of them');
  if (!declaredFamily(face)) throw Error(`UI_FONT: ${face} is not declared with @font-face in the page's theme`);
  return `${face},${generics[fallback]}`;
}

function backdrop(name, value) {
  if (value === 'none') return [0, 1];
  const match = typeof value === 'string' && value.trim().match(/^(?:blur\((\d+(?:\.\d+)?)px\))?\s*(?:saturate\((\d+(?:\.\d+)?)(%?)\))?$/);
  if (!match || (match[1] === undefined && match[2] === undefined)) invalid(name, value, 'none, blur(<px>) and/or saturate(<n>)');
  const blur = match[1] === undefined ? 0 : Number(match[1]);
  const saturation = match[2] === undefined ? 1 : Number(match[2]) / (match[3] ? 100 : 1);
  if (blur > 64 || saturation > 16) invalid(name, value, 'a blur up to 64px and a saturation up to 16');
  return [blur, saturation];
}

// Track lists are parsed natively; here they only need a bounded, function-only alphabet.
function tracks(name, value) {
  if (typeof value !== 'string' || value.length > 1024 || !/^[a-z0-9\s().,%-]+$/.test(value)) invalid(name, value, 'a track list such as repeat(3, 1fr) or 120px 1fr');
  return value.trim().replace(/\s+/g, ' ');
}

function gridEdge(name, value) {
  const text = typeof value === 'number' ? String(value) : typeof value === 'string' ? value.trim().replace(/\s+/g, ' ') : '';
  if (!/^(?:auto|-?[1-9]\d?|span [1-9]\d?)$/.test(text)) invalid(name, value, 'auto, a line from -99 to 99 (not 0), or span 1 to 99');
  return text;
}

function gridLine(name, value, lines) {
  const axis = name.startsWith('gridColumn') ? 'Column' : 'Row';
  const line = lines[axis] ??= {};
  if (name.endsWith('Start')) line.start = gridEdge(name, value);
  else if (name.endsWith('End')) line.end = gridEdge(name, value);
  else {
    const parts = String(value).split('/');
    if (parts.length > 2) invalid(name, value, 'start / end');
    line.shorthand = [gridEdge(name, parts[0]), parts[1] === undefined ? 'auto' : gridEdge(name, parts[1])];
  }
}

const translateLength = value => finite(value) ? Math.abs(value) <= 16384
  : typeof value === 'string' && /^-?(?:\d+(?:\.\d+)?|\.\d+)%$/.test(value) && Math.abs(parseFloat(value)) <= 1000;

// CSS individual transform properties, composed natively as translate, rotate, scale about the origin.
function transformPart(name, value, out) {
  const pair = check => {
    const parts = Array.isArray(value) ? value : [value, name === 'scale' ? value : 0];
    if (parts.length !== 2 || !parts.every(check)) invalid(name, value, name === 'scale' ? 'a factor or [x, y], each up to 1000' : 'points or a percentage, or [x, y] of them, up to 16384 points or 1000%');
    return [...parts];
  };
  if (name === 'translate') out.translate = pair(translateLength);
  else if (name === 'scale') out.scale = pair(v => finite(v) && Math.abs(v) <= 1000);
  else if (name === 'rotate') { if (!finite(value) || Math.abs(value) > 36000) invalid(name, value, 'degrees from -36000 to 36000'); out.rotate = value; }
  else {
    if (!Array.isArray(value) || value.length !== 2 || !value.every(v => finite(v) && Math.abs(v) <= 16)) invalid(name, value, '[x, y] as box fractions from -16 to 16');
    out.origin = [...value];
  }
}

// One keyword per CSS font-variant-numeric group, in CSS order.
const numericGroups = [['ordinal'], ['slashed-zero'], ['lining-nums', 'oldstyle-nums'], ['proportional-nums', 'tabular-nums'], ['diagonal-fractions', 'stacked-fractions']];
function numericVariant(name, value) {
  if (value === 'normal') return value;
  const tokens = typeof value === 'string' ? value.trim().split(/\s+/) : [];
  const picked = numericGroups.map(group => tokens.filter(token => group.includes(token)));
  if (!tokens.length || picked.some(group => group.length > 1) || picked.flat().length !== tokens.length) invalid(name, value, `normal, or at most one of each of ${numericGroups.map(group => group.join('/')).join(', ')}`);
  return picked.flat().join(' ');
}

// Up to four alpha fades to a side, intersected; `none` means no field.
function maskValue(value) {
  if (value === 'none') return undefined;
  const invalid = () => { throw Error('UI_STYLE_VALUE: maskImage'); };
  const position = stop => (stop.offset !== undefined) !== (stop.px !== undefined) &&
    (stop.offset === undefined || (finite(stop.offset) && Math.abs(stop.offset) <= 16)) && (stop.px === undefined || (finite(stop.px) && Math.abs(stop.px) <= 16384));
  if (!Array.isArray(value) || !value.length || value.length > 4 || value.some(layer => !layer || !['top', 'bottom', 'left', 'right'].includes(layer.to) ||
      Object.keys(layer).some(key => !['to', 'stops'].includes(key)) || !Array.isArray(layer.stops) || layer.stops.length < 2 || layer.stops.length > 16 ||
      layer.stops.some(stop => !stop || Object.keys(stop).some(key => !['alpha', 'offset', 'px'].includes(key)) || !finite(stop.alpha) || stop.alpha < 0 || stop.alpha > 1 || !position(stop)))) invalid();
  return JSON.stringify(value.map(({ to, stops }) => ({ to, stops: stops.map(({ alpha, offset, px }) => offset === undefined ? { px, alpha } : { offset, alpha }) })));
}

// One transition for the node's painted properties; `none` and an empty list mean no field.
function transitionValue(value) {
  if (value === 'none') return undefined;
  const invalid = () => { throw Error('UI_STYLE_VALUE: transition'); };
  if (!value || typeof value !== 'object' || Object.keys(value).some(key => !['properties', 'durationMs', 'delayMs', 'easing'].includes(key))) invalid();
  const { properties, delayMs = 0 } = value;
  let { durationMs, easing = [0.4, 0, 0.2, 1] } = value;
  const step = (ref, family) => isStep(ref) && scales[ref[0]].family === family && ref[1] === 1 && ref[2] === 0;
  // A motion step resolves when the transition starts; check it as its smallest legal stand-in.
  const durationStep = step(durationMs, 'duration'), easingStep = step(easing, 'ease');
  if (durationStep) durationMs = 1;
  if (easingStep) easing = 'linear';
  if (!Array.isArray(properties) || !properties.length || properties.length > options['transition-property'].length ||
      new Set(properties).size !== properties.length || properties.some(name => !options['transition-property'].includes(name)) ||
      !finite(durationMs) || durationMs <= 0 || durationMs > limits.transitionMs || !finite(delayMs) || delayMs < 0 || delayMs > limits.transitionMs ||
      !(easing === 'linear' || easing === 'step-end' || (Array.isArray(easing) && easing.length === 4 && easing.every(finite)) ||
        (easing && typeof easing === 'object' && Object.keys(easing).join() === 'spring' && finite(easing.spring) && easing.spring >= 0 && easing.spring <= 0.9))) invalid();
  return JSON.stringify({ properties, durationMs: durationStep ? value.durationMs : durationMs, delayMs, easing: easingStep ? value.easing : easing });
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
// CSS `object-position` as box fractions: keywords in either order, or percentages.
function objectPosition(value) {
  const parts = typeof value === 'string' ? value.trim().split(/\s+/) : [];
  const horizontal = { left: 0, right: 1 }, vertical = { top: 0, bottom: 1 };
  const fraction = part => {
    if (part === 'center') return 0.5;
    const match = /^(-?(?:\d*\.)?\d+)%$/.exec(part);
    if (!match || Math.abs(Number(match[1])) > 1600) invalid('objectPosition', part, 'center, left/right/top/bottom or a percentage up to 1600%');
    return Number(match[1]) / 100;
  };
  if (parts.length === 1) {
    const [part] = parts;
    return part in vertical ? [0.5, vertical[part]] : [part in horizontal ? horizontal[part] : fraction(part), 0.5];
  }
  if (parts.length !== 2) invalid('objectPosition', value, 'one or two positions');
  let [x, y] = parts;
  if (x in vertical || y in horizontal) [x, y] = [y, x];
  if (x in vertical || y in horizontal) invalid('objectPosition', value, 'one horizontal and one vertical position');
  return [x in horizontal ? horizontal[x] : fraction(x), y in vertical ? vertical[y] : fraction(y)];
}

export function lowerStyle(tag, style, env, inherited = { fontSize: 14 }) {
  admitStyle(tag, style);
  const out = {};
  const fontSize = style.fontSize === undefined ? inherited.fontSize : length('fontSize', style.fontSize);
  if (style.fontSize !== undefined && !isStep(style.fontSize) && style.fontSize <= 0) invalid('fontSize', style.fontSize, 'a number above 0');
  let lineHeight = inherited.lineHeight;
  const transform = {}, lines = {}, safe = { bits: 0 };
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
        if (value !== 'anchor(center)') length(name, amount, { signed: true });
        for (const side of [key].flat()) out[side] = value;
        break;
      }
      case 'length':
        length(name, value);
        for (const side of [key].flat()) out[side] = value;
        break;
      case 'z-index':
        if (!Number.isInteger(value) || Math.abs(value) > 10000) invalid(name, value, 'a whole number from -10000 to 10000');
        out[key] = value; break;
      case 'grid-template':
        if (Number.isInteger(value) && value >= 1 && value <= limits.gridTracks) out[key] = `repeat(${value}, minmax(0, 1fr))`;
        else out[key] = tracks(name, value);
        break;
      case 'grid-auto': out[key] = tracks(name, value); break;
      case 'grid-flow': out[key] = option(name, key, typeof value === 'string' ? value.trim().replace(/\s+/, '-') : value); break;
      case 'grid-line': gridLine(name, value, lines); break;
      case 'order':
        if (!Number.isInteger(value) || Math.abs(value) > 10000) invalid(name, value, 'a whole number from -10000 to 10000');
        out[key] = value; break;
      case 'align-content': out[key] = aligned(name, key, value, justification, safe); break;
      case 'line-clamp':
        if (!Number.isInteger(value) || value < 1 || value > 1024) invalid(name, value, 'a whole number of lines from 1 to 1024');
        out[key] = value; break;
      case 'number': out[key] = length(name, value); break;
      case 'opacity': if (!finite(value) || value < 0 || value > 1) invalid(name, value, 'a number from 0 to 1'); out[key] = value; break;
      case 'font-size': out[key] = fontSize; break;
      case 'font-weight': if (!finite(value) || value < 1 || value > 1000) invalid(name, value, 'a number from 1 to 1000'); out[key] = value; break;
      case 'color': {
        if (typeof value !== 'string' || !value) invalid(name, value, 'a colour');
        const problem = wireColorProblem(value, name);
        if (problem) throw Error(problem);
        out[key] = value === 'transparent' ? '#00000000' : value;
        break;
      }
      case 'gradient': out[key] = normalizeGradient(value); break;
      case 'shadow': out[key] = normalizeShadows(value); break;
      case 'ring': out[key] = normalizeRing(value); break;
      case 'backdrop': [out[key[0]], out[key[1]]] = backdrop(name, value); break;
      case 'animation': { const lowered = animation(value); if (lowered !== undefined) out[key] = lowered; break; }
      case 'transform': transformPart(name, value, transform); break;
      case 'transition': { const lowered = transitionValue(value); if (lowered !== undefined) out[key] = lowered; break; }
      case 'mask': { const lowered = maskValue(value); if (lowered !== undefined) out[key] = lowered; break; }
      case 'filter': {
        if (value === 'none') { out[key] = 0; break; }
        const blur = typeof value === 'string' && value.trim().match(/^blur\((\d+(?:\.\d+)?)px\)$/);
        if (!blur || Number(blur[1]) > 64) invalid(name, value, 'none or blur(<px>) up to 64px');
        out[key] = Number(blur[1]);
        break;
      }
      case 'aspect-ratio':
        if (value === 'auto') break;
        if (!finite(value) || value <= 0 || value > 10000) invalid(name, value, 'auto or a ratio above 0 up to 10000');
        out[key] = value; break;
      case 'font-variant-numeric': out[key] = numericVariant(name, value); break;
      case 'basis':
        if (value !== 'auto') length(name, percent(value) ?? value);
        out[key] = value; break;
      case 'snap-type': {
        // `scroll-snap-type: y mandatory`: the axis snaps; every snap here is mandatory.
        const axis = String(value).trim().split(/\s+/)[0];
        out[key] = option(name, key, axis); break;
      }
      case 'option':
        if (name === 'userSelect' && ['input', 'textarea'].includes(tag)) throw Error('UI_STYLE: editors select their text already; userSelect is for static text');
        out[key] = option(name, key, value); break;
      case 'alignment': out[key] = aligned(name, key, value, alignment, safe); break;
      case 'justification': out[key] = aligned(name, key, value, justification, safe); break;
      case 'family': out[key] = family(name, value); break;
      case 'line-height':
        if (isStep(value)) lineHeight = value;
        else if (typeof value === 'string' && /^\d+(?:\.\d+)?px$/.test(value)) lineHeight = length(name, Number(value.slice(0, -2)));
        else if (finite(value) && value > 0 && value <= 64) lineHeight = { multiplier: value };
        else invalid(name, value, 'a multiplier above 0 up to 64, or <n>px');
        break;
      case 'letter-spacing': {
        const em = typeof value === 'string' && value.match(/^(-?\d+(?:\.\d+)?)em$/);
        const pixels = em ? times(fontSize, Number(em[1])) : value;
        out[key] = length(name, pixels, { signed: true, max: 1024 });
        break;
      }
      case 'scroll-axis':
        if (value !== 'auto' || (style.overflowX !== undefined && style.overflowY !== undefined)) invalid(name, value, 'auto, on one axis only');
        out[key] = name === 'overflowX' ? 'horizontal' : 'vertical';
        break;
      case 'fit':
        if (!(tag === 'vector' ? ['contain', 'fill'] : ['cover', 'contain', 'fill']).includes(value)) invalid(name, value, tag === 'vector' ? 'contain or fill' : 'cover, contain or fill');
        out[key] = value; break;
      case 'object-position':
        out[key] = JSON.stringify(objectPosition(value)); break;
      case 'part': out[key] = partRecord(name, value, env); break;
      default: throw Error(`UI_STYLE: ${name} has no native lowering`);
    }
  }
  if (Object.keys(transform).length) out.transform = JSON.stringify(transform);
  if (safe.bits) out.alignSafe = safe.bits;
  // Longhands win over the shorthand whatever their order, as in CSS.
  for (const axis of ['Column', 'Row']) {
    const line = lines[axis];
    if (!line) continue;
    const start = line.start ?? line.shorthand?.[0] ?? 'auto', end = line.end ?? line.shorthand?.[1] ?? 'auto';
    out[`grid${axis}`] = end === 'auto' ? start : `${start} / ${end}`;
  }
  // A multiplier recomputes against each node's font size; pixels inherit natively.
  if (style.lineHeight !== undefined || (lineHeight && typeof lineHeight === 'object' && !isStep(lineHeight))) {
    const pixels = lineHeight?.multiplier !== undefined ? times(fontSize, lineHeight.multiplier) : lineHeight;
    if (!isStep(pixels) && !(pixels > 0 && pixels <= 16384)) invalid('lineHeight', pixels, 'a height above 0 up to 16384 points');
    out.lineHeight = pixels;
  }
  // Each step goes only where its family applies: a radius step never sizes a padding.
  for (const key in out) {
    const value = out[key];
    if (!isStep(value)) continue;
    const { name, family } = scales[value[0]];
    if (!scaledFields[family].includes(key)) throw Error(`UI_SCALE: ${key} does not take the theme step ${name}; it takes ${Object.entries(scaledFields).filter(([, keys]) => keys.includes(key)).map(([family]) => family).join(', ') || 'no step'}`);
  }
  return { fields: out, typography: { fontSize, lineHeight } };
}

// A slider part as `[field, value, …]` pairs; sizes are plain points, as the part has no box to resolve against.
function partRecord(name, value, env) {
  if (!value || typeof value !== 'object' || Array.isArray(value)) invalid(name, value, 'a style object');
  value = Object.fromEntries(Object.entries(value).map(([key, item]) => [key, fold(item)]));
  for (const key of ['width', 'height']) if (value[key] !== undefined && !(finite(value[key]) && value[key] >= 0 && value[key] <= 1024)) invalid(`${name}.${key}`, value[key], 'a number from 0 to 1024');
  const pairs = [];
  for (const [key, lowered] of Object.entries(lowerStyle('view', value, env).fields))
    pairs.push(field[key], (key === 'boxShadow' || key === 'ring') && lowered !== 'none' ? JSON.stringify(lowered) : lowered);
  return JSON.stringify(pairs);
}

/** The paint subset of a style; state styles may carry nothing else. */
export function paintOnly(state, tag, style) {
  admitStyle(tag, style);
  for (const name of Object.keys(style))
    if (!table[name].paint) throw Error(`UI_STYLE: ${state}: supports paint properties only, not ${name}`);
}

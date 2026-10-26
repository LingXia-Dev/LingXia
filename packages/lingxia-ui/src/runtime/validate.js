import { utf8Bytes, has } from './data.js';
import { limits } from '../generated/contract.js';
import { fieldOptions } from '../generated/wire.js';

// Per-element value checks; native decoding re-validates everything it receives.
export function label(value, code, required = false) {
  if (value === undefined && !required) return;
  if (typeof value !== 'string' || !value.trim() || utf8Bytes(value, 1025) > 1024 || value.includes('\0'))
    throw Error(`${code}: ${required ? 'accessibilityLabel is required' : 'invalid accessibility label'}`);
}

const finiteWithin = (value, bound) => typeof value === 'number' && Number.isFinite(value) && Math.abs(value) <= bound;
const hexColor = value => typeof value === 'string' && /^#(?:[0-9a-f]{3}|[0-9a-f]{4}|[0-9a-f]{6}|[0-9a-f]{8})$/i.test(value);
const pathKeys = ['d', 'fill', 'stroke', 'strokeWidth', 'strokeDasharray', 'strokeDashoffset', 'strokeLinecap', 'strokeLinejoin', 'transform', 'vectorEffect'];
const gradientKeys = ['type', 'x1', 'y1', 'x2', 'y2', 'cx', 'cy', 'r', 'fx', 'fy', 'fr', 'units', 'spread', 'stops'];
const plain = value => value && typeof value === 'object' && !Array.isArray(value);

function vectorGradient(id, gradient) {
  const invalid = () => { throw Error(`UI_VECTOR: invalid gradient ${id}`); };
  if (!/^[A-Za-z0-9_-]{1,64}$/.test(id) || !plain(gradient) || Object.keys(gradient).some(k => !gradientKeys.includes(k)) ||
      !['linear', 'radial'].includes(gradient.type) ||
      (gradient.type === 'linear' ? ['cx', 'cy', 'r', 'fx', 'fy', 'fr'] : ['x1', 'y1', 'x2', 'y2']).some(k => gradient[k] !== undefined) ||
      ['x1', 'y1', 'x2', 'y2', 'cx', 'cy', 'fx', 'fy'].some(k => gradient[k] !== undefined && !finiteWithin(gradient[k], 16384)) ||
      ['r', 'fr'].some(k => gradient[k] !== undefined && (!finiteWithin(gradient[k], 16384) || gradient[k] < 0)) ||
      (gradient.units !== undefined && !['objectBoundingBox', 'userSpaceOnUse'].includes(gradient.units)) ||
      (gradient.spread !== undefined && !['pad', 'reflect', 'repeat'].includes(gradient.spread)) ||
      !Array.isArray(gradient.stops) || !gradient.stops.length || gradient.stops.length > 16 ||
      gradient.stops.some(stop => !plain(stop) || Object.keys(stop).some(k => !['offset', 'color', 'opacity'].includes(k)) ||
        !finiteWithin(stop.offset, 1) || stop.offset < 0 || !hexColor(stop.color) ||
        (stop.opacity !== undefined && (!finiteWithin(stop.opacity, 1) || stop.opacity < 0)))) invalid();
}

export function vector(props, fit) {
  const box = props.viewBox ?? [0, 0, 24, 24];
  if (!Array.isArray(box) || box.length !== 4 || box.some(v => !finiteWithin(v, 16384)) || box[2] < 1 / 64 || box[3] < 1 / 64)
    throw Error('UI_VECTOR: invalid viewBox');
  if (!Array.isArray(props.paths) || !props.paths.length || props.paths.length > 32) throw Error('UI_VECTOR: invalid paths');
  const gradients = props.gradients ?? {};
  if (!plain(gradients) || Object.keys(gradients).length > 8) throw Error('UI_VECTOR: at most 8 gradients');
  for (const [id, gradient] of Object.entries(gradients)) vectorGradient(id, gradient);
  for (const path of props.paths) {
    if (!plain(path) || Object.keys(path).some(k => !pathKeys.includes(k)) ||
        typeof path.d !== 'string' || !path.d.length || path.d.length > 16384) throw Error('UI_VECTOR: invalid path');
    for (const key of ['fill', 'stroke']) {
      const value = path[key];
      if (value === undefined || value === 'none' || value === 'currentColor' || hexColor(value)) continue;
      const id = typeof value === 'string' && value.match(/^url\(#(.+)\)$/)?.[1];
      if (!id) throw Error('UI_VECTOR: invalid color');
      if (!has(gradients, id)) throw Error(`UI_VECTOR: unknown gradient ${id}`);
    }
    if (path.vectorEffect !== undefined && !['none', 'non-scaling-stroke'].includes(path.vectorEffect)) throw Error('UI_VECTOR: invalid vector effect');
    if (path.strokeLinecap !== undefined && !['butt', 'round', 'square'].includes(path.strokeLinecap)) throw Error('UI_VECTOR: invalid stroke-linecap');
    if (path.strokeLinejoin !== undefined && !['miter', 'round', 'bevel'].includes(path.strokeLinejoin)) throw Error('UI_VECTOR: invalid stroke-linejoin');
    if (path.strokeDashoffset !== undefined && !finiteWithin(path.strokeDashoffset, 16384)) throw Error('UI_VECTOR: invalid dash offset');
    if (path.transform !== undefined && (typeof path.transform !== 'string' || path.transform.length > 512 ||
        !/^\s*(?:(?:matrix|translate|scale|rotate|skewX|skewY)\s*\([^()]*\)[\s,]*)+$/.test(path.transform))) throw Error('UI_VECTOR: invalid transform');
    if (path.strokeDasharray !== undefined && (!Array.isArray(path.strokeDasharray) || path.strokeDasharray.length > 16 ||
        path.strokeDasharray.some(v => typeof v !== 'number' || !Number.isFinite(v) || v < 1 / 64 || v > 1024))) throw Error('UI_VECTOR: invalid dash pattern');
    if (path.strokeWidth !== undefined && (typeof path.strokeWidth !== 'number' || !Number.isFinite(path.strokeWidth) || path.strokeWidth < 0 || path.strokeWidth > 1024))
      throw Error('UI_VECTOR: invalid stroke width');
  }
  const source = JSON.stringify({ viewBox: box, paths: props.paths, ...(Object.keys(gradients).length ? { gradients } : {}), ...(fit === undefined ? {} : { fit }) });
  if (source.length > 32768) throw Error('UI_VECTOR: source too large');
  return source;
}

export function plot(props) {
  if (['smooth', 'area'].some(name => props[name] !== undefined && typeof props[name] !== 'boolean') ||
      (props.kind !== 'line' && (props.smooth || props.area)))
    throw Error('UI_PLOT: smooth and area must be booleans on line plots');
  label(props.accessibilityLabel, 'UI_PLOT', true);
  if (!['bar', 'line', 'pie'].includes(props.kind) || !Array.isArray(props.values) ||
      props.values.length < 1 || props.values.length > 128 ||
      props.values.some(v => typeof v !== 'number' || !Number.isFinite(v) || Math.abs(v) > 1e9 || (props.kind === 'pie' && v < 0)))
    throw Error('UI_PLOT: invalid kind or values');
  if (props.markerEmphasisScale !== undefined && (props.kind !== 'line' || typeof props.markerEmphasisScale !== 'number' ||
      !Number.isFinite(props.markerEmphasisScale) || props.markerEmphasisScale < 1 || props.markerEmphasisScale > 4))
    throw Error('UI_PLOT: invalid marker emphasis scale');
  if (props.pieLayout !== undefined) {
    const layout = props.pieLayout;
    if (props.kind !== 'pie' || !layout || typeof layout !== 'object' || Array.isArray(layout) ||
        Object.keys(layout).some(key => !['center', 'innerRadius', 'outerRadius', 'emphasisScale'].includes(key)) ||
        !Array.isArray(layout.center) || layout.center.length !== 2 ||
        [...layout.center, layout.innerRadius, layout.outerRadius].some(value => typeof value !== 'number' || !Number.isFinite(value) || value < 0 || value > 1) ||
        layout.innerRadius >= layout.outerRadius ||
        (layout.emphasisScale !== undefined && (typeof layout.emphasisScale !== 'number' || !Number.isFinite(layout.emphasisScale) || layout.emphasisScale < 0 || layout.emphasisScale > 64)))
      throw Error('UI_PLOT: invalid pie layout');
  }
  if (props.domainMin !== undefined || props.domainMax !== undefined) {
    const lo = props.domainMin, hi = props.domainMax;
    if (props.kind === 'pie' || !Number.isFinite(lo) || !Number.isFinite(hi) || lo < -1e9 || hi > 1e9 || lo >= hi || lo > 0 || hi < 0 ||
        props.values.some(value => value < lo || value > hi))
      throw Error('UI_PLOT: domain must contain zero and all values');
  }
  if (props.onSelect !== undefined && typeof props.onSelect !== 'function') throw Error('UI_PLOT: onSelect must be a function');
  if (props.selectedIndex !== undefined && (!Number.isInteger(props.selectedIndex) || props.selectedIndex < -1 || props.selectedIndex >= props.values.length))
    throw Error('UI_PLOT: invalid selected index');
  if (!Array.isArray(props.colors) || !props.colors.length || props.colors.length > 16 ||
      props.colors.some(c => typeof c !== 'string' || !/^#[0-9a-fA-F]{6}([0-9a-fA-F]{2})?$/.test(c)))
    throw Error('UI_PLOT: expected 1..16 hex colors');
}

export function sliderValue({ min, max, step }, value) {
  value = Math.max(min, Math.min(max, value));
  const snapped = Math.max(min, Math.min(max, min + Math.floor((value - min) / step + 0.5) * step));
  return Math.abs(max - value) < Math.abs(snapped - value) ? max : snapped;
}

export function slider(props) {
  for (const [name, fallback] of [['min', 0], ['max', 100], ['step', 1]]) if (props[name] === undefined) props[name] = fallback;
  if (props.value === undefined) props.value = props.min;
  const { min, max, step, value } = props;
  if (![min, max, step, value].every(v => typeof v === 'number' && Number.isFinite(v) && Math.abs(v) <= 1e9) || max <= min || step <= 0 ||
      step > max - min || (max - min) / step > 1e6 || min + step === min || max - step === max)
    throw Error('UI_SLIDER: invalid range or precision');
  label(props.accessibilityLabel, 'UI_SLIDER', true);
  props.value = sliderValue(props, value);
}

export function image(props) {
  if (typeof props.src !== 'string' || !props.src.length || props.src.length > limits.sourceBytes || props.src.includes('\0')) throw Error('UI_IMAGE: invalid source');
  if (props.alt !== undefined && (typeof props.alt !== 'string' || utf8Bytes(props.alt, 1024) > 1024 || props.alt.includes('\0') ||
      (props.alt.length > 0 && !props.alt.trim()))) throw Error('UI_IMAGE: invalid alternative text');
  if (props.retryKey !== undefined && (typeof props.retryKey !== 'string' || utf8Bytes(props.retryKey, 257) > 256 || props.retryKey.includes('\0')))
    throw Error('UI_IMAGE: invalid retry key');
  for (const name of ['onLoad', 'onError']) if (props[name] !== undefined && typeof props[name] !== 'function') throw Error('UI_IMAGE: invalid callback');
}

export function listKeys(props) {
  if (!Number.isSafeInteger(props.count) || props.count < 0 || props.count > 100000 || typeof props.row !== 'function' ||
      (props.height !== undefined && typeof props.height !== 'function')) throw Error('UI_LIST: invalid row description');
  if (props.itemKeys !== undefined) {
    if (!Array.isArray(props.itemKeys) || props.itemKeys.length !== props.count) throw Error('UI_LIST: itemKeys must match count');
    const seen = new Set();
    props.itemKeys = Array.from(props.itemKeys, key => {
      if (typeof key !== 'string' || utf8Bytes(key, 256) > 256 || utf8Bytes(`key:${JSON.stringify(key)}`, 256) > 256 || seen.has(key))
        throw Error('UI_LIST: invalid or duplicate itemKey');
      seen.add(key); return key;
    });
    if (utf8Bytes(JSON.stringify(props.itemKeys), limits.itemKeyBytes) > limits.itemKeyBytes) throw Error('UI_LIST: itemKeys budget exceeded');
  }
  if (props.anchor !== undefined && props.anchor !== 'end') {
    const a = props.anchor;
    if (!a || typeof a !== 'object' || Object.keys(a).some(k => !['index', 'align', 'seq'].includes(k)) ||
        !Number.isSafeInteger(a.index) || a.index < 0 || !['start', 'end'].includes(a.align) ||
        (a.seq !== undefined && (!Number.isSafeInteger(a.seq) || a.seq < 0))) throw Error('UI_LIST: invalid anchor');
    props.anchor = { index: a.index, align: a.align, ...(a.seq === undefined ? {} : { seq: a.seq }) };
  }
}

const iso = { date: /^\d{4}-(0[1-9]|1[0-2])-(0[1-9]|[12]\d|3[01])$/, time: /^([01]\d|2[0-3]):[0-5]\d(:[0-5]\d)?$/ };

/** Editor attributes as wire pairs; native enforces length and range again. */
export function editor(tag, props) {
  const out = [];
  const keyboard = props.keyboard;
  if (keyboard !== undefined && (keyboard === 'search' || !fieldOptions.keyboard.includes(keyboard))) throw Error('UI_INPUT: keyboard');
  if ((keyboard === 'date' || keyboard === 'time') && tag !== 'input') throw Error(`UI_INPUT: ${keyboard} pickers need <input>`);
  if (props.maxLength !== undefined) {
    if (!Number.isInteger(props.maxLength) || props.maxLength < 1 || props.maxLength > 65536) throw Error('UI_INPUT: maxLength is an integer 1..65536');
    out.push(['maxLength', props.maxLength]);
  }
  for (const [name, key] of [['autoComplete', 'autoComplete'], ['enterKeyHint', 'enterKeyHint']]) {
    if (props[name] === undefined) continue;
    if (!fieldOptions[key].includes(props[name])) throw Error(`UI_INPUT: ${name} must be one of ${fieldOptions[key].join(', ')}`);
    out.push([key, props[name]]);
  }
  if (props.autoCapitalize !== undefined) {
    const value = { off: 'none', on: 'sentences' }[props.autoCapitalize] ?? props.autoCapitalize;
    if (!fieldOptions.autoCapitalize.includes(value)) throw Error('UI_INPUT: autoCapitalize');
    out.push(['autoCapitalize', value]);
  }
  if (props.spellCheck !== undefined) {
    if (typeof props.spellCheck !== 'boolean') throw Error('UI_INPUT: spellCheck must be boolean');
    out.push(['spellCheck', +props.spellCheck]);
  }
  const bounded = ['min', 'max', 'step'].filter(name => props[name] !== undefined);
  if (bounded.length) {
    if (tag !== 'input' || !['number', 'date', 'time'].includes(keyboard)) throw Error('UI_INPUT: min, max and step need keyboard number, date or time');
    const valid = value => keyboard === 'number' ? typeof value === 'number' && Number.isFinite(value) && Math.abs(value) <= 1e15
      : typeof value === 'string' && iso[keyboard].test(value);
    for (const name of ['min', 'max']) if (props[name] !== undefined && !valid(props[name])) throw Error(`UI_INPUT: ${name} must be a ${keyboard === 'number' ? 'number' : `${keyboard} in ISO form`}`);
    if (props.min !== undefined && props.max !== undefined && props.min > props.max) throw Error('UI_INPUT: min exceeds max');
    if (props.step !== undefined && (keyboard !== 'number' || typeof props.step !== 'number' || !(props.step > 0) || props.step > 1e15))
      throw Error('UI_INPUT: step is a positive number on keyboard="number"');
    for (const [name, key] of [['min', 'inputMin'], ['max', 'inputMax'], ['step', 'inputStep']]) if (props[name] !== undefined) out.push([key, props[name]]);
  }
  return out;
}

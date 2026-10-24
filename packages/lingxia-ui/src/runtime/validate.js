import { utf8Bytes } from './data.js';

// Per-element value checks; native decoding re-validates everything it receives.
export function label(value, code, required = false) {
  if (value === undefined && !required) return;
  if (typeof value !== 'string' || !value.trim() || utf8Bytes(value, 1025) > 1024 || value.includes('\0'))
    throw Error(`${code}: ${required ? 'accessibilityLabel is required' : 'invalid accessibility label'}`);
}

export function vector(props, fit) {
  const box = props.viewBox ?? [0, 0, 24, 24];
  if (!Array.isArray(box) || box.length !== 4 || box.some(v => typeof v !== 'number' || !Number.isFinite(v) || Math.abs(v) > 16384) || box[2] < 1 / 64 || box[3] < 1 / 64)
    throw Error('UI_VECTOR: invalid viewBox');
  if (!Array.isArray(props.paths) || !props.paths.length || props.paths.length > 32) throw Error('UI_VECTOR: invalid paths');
  for (const path of props.paths) {
    if (!path || typeof path !== 'object' || Array.isArray(path) || Object.keys(path).some(k => !['d', 'fill', 'stroke', 'strokeWidth', 'strokeDasharray', 'vectorEffect'].includes(k)) ||
        typeof path.d !== 'string' || !path.d.length || path.d.length > 16384) throw Error('UI_VECTOR: invalid path');
    for (const key of ['fill', 'stroke']) if (path[key] !== undefined && (typeof path[key] !== 'string' ||
        (!['none', 'currentColor'].includes(path[key]) && !/^#(?:[0-9a-f]{3}|[0-9a-f]{4}|[0-9a-f]{6}|[0-9a-f]{8})$/i.test(path[key])))) throw Error('UI_VECTOR: invalid color');
    if (path.vectorEffect !== undefined && !['none', 'non-scaling-stroke'].includes(path.vectorEffect)) throw Error('UI_VECTOR: invalid vector effect');
    if (path.strokeDasharray !== undefined && (!Array.isArray(path.strokeDasharray) || path.strokeDasharray.length > 16 ||
        path.strokeDasharray.some(v => typeof v !== 'number' || !Number.isFinite(v) || v < 1 / 64 || v > 1024))) throw Error('UI_VECTOR: invalid dash pattern');
    if (path.strokeWidth !== undefined && (typeof path.strokeWidth !== 'number' || !Number.isFinite(path.strokeWidth) || path.strokeWidth < 0 || path.strokeWidth > 1024))
      throw Error('UI_VECTOR: invalid stroke width');
  }
  const source = JSON.stringify({ viewBox: box, paths: props.paths, ...(fit === undefined ? {} : { fit }) });
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
  if (typeof props.src !== 'string' || !props.src.length || props.src.length > 8192 || props.src.includes('\0')) throw Error('UI_IMAGE: invalid source');
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
    if (utf8Bytes(JSON.stringify(props.itemKeys), 262144) > 262144) throw Error('UI_LIST: itemKeys budget exceeded');
  }
  if (props.anchor !== undefined && props.anchor !== 'end') {
    const a = props.anchor;
    if (!a || typeof a !== 'object' || Object.keys(a).some(k => !['index', 'align', 'seq'].includes(k)) ||
        !Number.isSafeInteger(a.index) || a.index < 0 || !['start', 'end'].includes(a.align) ||
        (a.seq !== undefined && (!Number.isSafeInteger(a.seq) || a.seq < 0))) throw Error('UI_LIST: invalid anchor');
    props.anchor = { index: a.index, align: a.align, ...(a.seq === undefined ? {} : { seq: a.seq }) };
  }
}

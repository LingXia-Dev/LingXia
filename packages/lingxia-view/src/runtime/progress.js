// `<Progress>`: an amount done, as HTML's progress. A bar (a filled view) or a ring (a vector arc);
// without `value` it is indeterminate and moves on its own. A screen reader hears its share as a
// percentage unless `aria-valuetext` says otherwise.
import { h } from './nodes.js';

const fillMotion = { properties: ['layout'], durationMs: 200, delayMs: 0, easing: [0.4, 0, 0.2, 1] };
// A segment two-fifths of the track wide sweeps it end to end; reduced motion shows it still.
const slide = { name: 'lx-progress-slide', durationMs: 1200, delayMs: 0, iterations: null, whenReduced: 'base',
  easing: [0.42, 0, 0.58, 1], opacity: null,
  transforms: [{ kind: 'translate', x: [{ offset: 0, value: -1 }, { offset: 1, value: 2.5 }], y: [{ offset: 0, value: 0 }, { offset: 1, value: 0 }], relative: true }] };
const spin = { name: 'lx-progress-spin', durationMs: 900, delayMs: 0, iterations: null, whenReduced: 'base', easing: 'linear', opacity: null,
  transforms: [{ kind: 'rotate', angle: [{ offset: 0, value: 0 }, { offset: 1, value: 360 }] }] };
const circle = 'M12 2a10 10 0 1 1 0 20a10 10 0 1 1 0-20';
const around = 2 * Math.PI * 10;

export function Progress({ value, max = 100, variant = 'bar', size = 40, 'aria-label': label, 'aria-valuetext': text,
  className, 'data-testid': testId }) {
  if (typeof label !== 'string' || !label) throw Error('UI_PROP: aria-label is required');
  if (typeof max !== 'number' || !Number.isFinite(max) || max <= 0) throw Error('UI_PROP: max must be a positive number');
  if (value !== undefined && (typeof value !== 'number' || !Number.isFinite(value))) throw Error('UI_PROP: value must be a finite number');
  if (variant !== 'bar' && variant !== 'ring') throw Error('UI_PROP: variant is "bar" or "ring"');
  const share = value === undefined ? undefined : Math.min(1, Math.max(0, value / max));
  const semantics = { role: 'progressbar', 'aria-label': label, 'data-testid': testId,
    ...(value === undefined ? {} : { 'aria-valuenow': Math.min(max, Math.max(0, value)), 'aria-valuemin': 0, 'aria-valuemax': max }),
    ...(text === undefined || value === undefined ? {} : { 'aria-valuetext': text }) };
  if (variant === 'ring') {
    const arc = share === undefined ? around / 4 : around * share;
    return h('view', { ...semantics, className, style: { width: size, height: size } },
      h('vector', { 'aria-hidden': true, 'data-testid': testId && `${testId}-arc`,
        style: { width: size, height: size, ...(share === undefined ? { animation: spin } : {}) },
        paths: [
          { d: circle, fill: 'none', stroke: 'line', strokeWidth: 2.5 },
          { d: circle, fill: 'none', stroke: 'accent', strokeWidth: 2.5, strokeLinecap: 'round', strokeDasharray: [arc, around] },
        ] }));
  }
  return h('view', { ...semantics, className,
    style: { height: 4, borderRadius: 2, backgroundColor: 'surface-sunken', overflow: 'hidden' } },
    h('view', { 'data-testid': testId && `${testId}-fill`, style: share === undefined
      ? { width: '40%', height: '100%', borderRadius: 2, backgroundColor: 'accent', animation: slide }
      : { width: `${share * 100}%`, height: '100%', borderRadius: 2, backgroundColor: 'accent', transition: fillMotion } }));
}
Progress.__lxid = '@lingxia/view#Progress';

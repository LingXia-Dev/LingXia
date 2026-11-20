// Compiled Tailwind rules, matched per node. Rules carry CSS-named style;
// `when` narrows them by theme, viewport width, interaction state or the
// children a `space-*`/`divide-*` rule targets, or every child (`*:` grid placement).
import { resetLonghands } from './lower.js';

let byName = new Map();
const cache = new Map();
const states = ['focus', 'focusVisible', 'active', 'hover', 'disabled', 'starting', 'exit', 'hasChecked', 'hasDisabled', 'focusWithin', 'groupActive'];
const parts = ['track', 'range', 'secondary', 'thumb'];
const geometryKeys = { linear: ['direction', 'angle'], radial: ['shape', 'size', 'at'], conic: ['from', 'at'] };

function copy(value) {
  if (Array.isArray(value)) return value.map(copy);
  if (value && typeof value === 'object') return Object.fromEntries(Object.entries(value).map(([key, item]) => [key, copy(item)]));
  return value;
}

/** Bumped with each class table: anything lowered under an older one is stale. */
export let classGeneration = 0;

// A class with a theme condition (`dark:`) lowers differently per appearance.
let themed = false;
export const classesReadTheme = () => themed;

export function configureClasses(compiled) {
  classGeneration++;
  byName = new Map();
  cache.clear();
  themed = compiled.some(rule => rule.when?.theme !== undefined);
  compiled.forEach((rule, order) => {
    if (!byName.has(rule.name)) byName.set(rule.name, []);
    byName.get(rule.name).push({ ...copy(rule), order });
  });
}

function applies(rule, env) {
  const { theme, minWidth, maxWidth, direction } = rule.when ?? {};
  if (theme && (env.theme ?? 'light') !== theme) return false;
  if (direction && (env.direction ?? 'ltr') !== direction) return false;
  if (rule.when?.structure && !(env.structure ?? []).includes(rule.when.structure)) return false;
  if (rule.when?.checked && env.checked !== true) return false;
  if (rule.when?.indeterminate && env.indeterminate !== true) return false;
  if (rule.when?.data) {
    const [name, value] = rule.when.data, own = env.data?.[name];
    if (own === undefined || (value !== undefined && own !== value)) return false;
  }
  if (rule.when?.reducedMotion !== undefined && (env.reducedMotion === true) !== rule.when.reducedMotion) return false;
  if (minWidth !== undefined && !(env.viewport?.width >= minWidth)) return false;
  if (maxWidth !== undefined && !(env.viewport?.width < maxWidth)) return false;
  return true;
}

const pixels = value => `${value}px`;

// Tailwind composes gradients, shadows and filters from independently cascading
// variables; the cascade resolves them before they become one style value.
function cascade(rules, meta) {
  const style = {}, gradient = {}, shadow = {}, backdrop = {}, transform = {}, motion = {}, mask = {};
  let numeric = {};
  for (const rule of rules) {
    // Each class is its own layer: a later shorthand (`rounded-full`) clears earlier longhands.
    if (rule.style) resetLonghands(style, rule.style);
    Object.assign(style, copy(rule.style));
    if (meta && rule.style?.lineHeight !== undefined) meta.pairedLeading = rule.pairedLeading === true;
    Object.assign(motion, rule.motion);
    Object.assign(transform, rule.transform);
    // `normal-nums` and literal values replace the composed keywords; utilities add to them.
    if (rule.numeric?.normal || rule.numeric?.fixed) numeric = { ...rule.numeric };
    else if (rule.numeric) numeric = { ...numeric, ...rule.numeric, normal: false, fixed: undefined };
    Object.assign(gradient, rule.gradient);
    Object.assign(shadow, rule.shadow);
    Object.assign(backdrop, rule.backdrop);
    Object.assign(mask, rule.mask);
  }
  const filter = backdrop.mode === 'fixed' ? backdrop.fixed : backdrop.mode === 'variables' ? backdrop : undefined;
  if (filter && (filter.blur != null || filter.saturation != null))
    style.backdropFilter = `blur(${pixels(filter.blur ?? 0)}) saturate(${filter.saturation ?? 1})`;
  // Durations and easings from duration-*/ease-* win over a transition-* default whatever the order.
  const properties = [...new Set([...(motion.properties ?? []), ...(motion.layout ? ['layout'] : []), ...(motion.size ? ['size'] : [])])];
  if (properties.length) style.transition = {
    properties, durationMs: motion.duration ?? motion.durationDefault ?? 150, delayMs: motion.delay ?? 0,
    easing: copy(motion.easing ?? motion.easingDefault ?? [0.4, 0, 0.2, 1]),
  };
  else if (motion.properties) style.transition = 'none';
  // Tailwind gives bare duration-*/ease-* no transition; a drag's runtime-owned release still reads them.
  else if (meta && (motion.duration !== undefined || motion.easing !== undefined))
    meta.release = { durationMs: motion.duration ?? 300, easing: copy(motion.easing ?? [0.4, 0, 0.2, 1]) };
  // Edge masks fade from their `from` stop (opaque at the start by default) to their `to` stop.
  if (mask.none) style.maskImage = 'none';
  else if (mask.fixed) style.maskImage = copy(mask.fixed);
  else if (mask.edges) {
    const layers = ['top', 'bottom', 'left', 'right'].filter(side => mask[side]).map(side => ({ to: side, stops: [
      { alpha: mask[`${side}-from-color`] ?? 1, ...(mask[`${side}-from-position`] ?? { offset: 0 }) },
      { alpha: mask[`${side}-to-color`] ?? 0, ...(mask[`${side}-to-position`] ?? { offset: 1 }) },
    ] }));
    if (layers.length) style.maskImage = layers;
  }
  if (numeric.normal) style.fontVariantNumeric = 'normal';
  else if (numeric.fixed) style.fontVariantNumeric = numeric.fixed;
  else if (numeric.enabled) {
    const keywords = [numeric.ordinal, numeric.slashedZero, numeric.figure, numeric.spacing, numeric.fraction].filter(Boolean);
    style.fontVariantNumeric = keywords.length ? keywords.join(' ') : 'normal';
  }
  if (transform.translate) style.translate = [transform.translateX ?? 0, transform.translateY ?? 0];
  if (transform.scale) style.scale = [transform.scaleX ?? 1, transform.scaleY ?? 1];
  if (gradient.enabled !== undefined) {
    style.backgroundImage = 'none';
    if (gradient.enabled && gradient.fixed) style.backgroundImage = copy(gradient.fixed);
    else if (gradient.enabled && gradient.source && (gradient.source !== 'via' || gradient.via)) {
      const stops = [{ offset: gradient.fromOffset ?? 0, color: gradient.fromColor ?? '#00000000' }];
      if (gradient.via) stops.push({ offset: gradient.viaOffset ?? .5, color: gradient.viaColor ?? '#00000000' });
      stops.push({ offset: gradient.toOffset ?? 1, color: gradient.toColor ?? '#00000000' });
      // CSS stop fixup clamps later positions to the largest preceding position.
      for (let i = 1; i < stops.length; i++) stops[i].offset = Math.max(stops[i - 1].offset, stops[i].offset);
      const kind = gradient.kind ?? 'linear';
      const { colorSpace = 'srgb', ...geometry } = gradient.position ?? {};
      if (Object.keys(geometry).some(key => !geometryKeys[kind].includes(key)))
        throw Error(`UI_GRADIENT: ${kind} gradients do not take ${Object.keys(geometry).join(', ')}`);
      if (kind === 'linear' && geometry.angle === undefined && geometry.direction === undefined) geometry.direction = 'bottom';
      style.backgroundImage = { ...(kind === 'linear' ? {} : { kind }), ...geometry, colorSpace, stops };
    }
  }
  if (shadow.enabled) {
    style.boxShadow = (shadow.composite === false ? shadow.fixedLayers ?? [] : shadow.layers ?? []).map(({ useColor, ...layer }) => ({
      ...layer, color: useColor && shadow.color !== undefined ? shadow.color : layer.color,
    }));
    if (style.boxShadow.length > 8) throw Error('UI_STYLE: too many box shadows');
    // Rings are strokes, not shadow layers: the offset is a real gap unless a role fills it.
    const ring = {};
    if (shadow.composite !== false && shadow.ringWidth > 0) {
      ring.outer = { width: shadow.ringWidth, offset: shadow.ringOffset ?? 0, color: shadow.ringColor ?? 'currentColor' };
      if (shadow.ringOffsetColor !== undefined) ring.outer.offsetColor = shadow.ringOffsetColor;
    }
    if (shadow.composite !== false && shadow.insetRingWidth > 0)
      ring.inner = { width: shadow.insetRingWidth, color: shadow.insetRingColor ?? 'currentColor' };
    style.ring = ring.outer || ring.inner ? ring : 'none';
  }
  return style;
}

/** `className` in two: the classes with a rule `pick` holds for, and the rest; both keep their order. */
export function partitionClasses(className, pick) {
  const picked = [], rest = [];
  for (const name of typeof className === 'string' ? className.split(/\s+/).filter(Boolean) : [])
    (byName.get(name)?.some(pick) ? picked : rest).push(name);
  return [picked.join(' '), rest.join(' ')];
}

function changed(full, base) {
  const delta = {};
  for (const [key, value] of Object.entries(full)) if (JSON.stringify(value) !== JSON.stringify(base[key])) delta[key] = value;
  return delta;
}

/**
 * The class cascade for one node: `base`, the full style under `disabled`,
 * paint deltas under `focus`/`active`/`hover`, and the style its non-last
 * children receive. Only a mouse, stylus or trackpad hovers, so `hover:` needs no pointer gate.
 */
export function resolveClasses(className, env = {}) {
  if (className === undefined) return undefined;
  if (typeof className !== 'string') throw Error('UI_CLASS: className must be a string');
  if (className.length > 16384) throw Error('UI_CLASS: className exceeds 16384 characters');
  const environment = `${env.theme ?? 'light'}|${env.viewport?.width}|${env.viewport?.height}|${env.direction ?? 'ltr'}|${(env.structure ?? []).join(',')}|${env.reducedMotion === true}|${env.data ? JSON.stringify(env.data) : ''}|${env.checked === true}|${env.indeterminate === true}`;
  // Keyed by environment too: sibling structure changes it node to node, so clearing on change thrashed.
  const key = `${environment}\u0000${className}`;
  if (cache.has(key)) return copy(cache.get(key));
  const matched = [];
  const tokens = className.split(/\s+/).filter(Boolean);
  // A repeated class counts where it last appears, as in tailwind-merge.
  const position = new Map(tokens.map((name, index) => [name, index]));
  for (const name of position.keys()) {
    const rules = byName.get(name);
    if (!rules) throw Error(`UI_CLASS: ${name} was not compiled; use complete utility names in source`);
    for (const rule of rules) {
      if (rule.error) throw Error(`UI_CLASS: ${name}: ${rule.error.message}`);
      if (applies(rule, env)) matched.push({ ...rule, position: position.get(name) });
    }
  }
  // A pseudo-class adds specificity above media-only rules; source order breaks ties.
  const specific = rule => Number(!!rule.when?.state) + Number(!!rule.when?.checked);
  matched.sort((a, b) => specific(a) - specific(b) || a.order - b.order);
  // Among rules under the same variants, the class written later wins, so a caller's class
  // overrides a component's (cva + tailwind-merge); different variants keep Tailwind's order.
  const slots = new Map();
  matched.forEach((rule, index) => {
    const variant = JSON.stringify(rule.when ?? {});
    if (!slots.has(variant)) slots.set(variant, []);
    slots.get(variant).push(index);
  });
  const ordered = [...matched];
  for (const indices of slots.values()) {
    const group = indices.map(index => matched[index]).sort((a, b) => a.position - b.position || a.order - b.order);
    indices.forEach((index, at) => { ordered[index] = group[at]; });
  }
  matched.splice(0, matched.length, ...ordered);
  const own = matched.filter(rule => !rule.when?.children && !rule.when?.every && !rule.when?.part);
  const plain = own.filter(rule => !rule.when?.state);
  const meta = {};
  const base = cascade(plain, meta);
  const result = { base, ...(meta.pairedLeading ? { pairedLeading: true } : {}), ...(meta.release ? { release: meta.release } : {}) };
  for (const state of states) {
    const extra = own.filter(rule => rule.when?.state === state);
    if (!extra.length) continue;
    const full = cascade([...plain, ...extra]);
    result[state] = state === 'disabled' ? full : changed(full, base);
  }
  // A control part resolves on its own; a state carries the part whole, as that state draws it.
  for (const part of parts) {
    const rules = matched.filter(rule => rule.when?.part === part);
    if (!rules.length) continue;
    const key = `${part}Part`, plainPart = rules.filter(rule => !rule.when.state);
    const rest = cascade(plainPart);
    if (Object.keys(rest).length) base[key] = rest;
    for (const state of ['focusVisible', 'active', 'hover', 'disabled']) {
      const extra = rules.filter(rule => rule.when.state === state);
      if (!extra.length) continue;
      const full = cascade([...plainPart, ...extra]);
      if (state === 'disabled') (result.disabled ??= { ...base })[key] = full;
      else if (JSON.stringify(full) !== JSON.stringify(rest)) (result[state] ??= {})[key] = full;
    }
    if (result.disabled && !result.disabled[key] && base[key]) result.disabled[key] = base[key];
  }
  // `backdrop:` styles a dialog's backdrop on its own, not as a delta of the dialog.
  const backdrop = own.filter(rule => rule.when?.state === 'backdrop');
  if (backdrop.length) {
    const plainBackdrop = backdrop.filter(rule => !rule.when.presence);
    const backdropBase = cascade(plainBackdrop);
    result.backdrop = { base: backdropBase };
    // Bare duration-*/ease-*/delay-* retime the backdrop's default fade.
    const motion = Object.assign({}, ...plainBackdrop.map(rule => rule.motion ?? {}));
    const timing = { ...(motion.duration !== undefined ? { durationMs: motion.duration } : {}), ...(motion.delay !== undefined ? { delayMs: motion.delay } : {}),
      ...(motion.easing !== undefined ? { easing: copy(motion.easing) } : {}) };
    if (Object.keys(timing).length) result.backdrop.timing = timing;
    for (const presence of ['starting', 'exit']) {
      const extra = backdrop.filter(rule => rule.when.presence === presence);
      if (extra.length) result.backdrop[presence] = changed(cascade([...plainBackdrop, ...extra]), backdropBase);
    }
  }
  const children = matched.filter(rule => rule.when?.children);
  if (children.length) result.children = cascade(children);
  const every = matched.filter(rule => rule.when?.every);
  if (every.length) result.every = cascade(every);
  // Cache only the class cascade; inline values and inheritance resolve per node.
  if (cache.size >= 1024) cache.delete(cache.keys().next().value);
  cache.set(key, copy(result));
  return result;
}

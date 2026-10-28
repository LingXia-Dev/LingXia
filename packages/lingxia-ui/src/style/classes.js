// Compiled Tailwind rules, matched per node. Rules carry CSS-named style;
// `when` narrows them by theme, viewport width, interaction state or the
// children a `space-*`/`divide-*` rule targets.
let byName = new Map();
const cache = new Map();
const states = ['focus', 'active', 'hover', 'disabled', 'starting', 'exit', 'hasChecked', 'hasDisabled', 'focusWithin'];
const geometryKeys = { linear: ['direction', 'angle'], radial: ['shape', 'size', 'at'], conic: ['from', 'at'] };

function copy(value) {
  if (Array.isArray(value)) return value.map(copy);
  if (value && typeof value === 'object') return Object.fromEntries(Object.entries(value).map(([key, item]) => [key, copy(item)]));
  return value;
}

export function configureClasses(compiled) {
  byName = new Map();
  cache.clear();
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
  const style = {}, gradient = {}, shadow = {}, backdrop = {}, transform = {}, motion = {};
  let numeric = {};
  for (const rule of rules) {
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
  }
  const filter = backdrop.mode === 'fixed' ? backdrop.fixed : backdrop.mode === 'variables' ? backdrop : undefined;
  if (filter && (filter.blur != null || filter.saturation != null))
    style.backdropFilter = `blur(${pixels(filter.blur ?? 0)}) saturate(${filter.saturation ?? 1})`;
  // Durations and easings from duration-*/ease-* win over a transition-* default whatever the order.
  const properties = [...(motion.properties ?? []), ...(motion.layout ? ['layout'] : [])];
  if (properties.length) style.transition = {
    properties, durationMs: motion.duration ?? motion.durationDefault ?? 150, delayMs: motion.delay ?? 0,
    easing: copy(motion.easing ?? motion.easingDefault ?? [0.4, 0, 0.2, 1]),
  };
  else if (motion.properties) style.transition = 'none';
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
    if (shadow.composite !== false && shadow.ringWidth > 0)
      style.boxShadow.unshift({ offsetX: 0, offsetY: 0, blur: 0, spread: shadow.ringWidth, color: shadow.ringColor ?? 'currentColor' });
    if (style.boxShadow.length > 8) throw Error('UI_STYLE: too many box shadows');
  }
  return style;
}

function changed(full, base) {
  const delta = {};
  for (const [key, value] of Object.entries(full)) if (JSON.stringify(value) !== JSON.stringify(base[key])) delta[key] = value;
  return delta;
}

/**
 * The class cascade for one node: `base`, the full style under `disabled`,
 * paint deltas under `focus`/`active`/`hover`, and the style its non-last
 * children receive. Hover applies only to fine pointers.
 */
export function resolveClasses(className, env = {}) {
  if (className === undefined) return undefined;
  if (typeof className !== 'string') throw Error('UI_CLASS: className must be a string');
  if (className.length > 16384) throw Error('UI_CLASS: className exceeds 16384 characters');
  const environment = `${env.theme ?? 'light'}|${env.viewport?.width}|${env.viewport?.height}|${env.pointer}|${env.direction ?? 'ltr'}|${(env.structure ?? []).join(',')}|${env.reducedMotion === true}|${env.data ? JSON.stringify(env.data) : ''}`;
  // Keyed by environment too: sibling structure changes it node to node, so clearing on change thrashed.
  const key = `${environment}\u0000${className}`;
  if (cache.has(key)) return copy(cache.get(key));
  const matched = [];
  for (const name of new Set(className.split(/\s+/).filter(Boolean))) {
    const rules = byName.get(name);
    if (!rules) throw Error(`UI_CLASS: ${name} was not compiled; use complete utility names in source`);
    for (const rule of rules) {
      if (rule.error) throw Error(`UI_CLASS: ${name}: ${rule.error.message}`);
      if (applies(rule, env)) matched.push(rule);
    }
  }
  // A pseudo-class adds specificity above media-only rules; source order breaks ties.
  matched.sort((a, b) => Number(!!a.when?.state) - Number(!!b.when?.state) || a.order - b.order);
  const own = matched.filter(rule => !rule.when?.children);
  const plain = own.filter(rule => !rule.when?.state);
  const meta = {};
  const base = cascade(plain, meta);
  const result = { base, ...(meta.pairedLeading ? { pairedLeading: true } : {}) };
  for (const state of states) {
    const extra = own.filter(rule => rule.when?.state === state);
    if (!extra.length || (state === 'hover' && env.pointer !== 'fine')) continue;
    const full = cascade([...plain, ...extra]);
    result[state] = state === 'disabled' ? full : changed(full, base);
  }
  const children = matched.filter(rule => rule.when?.children);
  if (children.length) result.children = cascade(children);
  // Cache only the class cascade; inline values and inheritance resolve per node.
  if (cache.size >= 1024) cache.delete(cache.keys().next().value);
  cache.set(key, copy(result));
  return result;
}

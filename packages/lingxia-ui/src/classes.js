let byName = new Map();
const styles = new Map();
let styleTheme, styleWidth;
function copy(value) {
  if (Array.isArray(value)) return value.map(copy);
  if (value && typeof value === 'object') return Object.fromEntries(Object.entries(value).map(([key,item]) => [key,copy(item)]));
  return value;
}

export function configureClasses(compiled) {
  byName = new Map();
  styles.clear();
  compiled.forEach((rule, order) => {
    if (!byName.has(rule.name)) byName.set(rule.name, []);
    byName.get(rule.name).push({ ...copy(rule), order });
  });
}

export function classStyle(className, env = {}, focused = false) {
  if (className === undefined) return {};
  if (typeof className !== 'string') throw Error('UI_CLASS: className must be a string');
  if (className.length > 16384) throw Error('UI_CLASS: className exceeds 16384 characters');
  const theme = env.theme ?? 'light', width = env.viewport?.width;
  if (theme !== styleTheme || width !== styleWidth) {
    styles.clear(); styleTheme = theme; styleWidth = width;
  }
  const cacheKey = `${focused ? '1' : '0'}${className}`;
  if (styles.has(cacheKey)) return copy(styles.get(cacheKey));
  const names = new Set(className.split(/\s+/).filter(Boolean));
  const matched = [];
  for (const name of names) {
    if (!byName.has(name)) throw Error(`UI_CLASS: ${name} was not compiled; use complete utility names in source`);
    matched.push(...byName.get(name));
  }
  // :focus adds one pseudo-class specificity, above ordinary media-only rules.
  matched.sort((a, b) => Number(!!a.when?.focus) - Number(!!b.when?.focus) || a.order - b.order);
  const style = {}, gradient = {}, shadow = {}, backdrop = {};
  for (const rule of matched) {
    if (rule.error) throw Error(`UI_CLASS: ${rule.name}: ${rule.error}`);
    const { theme, minWidth, maxWidth, focus } = rule.when;
    if (focus && !focused) continue;
    if (theme && (env.theme ?? 'light') !== theme) continue;
    if (minWidth !== undefined && !(env.viewport?.width >= minWidth)) continue;
    if (maxWidth !== undefined && !(env.viewport?.width < maxWidth)) continue;
    Object.assign(style, copy(rule.style));
    Object.assign(gradient, rule.gradient);
    Object.assign(shadow, rule.shadow);
    Object.assign(backdrop, rule.backdrop);
  }
  const filter = backdrop.mode === 'fixed' ? backdrop.fixed : backdrop.mode === 'variables' ? backdrop : undefined;
  if (filter && (filter.blur != null || filter.saturation != null)) {
    style.backdropBlur = filter.blur ?? 0;
    style.backdropSaturation = filter.saturation ?? 1;
  }
  if (gradient.enabled !== undefined) {
    style.backgroundGradient = 'none';
    if (gradient.enabled && gradient.source && (gradient.source !== 'via' || gradient.via)) {
      const stops = [{offset:gradient.fromOffset ?? 0, color:gradient.fromColor ?? '#00000000'}];
      if (gradient.via) stops.push({offset:gradient.viaOffset ?? .5, color:gradient.viaColor ?? '#00000000'});
      stops.push({offset:gradient.toOffset ?? 1, color:gradient.toColor ?? '#00000000'});
      // CSS stop fixup clamps later positions to the largest preceding position.
      for (let i=1; i<stops.length; i++) stops[i].offset = Math.max(stops[i-1].offset, stops[i].offset);
      style.backgroundGradient = {...(gradient.orientation ?? {direction:'bottom'}), colorSpace:gradient.colorSpace ?? 'srgb', stops};
    }
  }
  if (shadow.enabled) {
    style.boxShadow = (shadow.composite === false ? shadow.fixedLayers ?? [] : shadow.layers ?? []).map(({useColor,...layer}) => ({
      ...layer, color:useColor && shadow.color !== undefined ? shadow.color : layer.color,
    }));
    if (shadow.composite !== false && shadow.ringWidth > 0) style.boxShadow.unshift({offsetX:0,offsetY:0,blur:0,spread:shadow.ringWidth,color:shadow.ringColor ?? 'currentColor'});
    if (style.boxShadow.length > 8) throw Error('UI_STYLE: too many box shadows');
  }
  // Cache only the class cascade; inline values and inherited typography resolve
  // per node. Bound dynamic class combinations and never share mutable styles.
  if (styles.size >= 256) styles.delete(styles.keys().next().value);
  styles.set(cacheKey, copy(style));
  return style;
}

export function resolveStyle(className, inline, env, inherited = { fontSize: 14 }, focused = false) {
  const style = classStyle(className, env, focused);
  if (inline?.padding !== undefined && className !== undefined)
    for (const side of ['paddingTop', 'paddingRight', 'paddingBottom', 'paddingLeft']) style[side] = inline.padding;
  if (inline?.borderWidth !== undefined && className !== undefined)
    for (const side of ['borderTopWidth', 'borderRightWidth', 'borderBottomWidth', 'borderLeftWidth']) style[side] = inline.borderWidth;
  if (inline?.radius !== undefined && className !== undefined)
    for (const corner of ['radiusTopLeft','radiusTopRight','radiusBottomRight','radiusBottomLeft']) style[corner] = inline.radius;
  Object.assign(style, inline);
  const typography = {
    fontSize: style.fontSize ?? inherited.fontSize,
    lineHeight: style.lineHeight ?? inherited.lineHeight,
  };
  // CSS unitless line height inherits its multiplier, not the parent's pixels.
  if (typography.lineHeight && typeof typography.lineHeight === 'object') {
    if (inline?.lineHeight !== undefined) throw Error('UI_STYLE_VALUE: lineHeight');
    style.lineHeight = typography.fontSize * typography.lineHeight.em;
  }
  if (style.letterSpacing && typeof style.letterSpacing === 'object') {
    if (inline?.letterSpacing !== undefined) throw Error('UI_STYLE_VALUE: letterSpacing');
    // Relative spacing computes once on its owner; descendants inherit pixels.
    style.letterSpacing = typography.fontSize * style.letterSpacing.em;
  }
  return { style, typography };
}


export function focusStyle(className, inline, env, inherited) {
  if (!className || !className.split(/\s+/).some(name => byName.get(name)?.some(rule => rule.when?.focus))) return undefined;
  const base = resolveStyle(className, inline, env, inherited).style;
  const active = resolveStyle(className, inline, env, inherited, true).style;
  const result = {};
  for (const [key,value] of Object.entries(active)) if (JSON.stringify(value)!==JSON.stringify(base[key])) {
    if (!['color','background','opacity','borderColor','backgroundGradient','boxShadow','placeholderColor'].includes(key)) throw Error(`UI_STYLE: focus does not support ${key}`);
    result[key]=value;
  }
  return Object.keys(result).length ? result : undefined;
}

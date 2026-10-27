import { compile } from 'tailwindcss';
import postcss from 'postcss';
import { parse, formatHex8 } from 'culori';
import { readFile } from 'node:fs/promises';
import { tokenSelectorTheme, validateTokens } from './stylesheets.mjs';
import { lowerAnimation, easing as lowerEasing } from './animation.mjs';
import { css as cssStyle, keywords, style as styleTable, limits, roles } from '../src/generated/contract.js';

// Tailwind owns utility expansion and cascade order; this module lowers each
// rule's declarations to the CSS-named style the native renderer accepts and
// reports everything else instead of quietly producing a different design.

class Unsupported extends Error {}
// A colour that is not a role; reported as UI_COLOR rather than unsupported.
class RoleError extends Unsupported {}
const roleClasses = 'use a role class such as bg-surface, text-muted, border-line or bg-accent text-on-accent';
const unsupported = message => { throw new Unsupported(message); };

// calc() arithmetic over px, rem, em, %, vw and vh; rem resolves to 16px.
function evaluate(text) {
  const source = text.trim(); let i = 0;
  const space = () => { while (/\s/.test(source[i] ?? '')) i++; };
  const expect = char => { space(); if (source[i] !== char) unsupported(`unsupported value: ${text}`); i++; };
  const primary = () => {
    space();
    if (source.startsWith('calc(', i) || source[i] === '(') {
      i += source[i] === '(' ? 1 : 5;
      const value = sum(); expect(')'); return value;
    }
    const match = source.slice(i).match(/^(-?(?:\d*\.)?\d+(?:e[+-]?\d+)?)(px|rem|em|%|[dsl]?vw|[dsl]?vh)?/i);
    if (!match) unsupported(`unsupported value: ${text}`);
    i += match[0].length;
    // The View viewport has no browser chrome, so dynamic, small and large units coincide.
    const unit = match[2]?.toLowerCase().replace(/^[dsl](v[wh])$/, '$1') ?? '';
    return unit === 'rem' ? { value: Number(match[1]) * 16, unit: 'px' } : { value: Number(match[1]), unit };
  };
  const product = () => {
    let left = primary();
    for (;;) {
      space();
      const op = source[i];
      if (op !== '*' && op !== '/') return left;
      i++;
      const right = primary();
      if (op === '*') {
        if (left.unit && right.unit) unsupported(`unsupported calculation: ${text}`);
        left = { value: left.value * right.value, unit: left.unit || right.unit };
      } else {
        if (right.unit) unsupported(`unsupported calculation: ${text}`);
        left = { value: left.value / right.value, unit: left.unit };
      }
    }
  };
  const sum = () => {
    let left = product();
    for (;;) {
      space();
      const op = source[i];
      if ((op !== '+' && op !== '-') || !/\s/.test(source[i + 1] ?? '')) return left;
      i++;
      const right = product();
      if (left.unit !== right.unit && left.value !== 0 && right.value !== 0) unsupported(`unsupported calculation: ${text}`);
      left = { value: op === '+' ? left.value + right.value : left.value - right.value, unit: left.unit || right.unit };
    }
  };
  const result = sum(); space();
  if (i !== source.length || !Number.isFinite(result.value)) unsupported(`unsupported value: ${text}`);
  return result;
}

// A length in pixels, or with `fraction` a unitless number where `%` means /100.
function number(value, length = false) {
  const { value: amount, unit } = evaluate(value);
  if (unit === 'px' || (unit === '' && (!length || amount === 0))) return amount;
  if (unit === '%' && !length) return amount / 100;
  return unsupported(`unsupported ${length ? 'length' : 'number'}: ${value}`);
}

function dimension(value) {
  if (['auto', 'fit-content', 'max-content', 'min-content'].includes(value)) return value;
  // `ch` resolves natively against the advance of "0" in the node's font.
  const ch = value.match(/^((?:\d*\.)?\d+)ch$/);
  if (ch) return `${Number(ch[1])}ch`;
  const { value: amount, unit } = evaluate(value);
  if (unit === '%' || unit === 'vw' || unit === 'vh') return `${Math.round(amount * 1e6) / 1e6}${unit}`;
  if (unit === 'px' || (unit === '' && amount === 0)) return amount;
  return unsupported(`unsupported length: ${value}`);
}

function values(value, count, lower) {
  const parts = postcss.list.space(value);
  if (!parts.length || parts.length > count) unsupported(`unsupported shorthand: ${value}`);
  const [top, right = top, bottom = top, left = right] = parts.map(lower);
  return count === 4 ? [top, right, bottom, left] : count === 2 ? [top, right] : [top];
}

function parseColor(value) {
  const role = value.match(/^role\(([a-z0-9-]+)\)$/);
  if (role) return { role: role[1] };
  const mix = value.match(/^color-mix\(in (?:srgb|oklab), (.+) ([\d.]+)%, transparent\)$/);
  const result = mix ? parseColor(mix[1]) : parse(value);
  if (!result) unsupported(`unsupported color: ${value}`);
  if (mix) result.alpha = (result.alpha ?? 1) * Number(mix[2]) / 100;
  return result;
}

// Roles lower to `role` or `role/NN`; anything else is a literal.
function color(value) {
  const parsed = parseColor(value);
  if (!parsed.role) return formatHex8(parsed);
  const percent = Math.round((parsed.alpha ?? 1) * 100);
  return percent === 100 ? parsed.role : `${parsed.role}/${percent}`;
}

// Set for one compileUtilities pass of a renderer test fixture.
let literalColors = false;

// Class colours must be roles; transparent is the one literal they keep.
function roleColor(value) {
  const lowered = color(value);
  if (!literalColors && lowered.startsWith('#') && !/^#[0-9a-f]{6}00$/.test(lowered)) throw new RoleError(`not a role; ${roleClasses}`);
  return lowered;
}

/** The Tailwind colour names for the roles: `text-muted` is `muted`, so the class reads `text-muted`. */
export const roleTheme = `@theme inline {\n${roles.map(role => `  --color-${role.replace(/^text-/, '')}: role(${role});`).join('\n')}\n}`;

function variables(value, vars, depth = 0) {
  if (depth > 32) unsupported('cyclic CSS variable');
  for (let start; (start = value.indexOf('var(')) !== -1;) {
    let end = start + 4, balance = 1;
    for (; end < value.length && balance; end++) {
      if (value[end] === '(') balance++;
      if (value[end] === ')') balance--;
    }
    if (balance) unsupported(`invalid CSS variable: ${value}`);
    const body = value.slice(start + 4, end - 1), comma = body.indexOf(',');
    const name = (comma < 0 ? body : body.slice(0, comma)).trim();
    const fallback = comma < 0 ? undefined : body.slice(comma + 1).trim();
    if (!(name in vars) && fallback === undefined) unsupported(`unresolved ${name}`);
    value = value.slice(0, start) + variables(vars[name] ?? fallback, vars, depth + 1) + value.slice(end);
  }
  return value;
}

function fontFamily(value) {
  const families = value.split(',').map(family => family.trim().replace(/^['"]|['"]$/g, '').toLowerCase());
  const stacks = [
    ['monospace', ['ui-monospace', 'sfmono-regular', 'menlo', 'monaco', 'consolas', 'liberation mono', 'courier new', 'monospace']],
    ['sans-serif', ['ui-sans-serif', 'system-ui', 'sans-serif', 'apple color emoji', 'segoe ui emoji', 'segoe ui symbol', 'noto color emoji']],
    ['sans-serif', ['-apple-system', 'blinkmacsystemfont', 'segoe ui', 'roboto', 'helvetica neue', 'noto sans', 'arial', 'sans-serif', 'apple color emoji', 'segoe ui emoji', 'segoe ui symbol', 'noto color emoji']],
  ];
  for (const [native, stack] of stacks)
    if (families.join(',') === stack.join(',') || (families.length === 1 && families[0] === native)) return native;
  return unsupported(`unsupported font-family: ${value}`);
}

function degrees(text) {
  const match = text.match(/^(-?(?:\d*\.)?\d+)(deg|turn|rad|grad)$/);
  if (!match) unsupported(`unsupported gradient angle: ${text}`);
  const value = Number(match[1]) * { deg: 1, turn: 360, rad: 180 / Math.PI, grad: 0.9 }[match[2]];
  if (Math.abs(value) > 36000) unsupported('gradient angle exceeds native bounds');
  return value;
}

// `at` positions as box fractions; keywords may come in either order, as in CSS.
function gradientCenter(text) {
  const parts = text.trim().split(/\s+/);
  const horizontal = { left: 0, right: 1 }, vertical = { top: 0, bottom: 1 };
  const fraction = part => part === 'center' ? .5 : /^-?(?:\d*\.)?\d+%$/.test(part) ? number(part)
    : unsupported(`unsupported gradient position: ${text}`);
  if (parts.length === 1) {
    const [part] = parts;
    if (part in vertical) return [.5, vertical[part]];
    return [part in horizontal ? horizontal[part] : fraction(part), .5];
  }
  if (parts.length !== 2) unsupported(`unsupported gradient position: ${text}`);
  let [x, y] = parts;
  if (x in vertical || y in horizontal) [x, y] = [y, x];
  const at = [x in horizontal ? horizontal[x] : fraction(x), y in vertical ? vertical[y] : fraction(y)];
  if (at.some(v => Math.abs(v) > 16)) unsupported('gradient position exceeds native bounds');
  return at;
}

/** Gradient geometry from a CSS prelude: `to right`, `45deg`, `circle closest-side at top`, `from 90deg`, each optionally `in oklab`. */
function gradientPosition(text) {
  const match = text.trim().match(/^([\s\S]*?)(?:\s*\bin (srgb|oklab))?$/);
  let rest = match[1].trim();
  const out = { colorSpace: match[2] ?? 'srgb' };
  const at = rest.match(/(?:^|\s)at\s+(.+)$/);
  if (at) { out.at = gradientCenter(at[1]); rest = rest.slice(0, at.index).trim(); }
  const from = rest.match(/^from\s+(\S+)$/);
  if (from) out.from = degrees(from[1]);
  else if (/^-?(?:\d*\.)?\d+(?:deg|turn|rad|grad)$/.test(rest)) out.angle = degrees(rest);
  else if (rest.startsWith('to ')) {
    const directions = { 'to top':'top', 'to right':'right', 'to bottom':'bottom', 'to left':'left',
      'to top right':'top-right', 'to right top':'top-right', 'to bottom right':'bottom-right', 'to right bottom':'bottom-right',
      'to bottom left':'bottom-left', 'to left bottom':'bottom-left', 'to top left':'top-left', 'to left top':'top-left' };
    out.direction = directions[rest.replace(/\s+/g, ' ')] ?? unsupported(`unsupported gradient direction: ${text}`);
  } else for (const token of rest.split(/\s+/).filter(Boolean)) {
    if (['circle', 'ellipse'].includes(token) && !out.shape) out.shape = token;
    else if (['closest-side', 'closest-corner', 'farthest-side', 'farthest-corner'].includes(token) && !out.size) out.size = token;
    else unsupported(`unsupported gradient shape: ${text}`);
  }
  return out;
}

const geometryKeys = { linear: ['direction', 'angle'], radial: ['shape', 'size', 'at'], conic: ['from', 'at'] };

/** The wire gradient for a kind, rejecting geometry CSS would drop the declaration for. */
function gradientValue(kind, position, stops) {
  const { colorSpace = 'srgb', ...geometry } = position ?? {};
  for (const key of Object.keys(geometry))
    if (!geometryKeys[kind].includes(key)) unsupported(`${kind} gradients do not take ${key}`);
  if (kind === 'linear' && geometry.angle === undefined && geometry.direction === undefined) geometry.direction = 'bottom';
  const [low, high] = kind === 'linear' ? [-16, 16] : kind === 'radial' ? [0, 16] : [0, 1];
  if (stops.some(stop => stop.offset < low || stop.offset > high)) unsupported(`${kind} gradient stop outside ${low * 100}%..${high * 100}%`);
  return { ...(kind === 'linear' ? {} : { kind }), ...geometry, colorSpace, stops };
}

// A complete `linear-|radial-|conic-gradient(...)` value, as authored in arbitrary values and theme tokens.
function cssGradient(value) {
  const match = value.trim().match(/^(linear|radial|conic)-gradient\(([\s\S]*)\)$/);
  if (!match) unsupported(`unsupported background-image: ${value}`);
  const kind = match[1], args = postcss.list.comma(match[2]);
  const isStop = text => Boolean(parse(postcss.list.space(text)[0] ?? '')) || /^color-mix\(/.test(text);
  const position = args.length && !isStop(args[0]) ? gradientPosition(args.shift()) : undefined;
  const offset = token => {
    if (/^-?(?:\d*\.)?\d+%$/.test(token)) return number(token);
    if (kind === 'conic' && /(?:deg|turn|rad|grad)$/.test(token)) return degrees(token) / 360;
    if (token === '0') return 0;
    return unsupported(`gradient stop positions must be percentages: ${token}`);
  };
  const stops = [];
  for (const arg of args) {
    const [tint, ...positions] = postcss.list.space(arg);
    if (!isStop(tint) || positions.length > 2) unsupported(`unsupported gradient stop: ${arg}`);
    // A complete CSS gradient is artwork, so its stops may be literal; from-/via-/to- utilities are roles.
    const hex = color(tint);
    if (!positions.length) stops.push({ color: hex });
    for (const position of positions) stops.push({ color: hex, offset: offset(position) });
  }
  if (stops.length < 2 || stops.length > 16) unsupported('gradients need 2..16 stops');
  // CSS stop fixup: ends default to 0 and 1, later stops clamp to earlier ones, gaps spread evenly.
  stops[0].offset ??= 0;
  stops[stops.length - 1].offset ??= 1;
  for (let i = 1; i < stops.length; i++) {
    if (stops[i].offset === undefined) {
      let j = i; while (stops[j].offset === undefined) j++;
      const start = stops[i - 1].offset, end = Math.max(start, stops[j].offset);
      for (let k = i; k < j; k++) stops[k].offset = start + (end - start) * (k - i + 1) / (j - i + 1);
    }
    stops[i].offset = Math.max(stops[i - 1].offset, stops[i].offset);
  }
  return gradientValue(kind, position, stops.map(({ offset, color }) => ({ offset, color })));
}

function gradientDeclaration(prop, raw, local, gradient) {
  if (prop === 'background-image') {
    const variable = raw.replace(/\s/g, '').match(/^(linear|radial|conic)-gradient\(var\(--tw-gradient-stops(?:,[^()]*)?\)\)$/);
    if (raw === 'none') gradient.enabled = false;
    else if (variable) Object.assign(gradient, { enabled: true, kind: variable[1], fixed: null });
    else Object.assign(gradient, { enabled: true, fixed: cssGradient(variables(raw, local)) });
    return;
  }
  if (prop === '--tw-gradient-stops') {
    const expression = raw.replace(/\s/g, '');
    if (expression === 'var(--tw-gradient-via-stops)') gradient.source = 'via';
    else if (expression === 'var(--tw-gradient-via-stops,var(--tw-gradient-position),var(--tw-gradient-from)var(--tw-gradient-from-position),var(--tw-gradient-to)var(--tw-gradient-to-position))') gradient.source = 'fallback';
    else unsupported(`unsupported gradient stops: ${raw}`);
    return;
  }
  if (prop === '--tw-gradient-via-stops') {
    const expression = raw.replace(/\s/g, '');
    if (expression === 'initial') gradient.via = false;
    else if (expression === 'var(--tw-gradient-position),var(--tw-gradient-from)var(--tw-gradient-from-position),var(--tw-gradient-via)var(--tw-gradient-via-position),var(--tw-gradient-to)var(--tw-gradient-to-position)') gradient.via = true;
    else unsupported(`unsupported gradient via stops: ${raw}`);
    return;
  }
  const value = variables(raw, local);
  if (prop === '--tw-gradient-position') gradient.position = gradientPosition(value);
  else {
    const match = prop.match(/^--tw-gradient-(from|via|to)(-position)?$/);
    if (!match) unsupported(`unsupported gradient property: ${prop}`);
    if (match[2]) {
      if (!/^-?(?:\d*\.)?\d+%$/.test(value)) unsupported(`unsupported gradient stop position: ${value}`);
      const offset = number(value);
      if (Math.abs(offset) > 16) unsupported('gradient stop exceeds native bounds');
      gradient[`${match[1]}Offset`] = offset;
    } else gradient[`${match[1]}Color`] = roleColor(value);
  }
}

// Tailwind's elevation layers are black at a low alpha; they take the theme's shadow ink instead.
function shadowInk(value) {
  const black = /^#000000([0-9a-f]{2})?$/i.exec(value);
  const alpha = black ? parseInt(black[1] ?? 'ff', 16) : 0;
  // A transparent layer stays transparent: it is an off state, not an ink.
  return alpha > 0 ? `shadow/${Math.max(1, Math.round(alpha / 2.55))}` : value;
}

function shadowLayers(value, local) {
  if (value === 'none') return [];
  // A variable may hold several layers, so split again after substitution.
  const layers = postcss.list.comma(value).flatMap(layer => {
    const useColor = layer.includes('var(--tw-shadow-color,');
    return postcss.list.comma(variables(layer, local)).map(part => [part, useColor]);
  }).map(([layer, useColor]) => {
    const tokens = postcss.list.space(layer);
    const lengths = [], colors = [];
    for (const token of tokens) {
      if (/^-?(?:\d*\.)?\d+(?:px|rem)?$/.test(token)) {
        if (!/px|rem/.test(token) && Number(token) !== 0) unsupported(`shadow length needs a unit: ${token}`);
        lengths.push(number(token, true));
      }
      else if (token === 'inset') unsupported('inset shadows are not supported yet');
      else colors.push(token);
    }
    if (lengths.length < 2 || lengths.length > 4 || colors.length > 1) unsupported(`unsupported box-shadow: ${layer}`);
    const [offsetX,offsetY,blur=0,spread=0] = lengths;
    if ([offsetX,offsetY,spread].some(value => Math.abs(value)>1024) || blur<0 || blur>256) unsupported('shadow exceeds native bounds');
    const tint = colors[0] ?? 'currentColor';
    return {offsetX,offsetY,blur,spread,color:tint.toLowerCase()==='currentcolor'?'currentColor':shadowInk(color(tint)),useColor};
  });
  if (layers.length > 8) unsupported('too many box shadows');
  return layers;
}

function shadowDeclaration(prop, raw, local, shadow) {
  if (prop === '--tw-shadow') shadow.layers = shadowLayers(raw, local);
  else if (prop === '--tw-shadow-color') shadow.color = roleColor(variables(raw,local));
  else if (prop === '--tw-ring-color') shadow.ringColor = roleColor(variables(raw,local));
  else if (prop === '--tw-ring-shadow') {
    const match = raw.match(/^var\(--tw-ring-inset,\)\s+0 0 0 calc\((.+?) \+ var\(--tw-ring-offset-width\)\) var\(--tw-ring-color, currentcolor\)$/i);
    if (!match) unsupported(`unsupported ring geometry: ${raw}`);
    const width = number(variables(match[1],local),true);
    if (width < 0 || width > 1024) unsupported('ring exceeds native bounds');
    shadow.ringWidth = width;
  }
  else if (prop === 'box-shadow') {
    shadow.enabled = true;
    const composite = 'var(--tw-inset-shadow),var(--tw-inset-ring-shadow),var(--tw-ring-offset-shadow),var(--tw-ring-shadow),var(--tw-shadow)';
    shadow.composite = raw.replace(/\s/g,'') === composite;
    if (!shadow.composite) {
      if (raw.includes('var(--tw-')) unsupported(`unsupported shadow composition: ${raw}`);
      shadow.fixedLayers = shadowLayers(raw, local);
    }
  } else unsupported(`unsupported shadow property: ${prop}`);
}

function backdropValue(name, raw, vars) {
  const value = variables(raw, vars).trim();
  if (!value) return null;
  const match = value.match(name === 'blur' ? /^blur\(([^()]*)\)$/ : /^saturate\(([^()]*)\)$/);
  if (!match) unsupported(`unsupported backdrop ${name}: ${value}`);
  const argument = match[1].trim();
  const n = name === 'blur' ? number(argument, true) : argument.endsWith('%') ? number(argument.slice(0,-1)) / 100 : number(argument);
  if (!Number.isFinite(n) || n < 0 || n > (name === 'blur' ? 64 : 16)) unsupported(`unsupported backdrop ${name}: ${value}`);
  return n;
}

function backdropDeclaration(prop, raw, vars, backdrop) {
  if (prop.startsWith('--tw-backdrop-')) {
    const name = {'--tw-backdrop-blur':'blur', '--tw-backdrop-saturate':'saturation'}[prop];
    if (!name) unsupported(`unsupported backdrop property: ${prop}`);
    backdrop[name] = backdropValue(name === 'blur' ? 'blur' : 'saturate', raw, vars);
  } else if (raw.trim() === 'none') backdrop.mode = 'none';
  else if (raw.includes('var(--tw-backdrop-')) {
    // Tailwind composes independently cascading filter variables in this order.
    const remainder = raw.replace(/var\(--tw-backdrop-(?:blur|brightness|contrast|grayscale|hue-rotate|invert|opacity|saturate|sepia),\)/g, '').trim();
    if (remainder) unsupported(`unsupported backdrop filter: ${raw}`);
    backdrop.mode = 'variables';
  } else {
    const value = variables(raw, vars).trim();
    const match = value.match(/^(blur\([^()]*\))?\s*(saturate\([^()]*\))?$/);
    if (!match || (!match[1] && !match[2])) unsupported(`unsupported backdrop filter: ${value}`);
    backdrop.mode = 'fixed';
    backdrop.fixed = {blur:match[1] ? backdropValue('blur',match[1],vars) : 0,
      saturation:match[2] ? backdropValue('saturate',match[2],vars) : 1};
  }
}

const supportedConditions = ['(color: color-mix(in lab, red, red))', '(background-image: linear-gradient(in lab, red, red))'];

// CSS shorthands expand to longhands in top, right, bottom, left order; the
// contract names which author style key each longhand sets.
const edges = ['top', 'right', 'bottom', 'left'];
const shorthands = {
  'border-width': edges.map(edge => `border-${edge}-width`),
  'border-inline-width': ['border-left-width', 'border-right-width'],
  'border-block-width': ['border-top-width', 'border-bottom-width'],
  inset: edges, 'inset-inline': ['left', 'right'], 'inset-block': ['top', 'bottom'],
};
for (const family of ['padding', 'margin']) {
  shorthands[family] = edges.map(edge => `${family}-${edge}`);
  shorthands[`${family}-inline`] = [`${family}-left`, `${family}-right`];
  shorthands[`${family}-block`] = [`${family}-top`, `${family}-bottom`];
}

// One CSS longhand value lowered by the author style key it sets.
function longhand(prop, value) {
  const key = cssStyle[prop];
  switch (styleTable[key].lower) {
    case 'dimension': return dimension(value);
    case 'basis': return value === 'auto' ? 'auto' : dimension(value);
    case 'margin': return value === 'auto' ? 'auto' : number(value, true);
    case 'inset': {
      const lowered = dimension(value);
      if (typeof lowered === 'string' && !lowered.endsWith('%')) unsupported(`unsupported inset: ${value}`);
      return lowered;
    }
    // The native radius is bounded and clamps to half the shorter box edge.
    case 'length': return prop.endsWith('radius') && /^calc\(infinity\s*\*\s*1px\)$/.test(value) ? 16384 : number(value, true);
    case 'font-size': return number(value, true);
    case 'number': case 'opacity': case 'font-weight': return number(value);
    case 'color': return ['borderColor', 'backgroundColor'].includes(key) && value.toLowerCase() === 'currentcolor' ? 'currentColor' : roleColor(value);
    case 'aspect-ratio': {
      if (value === 'auto') return value;
      const ratio = value.match(/^((?:\d*\.)?\d+)\s*(?:\/\s*((?:\d*\.)?\d+))?$/);
      if (!ratio || !(Number(ratio[2] ?? 1) > 0)) unsupported(`aspect-ratio: ${value}`);
      return Number(ratio[1]) / Number(ratio[2] ?? 1);
    }
    case 'family': return fontFamily(value);
    case 'z-index':
      if (!/^-?\d+$/.test(value)) unsupported(`z-index: ${value}`);
      return Number(value);
    case 'grid-template': case 'grid-auto': {
      if (/\[|subgrid|masonry|calc\(|var\(/.test(value)) unsupported(`${prop}: ${value} (named lines, subgrid and calc are not supported)`);
      // Lengths reach native in pixels; the track grammar itself is checked there.
      return value.replace(/(-?(?:\d*\.)?\d+)rem\b/g, (_, n) => `${Number(n) * 16}px`).replace(/\s+/g, ' ').trim();
    }
    case 'grid-line': case 'object-position': return value.replace(/\s+/g, ' ').trim();
    case 'grid-flow': return value.replace(/\s+/g, ' ').trim();
    case 'order': {
      if (/^calc\(-infinity\)$/.test(value)) return -10000;
      if (/^calc\(infinity\)$/.test(value)) return 10000;
      if (!/^-?\d+$/.test(value)) unsupported(`order: ${value}`);
      return Number(value);
    }
    case 'line-clamp':
      if (!/^\d+$/.test(value)) unsupported(`line clamp: ${value}`);
      return Number(value);
    case 'letter-spacing': {
      const em = value.match(/^(-?(?:\d*\.)?\d+)em$/);
      return em ? `${Number(em[1])}em` : value === 'normal' ? 0 : number(value, true);
    }
    case 'line-height': {
      const { value: amount, unit } = evaluate(value);
      if (unit === 'px') return `${amount}px`;
      if (unit === '') return amount;
      return unsupported(`line-height: ${value}`);
    }
    default:
      // `justify-self-start|end` emit the flex spellings, which mean start/end on a grid item.
      if (prop === 'justify-self' && (value === 'flex-start' || value === 'flex-end')) return value.slice(5);
      if (!keywords[key]?.includes(value)) unsupported(`${prop}: ${value}`);
      return value;
  }
}
// Properties with no native counterpart, each naming what to write instead.
const undrawn = [
  [/^animation-[a-z-]+$/, 'animation longhands (use an animate-* utility)'],
  [/^filter$/, 'filters (use opacity-*, or backdrop-blur-* behind a translucent background)'],
  [/^cursor$/, 'cursor (touch input has no pointer cursor; remove it)'],
  [/^pointer-events$/, 'pointer-events (hit testing follows handlers; omit onClick, or set disabled)'],
  [/^outline/, 'outline (use ring-* for a focus ring)'],
];

function angle(text) {
  const negated = text.match(/^calc\(([\s\S]+?)\s*\*\s*-1\)$/);
  return negated ? -angle(negated[1]) : degrees(text);
}

function translateLength(text) {
  const { value, unit } = evaluate(text);
  if (unit === '%') return `${Math.round(value * 1e6) / 1e6}%`;
  if (unit === 'px' || (unit === '' && value === 0)) return value;
  return unsupported(`unsupported translate: ${text}`);
}

// Tailwind composes translate and scale from per-axis variables that cascade independently.
function transformDeclaration(prop, raw, local, style, transform) {
  const value = variables(raw, local).trim();
  const axis = prop.match(/^--tw-(translate|scale)-(x|y)$/);
  if (axis) {
    const parts = postcss.list.space(value);
    // `translate-[x_y]` writes both axes into each variable.
    const part = parts.length === 2 ? parts[axis[2] === 'x' ? 0 : 1] : value;
    transform[`${axis[1]}${axis[2].toUpperCase()}`] = axis[1] === 'translate' ? translateLength(part) : number(part);
    return;
  }
  if (prop === 'rotate') { style.rotate = value === 'none' ? 0 : angle(value); return; }
  if (prop === 'transform-origin') {
    style.transformOrigin = gradientCenter(value.replace(/(^|\s)0(?=\s|$)/g, '$10%'));
    return;
  }
  if (prop === 'transform') {
    if (value) unsupported(`transform: ${value} (only translate, rotate and scale)`);
    return;
  }
  const kind = prop === 'translate' ? 'translate' : 'scale';
  if (raw.includes(`var(--tw-${kind}-z)`)) unsupported(`${kind}-z (3D transforms; use ${kind}-x-* or ${kind}-y-*)`);
  transform[kind] = true;
  if (raw.replace(/\s/g, '') === `var(--tw-${kind}-x)var(--tw-${kind}-y)`) return;
  const [x, y] = postcss.list.space(value);
  const lower = kind === 'translate' ? translateLength : number;
  transform[`${kind}X`] = lower(x);
  transform[`${kind}Y`] = y === undefined ? (kind === 'scale' ? transform.scaleX : 0) : lower(y);
}

// CSS transition properties onto the painted properties a transition may animate.
const transitionTargets = {
  opacity: ['opacity'], transform: ['transform'], translate: ['transform'], scale: ['transform'], rotate: ['transform'],
  color: ['fg'], fill: ['fg'], stroke: ['fg'], 'text-decoration-color': ['fg'], 'background-color': ['bg'],
  '--tw-gradient-from': ['bg'], '--tw-gradient-via': ['bg'], '--tw-gradient-to': ['bg'], 'border-color': ['border'], 'box-shadow': ['shadow'],
  all: ['opacity', 'transform', 'bg', 'fg', 'border', 'shadow'],
};
// Listed by Tailwind's defaults but without a native paint to animate; they never animate in CSS terms here either.
const transitionInert = new Set(['outline-color', 'filter', '-webkit-backdrop-filter', 'backdrop-filter', 'display', 'content-visibility', 'overlay', 'pointer-events']);
const layoutProperty = /^(?:width|height|min-|max-|padding|margin|inset|top|right|bottom|left|gap|row-gap|column-gap|flex|grid|font-size|line-height|letter-spacing|border(?:-[a-z]+)?-width|border-radius)/;

function transitionMs(value) {
  const match = value.trim().match(/^((?:\d*\.)?\d+)(ms|s)$/);
  if (!match) unsupported(`unsupported transition time: ${value}`);
  const ms = Number(match[1]) * (match[2] === 's' ? 1000 : 1);
  if (ms > limits.transitionMs) unsupported(`transition time over ${limits.transitionMs}ms: ${value}`);
  return ms;
}

function transitionDeclaration(prop, raw, local, motion) {
  const value = variables(raw, local).trim();
  if (prop === 'transition') unsupported(`transition: ${raw} (use transition-*, duration-*, delay-* and ease-* utilities)`);
  if (prop === 'transition-property') {
    const properties = new Set();
    for (const name of value === 'none' ? [] : postcss.list.comma(value)) {
      if (transitionTargets[name]) transitionTargets[name].forEach(target => properties.add(target));
      else if (layoutProperty.test(name)) unsupported(`transition of ${name} animates layout; nearest supported: transition-transform (scale or translate) or transition-opacity`);
      else if (!transitionInert.has(name)) unsupported(`transition of ${name}`);
    }
    motion.properties = [...properties];
  } else if (prop === '--tw-duration') motion.duration = transitionMs(value);
  else if (prop === 'transition-duration') {
    // `var(--tw-duration, default)` follows whichever duration-* utility applies.
    if (raw.includes('var(--tw-duration')) motion.durationDefault = transitionMs(value);
    else motion.duration = transitionMs(value);
  } else if (prop === 'transition-delay') motion.delay = transitionMs(value);
  else if (prop === '--tw-ease') motion.easing = lowerEasing(value);
  else if (prop === 'transition-timing-function') {
    if (raw.includes('var(--tw-ease')) motion.easingDefault = lowerEasing(value);
    else motion.easing = lowerEasing(value);
  } else unsupported(`${prop}: ${raw}`);
}

function declarations(nodes, vars, root) {
  nodes = nodes.flatMap(node => node.type === 'atrule' && node.name === 'supports' && supportedConditions.includes(node.params) ? node.nodes : [node]);
  const style = {}, gradient = {}, shadow = {}, backdrop = {}, transform = {}, variant = {}, motion = {}, local = { ...vars };
  let pairedLeading = false;
  for (const node of nodes) if (node.type === 'decl' && node.prop.startsWith('--')) local[node.prop] = node.value;
  const clamp = nodes.some(node => node.type === 'decl' && node.prop === '-webkit-line-clamp');
  for (const node of nodes) {
    if (node.type === 'comment') continue;
    if (node.type !== 'decl') unsupported(`nested rule ${node.toString().split('{')[0].trim()}`);
    const { prop } = node;
    if (node.important) unsupported('!important utilities');
    if (prop === 'animation') {
      style.animation = lowerAnimation(node.value, root, value => variables(value, local));
      continue;
    }
    if (/^transition(?:-property|-duration|-delay|-timing-function)?$/.test(prop) || prop === '--tw-duration' || prop === '--tw-ease') {
      transitionDeclaration(prop, node.value, local, motion); continue;
    }
    if (prop.startsWith('--tw-backdrop-') || prop === 'backdrop-filter' || prop === '-webkit-backdrop-filter') { backdropDeclaration(prop, node.value, local, backdrop); continue; }
    if (prop.startsWith('--tw-shadow') || prop.startsWith('--tw-ring-') || prop.startsWith('--tw-inset-') || prop === 'box-shadow') { shadowDeclaration(prop, node.value, local, shadow); continue; }
    if (prop.startsWith('--tw-gradient-') || prop === 'background-image') { gradientDeclaration(prop, node.value, local, gradient); continue; }
    const numeric = { '--tw-ordinal': 'ordinal', '--tw-slashed-zero': 'slashedZero', '--tw-numeric-figure': 'figure', '--tw-numeric-spacing': 'spacing', '--tw-numeric-fraction': 'fraction' }[prop];
    if (numeric) { variant[numeric] = variables(node.value, local).trim(); continue; }
    if (prop === 'font-variant-numeric') {
      const value = variables(node.value, local).trim();
      if (node.value.includes('var(--tw-')) variant.enabled = true;
      else Object.assign(variant, { normal: value === 'normal', fixed: value === 'normal' ? undefined : value });
      continue;
    }
    if (/^--tw-(?:translate|scale)-[xy]$/.test(prop) || ['translate', 'scale', 'rotate', 'transform', 'transform-origin'].includes(prop)) {
      transformDeclaration(prop, node.value, local, style, transform); continue;
    }
    // Border styles follow --tw-border-style, which only `border-solid|dashed|dotted` set.
    if (prop === '--tw-border-style') { style.borderStyle = longhand('border-style', variables(node.value, local).trim()); continue; }
    if (prop.startsWith('--')) continue;
    // Tailwind writes the prefixed twin of user-select; it carries nothing more.
    if (prop === '-webkit-user-select') continue;
    if (prop === 'user-select' && node.value.trim() === 'auto') unsupported('select-auto (text is unselectable by default; use select-text or select-none)');
    const missing = undrawn.find(([test]) => test.test(prop));
    if (missing) unsupported(missing[1]);
    const value = variables(node.value, local).trim();
    if (prop in shorthands) {
      const longhands = shorthands[prop];
      const lowered = values(value, longhands.length, part => longhand(longhands[0], part));
      longhands.forEach((name, index) => { style[cssStyle[name]] = lowered[index]; });
    }
    else if (/^border(?:-(?:top|right|bottom|left|inline|block|x|y)(?:-start|-end)?)?-style$/.test(prop) && node.value.replace(/\s/g, '') === 'var(--tw-border-style)') { /* set by --tw-border-style */ }
    else if (/^border-(?:top|right|bottom|left|inline|block|x|y)(?:-start|-end)?-style$/.test(prop)) {
      if (value !== 'solid') unsupported(`${prop}: ${value} (per-edge border styles)`);
    }
    else if (['place-items', 'place-content', 'place-self'].includes(prop)) {
      const [block, inline = block] = postcss.list.space(value);
      const [first, second] = { 'place-items': ['align-items', 'justify-items'], 'place-content': ['align-content', 'justify-content'], 'place-self': ['align-self', 'justify-self'] }[prop];
      style[cssStyle[first]] = longhand(first, block);
      style[cssStyle[second]] = longhand(second, inline);
    }
    else if (prop === 'gap') {
      const [row, column = row] = postcss.list.space(value).map(part => number(part, true));
      if (row === column) style.gap = row; else Object.assign(style, { rowGap: row, columnGap: column });
    }
    else if (prop === 'flex') {
      if (value === 'auto') Object.assign(style, { flexGrow: 1, flexShrink: 1, flexBasis: 'auto' });
      else if (value === 'none') Object.assign(style, { flexGrow: 0, flexShrink: 0, flexBasis: 'auto' });
      else if (value === 'initial' || value === '0 auto') Object.assign(style, { flexGrow: 0, flexShrink: 1, flexBasis: 'auto' });
      else if (/^(?:\d+(?:\.\d+)?|\.\d+)$/.test(value)) Object.assign(style, { flexGrow: number(value), flexShrink: 1, flexBasis: '0%' });
      else {
        const parts = value.split(/\s+/);
        if (parts.length !== 3) unsupported(`flex: ${value}`);
        Object.assign(style, { flexGrow: number(parts[0]), flexShrink: number(parts[1]), flexBasis: parts[2] === 'auto' ? 'auto' : dimension(parts[2]) });
      }
    }
    else if (clamp && ((prop === 'display' && value === '-webkit-box') || (prop === '-webkit-box-orient' && value === 'vertical'))) { /* The line clamp owns these. */ }
    else if (prop in cssStyle) {
      style[cssStyle[prop]] = longhand(prop, value);
      // `text-sm` pairs a line height with its size; a text run takes the size and leaves the leading.
      if (prop === 'line-height' && /^var\(--tw-leading,/.test(node.value.trim())) pairedLeading = true;
    }
    else unsupported(`${prop}: ${value}`);
  }
  return { style, ...(pairedLeading ? { pairedLeading } : {}), ...(Object.keys(motion).length ? { motion } : {}), ...(Object.keys(transform).length ? { transform } : {}), ...(Object.keys(variant).length ? { numeric: variant } : {}), ...(Object.keys(backdrop).length ? { backdrop } : {}), ...(Object.keys(gradient).length ? { gradient } : {}), ...(Object.keys(shadow).length ? { shadow } : {}) };
}

function conditions(rule) {
  const result = {};
  for (let parent = rule.parent; parent?.type !== 'root'; parent = parent.parent) {
    if (parent.type === 'atrule' && (parent.name === 'layer' || (parent.name === 'supports' && supportedConditions.includes(parent.params)))) continue;
    if (parent.type !== 'atrule' || parent.name !== 'media') unsupported('nested selector');
    if (parent.params === '(prefers-color-scheme: dark)') result.theme = 'dark';
    else if (parent.params === '(hover: hover)') result.hoverMedia = true;
    else if (parent.params === '(prefers-reduced-motion: reduce)') result.reducedMotion = true;
    else if (parent.params === '(prefers-reduced-motion: no-preference)') result.reducedMotion = false;
    else {
      const match = parent.params.match(/^\(width (>=|<) ([\d.]+(?:rem|px))\)$/);
      if (!match) unsupported(`media query ${parent.params}`);
      const key = match[1] === '>=' ? 'minWidth' : 'maxWidth';
      const value = number(match[2], true);
      result[key] = key === 'minWidth' ? Math.max(result[key] ?? 0, value) : Math.min(result[key] ?? Infinity, value);
    }
  }
  return result;
}

// A CSS identifier with escapes; `\\32 xl` is how `2xl:` starts.
const ident = String.raw`(?:\\[0-9a-fA-F]{1,6}\s?|\\[^0-9a-fA-F]|[^\s:.>+~[\\()])+`;

// Each relative variant names the TS-side way to say the same.
function relatives(selector) {
  if (/^:is\(\.\S+ > \*\)$/.test(selector)) return 'child variants (put the classes on each child, e.g. when mapping the children in TS)';
  if (/^:is\(\.\S+ \*\)$/.test(selector)) return 'descendant variants (put the classes on the descendants that need them)';
  if (/^:where\(:[a-z-]+\) \./.test(selector)) return 'ancestor-state variants (in-*; track the ancestor state in TS, e.g. onFocus/onBlur, and choose the classes from it)';
  if (/\.(?:group|peer)\b/.test(selector)) return 'group-*/peer-* variants (hold the shared state in TS and choose both elements\' classes from it)';
  return undefined;
}

const unescape = name => name.replace(/\\([0-9a-f]{1,6})\s?|\\(.)/gi, (_, hex, char) => hex ? String.fromCodePoint(parseInt(hex, 16)) : char);

// `.name`, `.name:state`, `.name::placeholder` and `:where(.name > :not(:last-child))`.
function selectorTarget(source) {
  // `rtl:`/`ltr:` match the node's inherited direction at runtime, like `dark:` does the theme.
  const dir = source.match(/:where\(:dir\((ltr|rtl)\), \[dir="\1"\], \[dir="\1"\] \*\)/);
  const selector = dir ? source.replace(dir[0], '') : source;
  const target = selectorParts(selector);
  return target && dir ? { ...target, direction: dir[1] } : target;
}

// Variants that test DOM state the View does not have; name the TS-side way to say the same.
function alternative(rest) {
  if (rest.startsWith(':has(')) return ' (has-[…]: compute the condition in TS and choose the class, e.g. hasError ? "border-error" : "border-input")';
  if (rest.startsWith('[data-')) return ' (data-[…]: there are no data attributes; choose classes from the state in TS)';
  if (rest.startsWith('[aria-')) return ' (aria-*: choose classes from the state in TS and set accessibility props such as role separately)';
  return '';
}

function selectorParts(selector) {
  const children = selector.match(new RegExp(`^:where\\(\\.(${ident}) > :not\\(:last-child\\)\\)$`));
  if (children) return { name: unescape(children[1]), children: true };
  const match = selector.match(new RegExp(`^\\.(${ident})(.*)$`));
  if (!match) {
    // Variants that style relatives (`*:`, `**:`, `in-*:`, `group-*:`, `peer-*:`) put the class elsewhere in the selector.
    const relative = selector.match(new RegExp(`\\.(${ident})`));
    return relative && { name: unescape(relative[1]), error: relatives(selector) ?? `selector ${selector}` };
  }
  const name = unescape(match[1]), rest = match[2];
  if (!rest) return { name };
  if (rest === '::placeholder') return { name, placeholder: true };
  // Structural pseudo-classes resolve against the node's position among its siblings.
  const structures = { ':first-child': 'first', ':last-child': 'last', ':only-child': 'only', ':nth-child(odd)': 'odd', ':nth-child(even)': 'even', ':empty': 'empty' };
  let remaining = rest, structure;
  for (const [pseudo, kind] of Object.entries(structures))
    if (remaining.startsWith(pseudo) && !structure) { structure = kind; remaining = remaining.slice(pseudo.length); }
  const state = remaining.match(/^:(focus|active|hover|disabled)$/)?.[1];
  if (!remaining || state) return { name, ...(state ? { state } : {}), ...(structure ? { structure } : {}) };
  return { name, error: relatives(selector) ?? `selector ${rest}${alternative(rest)}` };
}

// Utilities that lower to nothing drawn are refused, never compiled to a no-op.
function inert(name) {
  const base = name.slice(name.lastIndexOf(':') + 1).replace(/^-/, '');
  const families = [
    [/^(?:space|divide)-[xy]-reverse$/, 'reversed space/divide order (use gap-* with flex-row-reverse or flex-col-reverse)'],
    [/^drop-shadow/, 'drop-shadow colors (filter drop shadows are not drawn; use shadow-* with shadow-<color>)'],
    [/^text-shadow/, 'text shadows (not drawn; use a contrasting text-* color)'],
    [/^mask/, 'masks (not drawn; use rounded-* with overflow-hidden, or a bg-linear-* overlay)'],
    [/^transform/, 'transform (translate-*, rotate-* and scale-* apply without it; remove it)'],
  ];
  return families.find(([test]) => test.test(base))?.[1] ?? `${base} (it sets nothing the native renderer draws)`;
}

const paintsColor = result => Object.keys(result.style).some(key => styleTable[key]?.lower === 'color') ||
  Boolean(result.gradient) || result.shadow?.color !== undefined || result.shadow?.ringColor !== undefined;

// A warm dev compiler rebuilds one page per save; Tailwind's compile of the theme is the slow part.
const compilers = new Map();
/** Whether `theme` already has a warm Tailwind compile. */
export const tailwindWarm = theme => compilers.has(theme);

function compilerFor(theme) {
  let compiler = compilers.get(theme);
  if (!compiler) {
    compiler = readFile(new URL(import.meta.resolve('tailwindcss/theme.css')), 'utf8')
      .then(defaults => compile(`${defaults}\n${theme}\n${roleTheme}\n@tailwind utilities;`));
    compiler.catch(() => compilers.delete(theme));
    if (compilers.size >= 8) compilers.delete(compilers.keys().next().value);
    compilers.set(theme, compiler);
  }
  return compiler;
}

/** Lowered rules for every candidate Tailwind generates; unsupported ones carry an error. */
export async function compileUtilities(candidates, theme = '', { literalColors: literal = false } = {}) {
  const tokens = postcss.parse(theme);
  validateTokens(tokens);
  const compiler = await compilerFor(theme);
  const root = postcss.parse(compiler.build([...candidates]));
  const palettes = { light: Object.create(null), dark: Object.create(null) }, rules = [];
  root.walkAtRules('property', rule => {
    for (const decl of rule.nodes ?? []) if (decl.type === 'decl' && decl.prop === 'initial-value')
      for (const palette of Object.values(palettes)) palette[rule.params] = decl.value;
  });
  root.walkRules(rule => {
    const theme = tokenSelectorTheme(rule.selector);
    if (!theme) return;
    for (const name of theme === 'all' ? ['light', 'dark'] : [theme])
      for (const decl of rule.nodes) if (decl.type === 'decl' && decl.prop.startsWith('--')) palettes[name][decl.prop] = decl.value;
  });
  literalColors = literal;
  try { root.walkRules(rule => {
    const target = selectorTarget(rule.selector);
    if (!target || !candidates.has(target.name)) return;
    const { name } = target;
    try {
      if (target.error) unsupported(target.error);
      const { hoverMedia, ...media } = conditions(rule);
      if (hoverMedia && target.state !== 'hover') unsupported('(hover: hover) outside a hover variant');
      const when = { ...media, ...(target.state ? { state: target.state } : {}), ...(target.children ? { children: true } : {}),
        ...(target.direction ? { direction: target.direction } : {}), ...(target.structure ? { structure: target.structure } : {}) };
      const lower = vars => {
        const result = declarations(rule.nodes, vars, root);
        if (!Object.keys(result.style).length && Object.keys(result).length === 1) unsupported(inert(name));
        // Native drops plain animations under reduced motion; ones authored under motion-reduce:/motion-safe: say so.
        if (result.style.animation) result.style.animation.whenReduced = when.reducedMotion === undefined ? 'base' : 'keep';
        if (!target.placeholder) return result;
        if (Object.keys(result).some(key => key !== 'style') || Object.keys(result.style).some(key => key !== 'color') || !result.style.color)
          unsupported('placeholder supports only color');
        return { style: { placeholderColor: result.style.color } };
      };
      const light = when.theme === 'dark' ? undefined : lower(palettes.light);
      const dark = lower(palettes.dark);
      if (when.theme === 'dark' && !literal && paintsColor(dark)) throw new RoleError('roles follow appearance; drop dark:');
      if (when.theme || JSON.stringify(light) === JSON.stringify(dark)) rules.push({ name, when, ...dark });
      else rules.push({ name, when: { ...when, theme: 'light' }, ...light }, { name, when: { ...when, theme: 'dark' }, ...dark });
    } catch (error) {
      if (!(error instanceof Unsupported) && !String(error.message).startsWith('UI_ANIMATION')) throw error;
      rules.push({ name, error: { category: error instanceof RoleError ? 'color' : 'unsupported', message: error.message } });
    }
  }); } finally { literalColors = false; }
  return rules;
}

import { compile } from 'tailwindcss';
import postcss from 'postcss';
import { parse, formatHex8 } from 'culori';
import { readFile } from 'node:fs/promises';
import { tokenSelectorTheme, validateTokens } from './stylesheets.mjs';
import { lowerAnimation } from './animation.mjs';
import { css as cssStyle, keywords, style as styleTable } from '../src/generated/contract.js';

// Tailwind owns utility expansion and cascade order; this module lowers each
// rule's declarations to the CSS-named style the native renderer accepts and
// reports everything else instead of quietly producing a different design.

class Unsupported extends Error {}
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
    const match = source.slice(i).match(/^(-?(?:\d*\.)?\d+(?:e[+-]?\d+)?)(px|rem|em|%|vw|vh)?/i);
    if (!match) unsupported(`unsupported value: ${text}`);
    i += match[0].length;
    const unit = match[2]?.toLowerCase() ?? '';
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
  const mix = value.match(/^color-mix\(in (?:srgb|oklab), (.+) ([\d.]+)%, transparent\)$/);
  const result = mix ? parseColor(mix[1]) : parse(value);
  if (!result) unsupported(`unsupported color: ${value}`);
  if (mix) result.alpha = (result.alpha ?? 1) * Number(mix[2]) / 100;
  return result;
}

function color(value) { return formatHex8(parseColor(value)); }

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

function gradientDeclaration(prop, raw, local, gradient) {
  if (prop === 'background-image') {
    if (raw === 'none') gradient.enabled = false;
    else if (raw.replace(/\s/g, '') === 'linear-gradient(var(--tw-gradient-stops))') gradient.enabled = true;
    else unsupported(`unsupported background-image: ${raw}`);
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
  if (prop === '--tw-gradient-position') {
    const match = value.match(/^(.*?)(?: in (srgb|oklab))?$/);
    const position = match[1].trim();
    gradient.colorSpace = match[2] ?? 'srgb';
    if (/^-?(?:\d*\.)?\d+deg$/.test(position)) {
      gradient.orientation = { angle: Number(position.slice(0, -3)) };
      if (Math.abs(gradient.orientation.angle) > 36000) unsupported('gradient angle exceeds native bounds');
    } else {
      const directions = { 'to top':'top', 'to right':'right', 'to bottom':'bottom', 'to left':'left',
        'to top right':'top-right', 'to right top':'top-right', 'to bottom right':'bottom-right', 'to right bottom':'bottom-right',
        'to bottom left':'bottom-left', 'to left bottom':'bottom-left', 'to top left':'top-left', 'to left top':'top-left' };
      if (!directions[position]) unsupported(`unsupported gradient direction: ${value}`);
      gradient.orientation = { direction: directions[position] };
    }
  } else {
    const match = prop.match(/^--tw-gradient-(from|via|to)(-position)?$/);
    if (!match) unsupported(`unsupported gradient property: ${prop}`);
    if (match[2]) {
      if (!/^-?(?:\d*\.)?\d+%$/.test(value)) unsupported(`unsupported gradient stop position: ${value}`);
      const offset = number(value);
      if (Math.abs(offset) > 16) unsupported('gradient stop exceeds native bounds');
      gradient[`${match[1]}Offset`] = offset;
    } else gradient[`${match[1]}Color`] = color(value);
  }
}

function shadowLayers(value, local) {
  if (value === 'none') return [];
  const layers = postcss.list.comma(value).map(layer => {
    const useColor = layer.includes('var(--tw-shadow-color,');
    const tokens = postcss.list.space(variables(layer, local));
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
    return {offsetX,offsetY,blur,spread,color:tint.toLowerCase()==='currentcolor'?'currentColor':color(tint),useColor};
  });
  if (layers.length > 8) unsupported('too many box shadows');
  return layers;
}

function shadowDeclaration(prop, raw, local, shadow) {
  if (prop === '--tw-shadow') shadow.layers = shadowLayers(raw, local);
  else if (prop === '--tw-shadow-color') shadow.color = color(variables(raw,local));
  else if (prop === '--tw-ring-color') shadow.ringColor = color(variables(raw,local));
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
    case 'color': return key === 'borderColor' && value.toLowerCase() === 'currentcolor' ? 'currentColor' : color(value);
    case 'family': return fontFamily(value);
    case 'z-index':
      if (!/^-?\d+$/.test(value)) unsupported(`z-index: ${value}`);
      return Number(value);
    case 'columns': {
      const columns = value.match(/^repeat\((\d+),\s*minmax\(0,\s*1fr\)\)$/);
      if (!columns) unsupported(`grid-template-columns: ${value}`);
      return Number(columns[1]);
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
      if (!keywords[key]?.includes(value)) unsupported(`${prop}: ${value}`);
      return value;
  }
}
const transitions = /^(?:transition|transition-[a-z-]+|animation-[a-z-]+|transform|translate|scale|rotate|filter|cursor|pointer-events|user-select|outline.*)$/;

function declarations(nodes, vars, root, animationLowering) {
  nodes = nodes.flatMap(node => node.type === 'atrule' && node.name === 'supports' && supportedConditions.includes(node.params) ? node.nodes : [node]);
  const style = {}, gradient = {}, shadow = {}, backdrop = {}, local = { ...vars };
  for (const node of nodes) if (node.type === 'decl' && node.prop.startsWith('--')) local[node.prop] = node.value;
  const clamp = nodes.some(node => node.type === 'decl' && node.prop === '-webkit-line-clamp');
  for (const node of nodes) {
    if (node.type === 'comment') continue;
    if (node.type !== 'decl') unsupported(`nested rule ${node.toString().split('{')[0].trim()}`);
    const { prop } = node;
    if (node.important) unsupported('!important utilities');
    if (prop === 'animation') {
      if (!animationLowering) throw Error('UI_ANIMATION: native presentation scheduling is not connected');
      style.animation = lowerAnimation(node.value, root, value => variables(value, local));
      continue;
    }
    if (prop.startsWith('--tw-backdrop-') || prop === 'backdrop-filter' || prop === '-webkit-backdrop-filter') { backdropDeclaration(prop, node.value, local, backdrop); continue; }
    if (prop.startsWith('--tw-shadow') || prop.startsWith('--tw-ring-') || prop.startsWith('--tw-inset-') || prop === 'box-shadow') { shadowDeclaration(prop, node.value, local, shadow); continue; }
    if (prop.startsWith('--tw-gradient-') || prop === 'background-image') { gradientDeclaration(prop, node.value, local, gradient); continue; }
    if (prop.startsWith('--')) continue;
    if (transitions.test(prop)) unsupported(`${prop} (the native renderer has no transitions or transforms)`);
    const value = variables(node.value, local).trim();
    if (prop in shorthands) {
      const longhands = shorthands[prop];
      const lowered = values(value, longhands.length, part => longhand(longhands[0], part));
      longhands.forEach((name, index) => { style[cssStyle[name]] = lowered[index]; });
    }
    else if (/^border(?:-(?:top|right|bottom|left|inline|block|x|y))?-style$/.test(prop)) { if (value !== 'solid') unsupported(`${prop}: ${value}`); }
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
    else if (prop in cssStyle) style[cssStyle[prop]] = longhand(prop, value);
    else unsupported(`${prop}: ${value}`);
  }
  return { style, ...(Object.keys(backdrop).length ? { backdrop } : {}), ...(Object.keys(gradient).length ? { gradient } : {}), ...(Object.keys(shadow).length ? { shadow } : {}) };
}

function conditions(rule) {
  const result = {};
  for (let parent = rule.parent; parent?.type !== 'root'; parent = parent.parent) {
    if (parent.type === 'atrule' && (parent.name === 'layer' || (parent.name === 'supports' && supportedConditions.includes(parent.params)))) continue;
    if (parent.type !== 'atrule' || parent.name !== 'media') unsupported('nested selector');
    if (parent.params === '(prefers-color-scheme: dark)') result.theme = 'dark';
    else if (parent.params === '(hover: hover)') result.hoverMedia = true;
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

const unescape = name => name.replace(/\\([0-9a-f]{1,6})\s?|\\(.)/gi, (_, hex, char) => hex ? String.fromCodePoint(parseInt(hex, 16)) : char);

// `.name`, `.name:state`, `.name::placeholder` and `:where(.name > :not(:last-child))`.
function selectorTarget(selector) {
  const children = selector.match(/^:where\(\.((?:\\.|[^\s>])+) > :not\(:last-child\)\)$/);
  if (children) return { name: unescape(children[1]), children: true };
  const match = selector.match(/^\.((?:\\.|[^\s:.>+~[])+)(.*)$/);
  if (!match) return undefined;
  const name = unescape(match[1]), rest = match[2];
  if (!rest) return { name };
  if (rest === '::placeholder') return { name, placeholder: true };
  const state = rest.match(/^:(focus|active|hover|disabled)$/)?.[1];
  return state ? { name, state } : { name, error: `selector ${rest}` };
}

/** Lowered rules for every candidate Tailwind generates; unsupported ones carry an error. */
export async function compileUtilities(candidates, theme = '', { animationLowering = false } = {}) {
  const tokens = postcss.parse(theme);
  validateTokens(tokens, { animationLowering });
  const defaults = await readFile(new URL(import.meta.resolve('tailwindcss/theme.css')), 'utf8');
  const compiler = await compile(`${defaults}\n${theme}\n@tailwind utilities;`);
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
  root.walkRules(rule => {
    const target = selectorTarget(rule.selector);
    if (!target || !candidates.has(target.name)) return;
    const { name } = target;
    try {
      if (target.error) unsupported(target.error);
      const { hoverMedia, ...media } = conditions(rule);
      if (hoverMedia && target.state !== 'hover') unsupported('(hover: hover) outside a hover variant');
      const when = { ...media, ...(target.state ? { state: target.state } : {}), ...(target.children ? { children: true } : {}) };
      const lower = vars => {
        const result = declarations(rule.nodes, vars, root, animationLowering);
        if (!target.placeholder) return result;
        if (Object.keys(result).some(key => key !== 'style') || Object.keys(result.style).some(key => key !== 'color') || !result.style.color)
          unsupported('placeholder supports only color');
        return { style: { placeholderColor: result.style.color } };
      };
      const light = when.theme === 'dark' ? undefined : lower(palettes.light);
      const dark = lower(palettes.dark);
      if (when.theme || JSON.stringify(light) === JSON.stringify(dark)) rules.push({ name, when, ...dark });
      else rules.push({ name, when: { ...when, theme: 'light' }, ...light }, { name, when: { ...when, theme: 'dark' }, ...dark });
    } catch (error) {
      if (!(error instanceof Unsupported) && !String(error.message).startsWith('UI_ANIMATION')) throw error;
      rules.push({ name, error: { category: 'unsupported', message: error.message } });
    }
  });
  return rules;
}

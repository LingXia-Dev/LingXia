import { compile } from 'tailwindcss';
import postcss from 'postcss';
import { parse, formatHex8 } from 'culori';
import { readFile } from 'node:fs/promises';
import { tokenSelectorTheme, validateTokens } from './stylesheets.mjs';
import { lowerAnimation } from './animation.mjs';

const dimensions = {
  width: 'width', height: 'height', 'min-width': 'minWidth', 'max-width': 'maxWidth',
  'min-height': 'minHeight', 'max-height': 'maxHeight', gap: 'gap',
  'border-radius': 'radius', 'font-size': 'fontSize',
};
const alignment = { 'flex-start': 'start', start: 'start', center: 'center', end: 'end', 'flex-end': 'end', stretch: 'stretch', baseline: 'baseline', 'space-between': 'between' };
const paddingSides = {
  padding: ['paddingTop', 'paddingRight', 'paddingBottom', 'paddingLeft'],
  'padding-inline': ['paddingLeft', 'paddingRight'], 'padding-block': ['paddingTop', 'paddingBottom'],
  'padding-top': ['paddingTop'], 'padding-right': ['paddingRight'],
  'padding-bottom': ['paddingBottom'], 'padding-left': ['paddingLeft'],
};

const borderSides = {
  'border-width': ['borderTopWidth', 'borderRightWidth', 'borderBottomWidth', 'borderLeftWidth'],
  'border-inline-width': ['borderLeftWidth', 'borderRightWidth'],
  'border-block-width': ['borderTopWidth', 'borderBottomWidth'],
  'border-top-width': ['borderTopWidth'], 'border-right-width': ['borderRightWidth'],
  'border-bottom-width': ['borderBottomWidth'], 'border-left-width': ['borderLeftWidth'],
};

function paddingValues(value, sides) {
  const values = []; let start = 0, depth = 0;
  for (let i = 0; i <= value.length; i++) {
    if (value[i] === '(') depth++;
    if (value[i] === ')') depth--;
    if (i === value.length || (!depth && /\s/.test(value[i]))) {
      if (i > start) values.push(number(value.slice(start, i), true));
      start = i + 1;
    }
  }
  if (!values.length || values.length > sides.length || values.some(n => n < 0 || n > 16384)) throw Error(`unsupported padding: ${value}`);
  const [top, right = top, bottom = top, left = right] = values;
  const expanded = sides.length === 4 ? [top, right, bottom, left] : sides.length === 2 ? [top, right] : [top];
  return Object.fromEntries(sides.map((side, index) => [side, expanded[index]]));
}

function variables(value, vars, depth = 0) {
  if (depth > 32) throw Error('cyclic CSS variable');
  for (let start; (start = value.indexOf('var(')) !== -1;) {
    let end = start + 4, balance = 1;
    for (; end < value.length && balance; end++) {
      if (value[end] === '(') balance++;
      if (value[end] === ')') balance--;
    }
    if (balance) throw Error(`invalid CSS variable: ${value}`);
    const body = value.slice(start + 4, end - 1), comma = body.indexOf(',');
    const name = (comma < 0 ? body : body.slice(0, comma)).trim();
    const fallback = comma < 0 ? undefined : body.slice(comma + 1).trim();
    if (!(name in vars) && fallback === undefined) throw Error(`unresolved ${name}`);
    value = value.slice(0, start) + variables(vars[name] ?? fallback, vars, depth + 1) + value.slice(end);
  }
  return value;
}

function number(value, length = false) {
  value = value.trim();
  if (value.startsWith('calc(') && value.endsWith(')')) {
    const match = value.slice(5, -1).match(/^(.+?)\s+([*/])\s+(.+)$/);
    if (!match) throw Error(`unsupported calculation: ${value}`);
    const a = number(match[1], length), b = number(match[3]);
    const result = match[2] === '*' ? a * b : a / b;
    if (!Number.isFinite(result)) throw Error(`invalid calculation: ${value}`);
    return result;
  }
  const match = value.match(/^(-?(?:\d*\.)?\d+)(px|rem|%)?$/);
  if (!match || (length && match[2] === '%')) throw Error(`unsupported value: ${value}`);
  return Number(match[1]) * (match[2] === 'rem' ? 16 : match[2] === '%' ? .01 : 1);
}

function dimension(value) {
  const fraction = value.match(/^calc\(([\d.]+)\s*\/\s*([\d.]+)\s*\*\s*100%\)$/);
  if (fraction) value = `${Number(fraction[1])/Number(fraction[2])*100}%`;
  if (value.endsWith('%')) {
    const amount = number(value)*100;
    if (!Number.isFinite(amount) || amount<0 || amount>16384) throw Error(`invalid percentage: ${value}`);
    return `${amount}%`;
  }
  return number(value,true);
}

function parseColor(value) {
  const mix = value.match(/^color-mix\(in (?:srgb|oklab), (.+) ([\d.]+)%, transparent\)$/);
  const result = mix ? parseColor(mix[1]) : parse(value);
  if (!result) throw Error(`unsupported color: ${value}`);
  if (mix) result.alpha = (result.alpha ?? 1) * Number(mix[2]) / 100;
  return result;
}

function color(value) { return formatHex8(parseColor(value)); }

function fontFamily(value) {
  const families = value.split(',').map(family => family.trim().replace(/^['"]|['"]$/g, '').toLowerCase());
  const stacks = [
    ['mono', ['ui-monospace', 'sfmono-regular', 'menlo', 'monaco', 'consolas', 'liberation mono', 'courier new', 'monospace']],
    ['sans', ['ui-sans-serif', 'system-ui', 'sans-serif', 'apple color emoji', 'segoe ui emoji', 'segoe ui symbol', 'noto color emoji']],
    ['sans', ['-apple-system', 'blinkmacsystemfont', 'segoe ui', 'roboto', 'helvetica neue', 'noto sans', 'arial', 'sans-serif', 'apple color emoji', 'segoe ui emoji', 'segoe ui symbol', 'noto color emoji']],
  ];
  for (const [native, stack] of stacks)
    if (families.join(',') === stack.join(',') ||
        (families.length === 1 && families[0] === (native === 'mono' ? 'monospace' : 'sans-serif'))) return native;
  throw Error(`unsupported font-family: ${value}`);
}

function gradientDeclaration(prop, raw, local, gradient) {
  if (prop === 'background-image') {
    if (raw === 'none') gradient.enabled = false;
    else if (raw.replace(/\s/g, '') === 'linear-gradient(var(--tw-gradient-stops))') gradient.enabled = true;
    else throw Error(`unsupported background-image: ${raw}`);
    return;
  }
  if (prop === '--tw-gradient-stops') {
    const expression = raw.replace(/\s/g, '');
    if (expression === 'var(--tw-gradient-via-stops)') gradient.source = 'via';
    else if (expression === 'var(--tw-gradient-via-stops,var(--tw-gradient-position),var(--tw-gradient-from)var(--tw-gradient-from-position),var(--tw-gradient-to)var(--tw-gradient-to-position))') gradient.source = 'fallback';
    else throw Error(`unsupported gradient stops: ${raw}`);
    return;
  }
  if (prop === '--tw-gradient-via-stops') {
    const expression = raw.replace(/\s/g, '');
    if (expression === 'initial') gradient.via = false;
    else if (expression === 'var(--tw-gradient-position),var(--tw-gradient-from)var(--tw-gradient-from-position),var(--tw-gradient-via)var(--tw-gradient-via-position),var(--tw-gradient-to)var(--tw-gradient-to-position)') gradient.via = true;
    else throw Error(`unsupported gradient via stops: ${raw}`);
    return;
  }
  const value = variables(raw, local);
  if (prop === '--tw-gradient-position') {
    const match = value.match(/^(.*?)(?: in (srgb|oklab))?$/);
    const position = match[1].trim();
    gradient.colorSpace = match[2] ?? 'srgb';
    if (/^-?(?:\d*\.)?\d+deg$/.test(position)) {
      gradient.orientation = { angle: Number(position.slice(0, -3)) };
      if (Math.abs(gradient.orientation.angle) > 36000) throw Error('gradient angle exceeds native bounds');
    } else {
      const directions = { 'to top':'top', 'to right':'right', 'to bottom':'bottom', 'to left':'left',
        'to top right':'top-right', 'to right top':'top-right', 'to bottom right':'bottom-right', 'to right bottom':'bottom-right',
        'to bottom left':'bottom-left', 'to left bottom':'bottom-left', 'to top left':'top-left', 'to left top':'top-left' };
      if (!directions[position]) throw Error(`unsupported gradient direction: ${value}`);
      gradient.orientation = { direction: directions[position] };
    }
  } else {
    const match = prop.match(/^--tw-gradient-(from|via|to)(-position)?$/);
    if (!match) throw Error(`unsupported gradient property: ${prop}`);
    if (match[2]) {
      if (!/^-?(?:\d*\.)?\d+%$/.test(value)) throw Error(`unsupported gradient stop position: ${value}`);
      const offset = number(value);
      if (Math.abs(offset) > 16) throw Error('gradient stop exceeds native bounds');
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
        if (!/px|rem/.test(token) && Number(token) !== 0) throw Error(`shadow length needs a unit: ${token}`);
        lengths.push(number(token, true));
      }
      else if (token === 'inset') throw Error('inset shadows are not supported yet');
      else colors.push(token);
    }
    if (lengths.length < 2 || lengths.length > 4 || colors.length > 1) throw Error(`unsupported box-shadow: ${layer}`);
    const [offsetX,offsetY,blur=0,spread=0] = lengths;
    if ([offsetX,offsetY,spread].some(value => Math.abs(value)>1024) || blur<0 || blur>256) throw Error('shadow exceeds native bounds');
    const tint = colors[0] ?? 'currentColor';
    return {offsetX,offsetY,blur,spread,color:tint.toLowerCase()==='currentcolor'?'currentColor':color(tint),useColor};
  });
  if (layers.length > 8) throw Error('too many box shadows');
  return layers;
}

function shadowDeclaration(prop, raw, local, shadow) {
  if (prop === '--tw-shadow') shadow.layers = shadowLayers(raw, local);
  else if (prop === '--tw-shadow-color') shadow.color = color(variables(raw,local));
  else if (prop === '--tw-ring-color') shadow.ringColor = color(variables(raw,local));
  else if (prop === '--tw-ring-shadow') {
    const match = raw.match(/^var\(--tw-ring-inset,\)\s+0 0 0 calc\((.+?) \+ var\(--tw-ring-offset-width\)\) var\(--tw-ring-color, currentcolor\)$/i);
    if (!match) throw Error(`unsupported ring geometry: ${raw}`);
    const width = number(variables(match[1],local),true);
    if (width < 0 || width > 1024) throw Error('ring exceeds native bounds');
    shadow.ringWidth = width;
  }
  else if (prop === 'box-shadow') {
    shadow.enabled = true;
    const composite = 'var(--tw-inset-shadow),var(--tw-inset-ring-shadow),var(--tw-ring-offset-shadow),var(--tw-ring-shadow),var(--tw-shadow)';
    shadow.composite = raw.replace(/\s/g,'') === composite;
    if (!shadow.composite) {
      if (raw.includes('var(--tw-')) throw Error(`unsupported shadow composition: ${raw}`);
      shadow.fixedLayers = shadowLayers(raw, local);
    }
  } else throw Error(`unsupported shadow property: ${prop}`);
}

function backdropValue(name, raw, vars) {
  const value = variables(raw, vars).trim();
  if (!value) return null;
  const match = value.match(name === 'blur' ? /^blur\(([^()]*)\)$/ : /^saturate\(([^()]*)\)$/);
  if (!match) throw Error(`unsupported backdrop ${name}: ${value}`);
  const argument = match[1].trim();
  const n = name === 'blur' ? number(argument, true) : argument.endsWith('%') ? number(argument.slice(0,-1)) / 100 : number(argument);
  if (!Number.isFinite(n) || n < 0 || n > (name === 'blur' ? 64 : 16)) throw Error(`unsupported backdrop ${name}: ${value}`);
  return n;
}

function backdropDeclaration(prop, raw, vars, backdrop) {
  if (prop.startsWith('--tw-backdrop-')) {
    const name = {'--tw-backdrop-blur':'blur', '--tw-backdrop-saturate':'saturation'}[prop];
    if (!name) throw Error(`unsupported backdrop property: ${prop}`);
    backdrop[name] = backdropValue(name === 'blur' ? 'blur' : 'saturate', raw, vars);
  } else if (raw.trim() === 'none') backdrop.mode = 'none';
  else if (raw.includes('var(--tw-backdrop-')) {
    // Tailwind composes independently cascading filter variables in this order.
    const remainder = raw.replace(/var\(--tw-backdrop-(?:blur|brightness|contrast|grayscale|hue-rotate|invert|opacity|saturate|sepia),\)/g, '').trim();
    if (remainder) throw Error(`unsupported backdrop filter: ${raw}`);
    backdrop.mode = 'variables';
  } else {
    const value = variables(raw, vars).trim();
    const match = value.match(/^(blur\([^()]*\))?\s*(saturate\([^()]*\))?$/);
    if (!match || (!match[1] && !match[2])) throw Error(`unsupported backdrop filter: ${value}`);
    backdrop.mode = 'fixed';
    backdrop.fixed = {blur:match[1] ? backdropValue('blur',match[1],vars) : 0,
      saturation:match[2] ? backdropValue('saturate',match[2],vars) : 1};
  }
}

const supportedConditions = ['(color: color-mix(in lab, red, red))', '(background-image: linear-gradient(in lab, red, red))'];

function declarations(nodes, vars, root, animationLowering) {
  nodes = nodes.flatMap(node => node.type === 'atrule' && node.name === 'supports' && supportedConditions.includes(node.params) ? node.nodes : [node]);
  const style = {}, gradient = {}, shadow = {}, backdrop = {}, local = { ...vars };
  for (const node of nodes) if (node.type === 'decl' && node.prop.startsWith('--')) local[node.prop] = node.value;
  for (const node of nodes) {
    if (node.type === 'comment') continue;
    if (node.type !== 'decl') throw Error(`unsupported selector or condition: ${node.toString()}`);
    const { prop } = node;
    if (node.important) throw Error('important utilities are not supported yet');
    if (prop === 'animation') {
      if (!animationLowering) throw Error('UI_ANIMATION: native presentation scheduling is not connected');
      style.animation = lowerAnimation(node.value, root, value => variables(value, local));
      continue;
    }
    if (prop.startsWith('--tw-backdrop-') || prop === 'backdrop-filter' || prop === '-webkit-backdrop-filter') {
      backdropDeclaration(prop, node.value, local, backdrop);
      continue;
    }
    if (prop.startsWith('--tw-shadow') || prop.startsWith('--tw-ring-') || prop.startsWith('--tw-inset-') || prop === 'box-shadow') {
      shadowDeclaration(prop, node.value, local, shadow);
      continue;
    }
    if (prop.startsWith('--tw-gradient-') || prop === 'background-image') {
      gradientDeclaration(prop, node.value, local, gradient);
      continue;
    }
    if (prop.startsWith('--')) continue;
    const value = variables(node.value, local);
    if (prop in paddingSides) Object.assign(style, paddingValues(value, paddingSides[prop]));
    else if (prop in borderSides) Object.assign(style, paddingValues(value, borderSides[prop]));
    else if (/^border(?:-(?:top|right|bottom|left|inline|block))?-style$/.test(prop) && value === 'solid') { /* Native borders are solid. */ }
    else if (prop === 'border-color') style.borderColor = value.toLowerCase() === 'currentcolor' ? 'currentColor' : color(value);
    // The native radius is bounded and clamps to half the shorter box edge.
    else if (/^border-(?:(?:top|bottom)-(?:left|right)-)?radius$/.test(prop)) {
      const radius = /^calc\(infinity\s*\*\s*1px\)$/.test(value) ? 16384 : number(value, true);
      const corners = {'border-top-left-radius':'radiusTopLeft','border-top-right-radius':'radiusTopRight','border-bottom-right-radius':'radiusBottomRight','border-bottom-left-radius':'radiusBottomLeft'};
      if (prop === 'border-radius') { style.radius=radius; for(const name of Object.values(corners)) style[name]=radius; }
      else style[corners[prop]]=radius;
    }
    else if (prop === 'flex-basis') style.flexBasis = value === 'auto' ? 'auto' : dimension(value);
    else if (prop === 'flex') {
      if (value === 'auto') Object.assign(style,{grow:1,shrink:1,flexBasis:'auto'});
      else if (value === 'none') Object.assign(style,{grow:0,shrink:0,flexBasis:'auto'});
      else if (value === 'initial' || value === '0 auto') Object.assign(style,{grow:0,shrink:1,flexBasis:'auto'});
      else if (/^(?:\d+(?:\.\d+)?|\.\d+)$/.test(value)) Object.assign(style,{grow:number(value),shrink:1,flexBasis:'0%'});
      else {
        const parts=value.split(/\s+/);
        if(parts.length!==3) throw Error(`unsupported flex: ${value}`);
        Object.assign(style,{grow:number(parts[0]),shrink:number(parts[1]),flexBasis:parts[2]==='auto'?'auto':dimension(parts[2])});
      }
    }
    else if (prop === 'height' && value === 'auto') style.height = 'auto';
    else if (prop in dimensions) style[dimensions[prop]] = /^(?:min-|max-)?(?:width|height)$/.test(prop) ? dimension(value) : number(value, true);
    else if (prop === 'font-style' && ['normal', 'italic'].includes(value)) style.fontStyle = value;
    else if (prop === 'font-family') style.fontFamily = fontFamily(value);
    else if (prop === 'color' || prop === 'background-color') style[prop === 'color' ? 'color' : 'background'] = color(value);
    else if (prop === 'opacity' || prop === 'font-weight' || prop === 'flex-grow' || prop === 'flex-shrink')
      style[{ opacity: 'opacity', 'font-weight': 'fontWeight', 'flex-grow': 'grow', 'flex-shrink': 'shrink' }[prop]] = number(value);
    else if (prop === 'overflow-x' && value === 'auto') style.scrollAxis = 'horizontal';
    else if (prop === 'overflow-wrap' && ['normal', 'break-word', 'anywhere'].includes(value)) style.overflowWrap = value;
    else if (prop === 'flex-wrap' && ['nowrap', 'wrap', 'wrap-reverse'].includes(value)) style.flexWrap = value;
    else if (prop === 'flex-direction' && ['row', 'column', 'row-reverse', 'column-reverse'].includes(value)) style.flexDirection = value;
    else if (prop === 'display' && value === 'flex') style.display = value;
    else if (prop === 'word-break' && ['normal', 'break-all', 'keep-all'].includes(value)) style.wordBreak = value;
    else if (prop === 'text-decoration-line' && ['none', 'line-through'].includes(value)) style.textDecorationLine = value;
    else if (prop === 'text-align' && ['start', 'end', 'left', 'center', 'right', 'justify'].includes(value)) style.textAlign = value;
    else if (prop === 'letter-spacing') {
      const em = value.match(/^(-?(?:\d*\.)?\d+)em$/);
      style.letterSpacing = em ? { em: Number(em[1]) } : value === 'normal' ? 0 : number(value, true);
    }
    else if (prop === 'line-height') {
      const n = number(value, true);
      if (/px|rem/.test(value)) style.lineHeight = n;
      else style.lineHeight = { em: n };
    } else if (['align-items', 'align-self', 'justify-content'].includes(prop) && value in alignment) {
      const key = { 'align-items': 'align', 'align-self': 'alignSelf', 'justify-content': 'justify' }[prop];
      if ((key === 'justify' && ['stretch', 'baseline'].includes(value)) || (key !== 'justify' && value === 'space-between')) throw Error(`unsupported ${prop}: ${value}`);
      style[key] = alignment[value];
    } else throw Error(`renderer does not support ${prop}: ${value}`);
  }
  return { style, ...(Object.keys(backdrop).length ? {backdrop} : {}), ...(Object.keys(gradient).length ? {gradient} : {}), ...(Object.keys(shadow).length ? {shadow} : {}) };
}

function conditions(rule) {
  const result = {};
  for (let parent = rule.parent; parent?.type !== 'root'; parent = parent.parent) {
    if (parent.type === 'atrule' && (parent.name === 'layer' || (parent.name === 'supports' && supportedConditions.includes(parent.params)))) continue;
    if (parent.type !== 'atrule' || parent.name !== 'media') throw Error('unsupported nested selector');
    if (parent.params === '(prefers-color-scheme: dark)') result.theme = 'dark';
    else {
      const match = parent.params.match(/^\(width (>=|<) ([\d.]+(?:rem|px))\)$/);
      if (!match) throw Error(`unsupported media query: ${parent.params}`);
      const key = match[1] === '>=' ? 'minWidth' : 'maxWidth';
      const value = number(match[2], true);
      result[key] = key === 'minWidth' ? Math.max(result[key] ?? 0, value) : Math.min(result[key] ?? Infinity, value);
    }
  }
  return result;
}

// Tailwind owns utility expansion and cascade order. Native lowering rejects
// unsupported declarations instead of quietly producing a different design.
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
    if (!rule.selector.startsWith('.')) return;
    const placeholder = rule.selector.endsWith('::placeholder');
    let selector = placeholder ? rule.selector.slice(0, -'::placeholder'.length) : rule.selector;
    const focus = selector.endsWith(':focus');
    if (focus) selector = selector.slice(0, -':focus'.length);
    const name = selector.slice(1).replace(/\\([0-9a-f]{1,6})\s?|\\(.)/gi, (_, hex, char) => hex ? String.fromCodePoint(parseInt(hex, 16)) : char);
    if (!candidates.has(name)) return;
    try {
      const when = {...conditions(rule), ...(focus ? {focus:true} : {})};
      const lower = vars => {
        const result = declarations(rule.nodes, vars, root, animationLowering);
        if (!placeholder) {
          if (focus && (result.backdrop || Object.keys(result.style).some(key => !['color','background','opacity','borderColor'].includes(key)))) throw Error('focus supports paint properties only');
          return result;
        }
        if (Object.keys(result).some(key => key !== 'style') || Object.keys(result.style).some(key => key !== 'color') || !result.style.color)
          throw Error('placeholder supports only color');
        return {style:{placeholderColor:result.style.color}};
      };
      const light = when.theme === 'dark' ? undefined : lower(palettes.light);
      const dark = lower(palettes.dark);
      if (when.theme || JSON.stringify(light) === JSON.stringify(dark)) rules.push({ name, when, ...dark });
      else rules.push({ name, when: { ...when, theme: 'light' }, ...light }, { name, when: { ...when, theme: 'dark' }, ...dark });
    }
    catch (error) { rules.push({ name, error: error.message }); }
  });
  return rules;
}

// Build-time lowering of keyframes and transitions into bounded payloads native revalidates.
const fail = message => { throw Error(`UI_ANIMATION: ${message}`); };
const numeric = (value, low, high) => {
  if (!/^-?(?:\d+(?:\.\d*)?|\.\d+)$/.test(value)) fail(`invalid number ${value}`);
  const n = Number(value);
  if (!Number.isFinite(n) || n < low || n > high) fail(`number outside bounds ${value}`);
  return n;
};
function words(value) {
  const result = []; let depth = 0, start = 0;
  for (let i = 0; i <= value.length; i++) {
    if (value[i] === '(') depth++;
    if (value[i] === ')' && --depth < 0) fail('unbalanced parentheses');
    if (!depth && value[i] === ',') fail('multiple animations are unsupported');
    if (i === value.length || (!depth && /\s/.test(value[i]))) {
      if (i > start) result.push(value.slice(start, i));
      start = i + 1;
    }
  }
  if (depth) fail('unbalanced parentheses');
  return result;
}
/** A transition's timing: an easing, or `spring(bounce)` (bounce 0..0.9), which keyframes refuse. */
export function transitionEasing(value) {
  const spring = /^spring\(\s*([^()]*)\s*\)$/.exec(value);
  if (!spring) return easing(value);
  const bounce = numeric(spring[1], 0, 0.9);
  return { spring: bounce };
}
export function easing(value) {
  if (/^spring\(/.test(value)) fail('spring timing applies to transitions, not keyframes');
  const named = {linear:'linear', 'step-end':'step-end', ease:[.25,.1,.25,1],
    'ease-in':[.42,0,1,1], 'ease-out':[0,0,.58,1], 'ease-in-out':[.42,0,.58,1]};
  if (Object.hasOwn(named,value)) return named[value];
  const match = /^cubic-bezier\(([^()]*)\)$/.exec(value);
  if (!match) fail(`unsupported easing ${value}`);
  const points = match[1].split(',').map(v => numeric(v.trim(), -16, 16));
  if (points.length !== 4 || points[0] < 0 || points[0] > 1 || points[2] < 0 || points[2] > 1) fail('invalid cubic-bezier');
  return points;
}
function shorthand(value) {
  if (value.trim() === 'none') return null;
  const result = {durationMs:0,delayMs:0,iterations:1,easing:easing('ease')};
  let name, times = 0, hasEasing = false, hasIterations = false;
  for (const part of words(value.trim())) {
    const time = /^(\d*(?:\.\d+)?|\d+)(ms|s)$/.exec(part);
    if (time) {
      if (++times > 2) fail('too many time values');
      result[times === 1 ? 'durationMs' : 'delayMs'] = numeric(time[1],0,86400000) * (time[2] === 's' ? 1000 : 1);
    } else if (/^(linear|step-end|ease(?:-in(?:-out)?|-out)?)$/.test(part) || part.startsWith('cubic-bezier(')) {
      if (hasEasing) fail('duplicate easing');
      result.easing = easing(part); hasEasing = true;
    } else if (part === 'infinite' || /^\d+(?:\.\d+)?$/.test(part)) {
      if (hasIterations) fail('duplicate iteration count');
      result.iterations = part === 'infinite' ? null : numeric(part,1,4294967295);
      if (result.iterations !== null && !Number.isInteger(result.iterations)) fail('fractional iterations unsupported');
      hasIterations = true;
    } else if (/^(forwards|backwards|both)$/.test(part)) {
      if (result.fill) fail('duplicate fill mode');
      result.fill = part;
    } else {
      if (!/^[-_a-zA-Z][-_a-zA-Z0-9]*$/.test(part) || /^(none|normal|reverse|alternate|alternate-reverse|forwards|backwards|both|running|paused|initial|inherit|unset|revert)$/.test(part)) fail(`unsupported animation token ${part}`);
      if (name) fail('multiple names or unsupported animation mode');
      name = part;
    }
  }
  if (!name || name.length > 128 || result.durationMs < 1 || result.durationMs > 86400000 || result.delayMs > 86400000) fail('invalid name or timing');
  return {name, ...result};
}
// Pixels, or a fraction of the node's own box when `relative`.
function length(value) {
  if (/^[-+]?0(?:\.0*)?$/.test(value)) return { value: 0, relative: false };
  const match = /^(-?(?:\d+(?:\.\d*)?|\.\d+))(px|rem|%)$/.exec(value);
  if (!match) fail(`unsupported translation ${value}`);
  if (match[2] === '%') return { value: numeric(match[1], -1000, 1000) / 100, relative: true };
  const result = numeric(match[1], -16384,16384) * (match[2] === 'rem' ? 16 : 1);
  if (Math.abs(result) > 16384) fail('translation outside bounds');
  return { value: result, relative: false };
}

// `background-position` percentages (or 0 and keywords) as fractions, for a gradient sweep.
function backgroundPosition(value) {
  const keyword = { left: 0, top: 0, center: .5, right: 1, bottom: 1 };
  const parts = value.trim().split(/\s+/);
  if (!parts.length || parts.length > 2) fail(`unsupported background-position ${value}`);
  const fraction = token => token in keyword ? keyword[token] : /^[-+]?0(?:\.0*)?$/.test(token) ? 0
    : /^-?(?:\d+(?:\.\d*)?|\.\d+)%$/.test(token) ? numeric(token.slice(0,-1),-1600,1600)/100 : fail(`background-position takes percentages: ${token}`);
  return [fraction(parts[0]), fraction(parts[1] ?? 'center')];
}

function angle(value) {
  const match = /^(-?(?:\d+(?:\.\d*)?|\.\d+))(deg|turn|rad)$/.exec(value);
  if (!match) fail(`unsupported rotation ${value}`);
  return numeric(match[1], -36000, 36000) * { deg: 1, turn: 360, rad: 180 / Math.PI }[match[2]];
}
function transforms(value) {
  if (value === 'none') return null;
  const result = []; let end = 0;
  for (const match of value.matchAll(/([a-zA-Z]+)\(([^()]*)\)/g)) {
    if (value.slice(end,match.index).trim()) fail('unsupported transform expression');
    end = match.index + match[0].length;
    const [name, body] = [match[1],match[2]];
    const args = body.trim().split(/\s*,\s*|\s+/);
    if (name === 'rotate' || name === 'rotateZ') {
      if (args.length !== 1) fail(`unsupported transform ${name}`);
      result.push({kind:'rotate',angle:angle(args[0])});
      continue;
    }
    const scale = /^scale(?:X|Y)?$/.test(name), translate = /^translate(?:X|Y)?$/.test(name);
    if ((!scale && !translate) || args.length > (/[XY]$/.test(name) ? 1 : 2)) fail(`unsupported transform ${name}`);
    const lengths = translate ? args.map(length) : [];
    if (lengths.length === 2 && lengths[0].relative !== lengths[1].relative) fail('mixed percent and length translation');
    const values = scale ? args.map(v => numeric(v,Number.MIN_VALUE,16)) : lengths.map(l => l.value);
    const identity = scale ? 1 : 0;
    const x = name.endsWith('Y') ? identity : values[0];
    const y = name.endsWith('X') ? identity : values[1] ?? (scale || name.endsWith('Y') ? values[0] : 0);
    result.push({kind:scale?'scale':'translate',x,y,...(lengths[0]?.relative ? {relative:true} : {})});
  }
  if (!result.length || result.length > 8 || value.slice(end).trim()) fail('invalid transform list');
  return result;
}

export function lowerAnimation(value, root, resolve = value => value) {
  const timing = shorthand(resolve(value));
  if (!timing) return null;
  let definition;
  root.walkAtRules('keyframes', rule => {
    if (rule.params === timing.name) {
      if (rule.parent.type !== 'root') fail('conditional keyframes unsupported');
      definition = rule;
    }
  });
  if (!definition) fail(`missing keyframes ${timing.name}`);
  const frames = new Map();
  for (const rule of definition.nodes ?? []) {
    if (rule.type === 'comment') continue;
    if (rule.type !== 'rule') fail('invalid keyframe rule');
    const properties = {};
    for (const decl of rule.nodes ?? []) {
      if (decl.type === 'comment') continue;
      if (decl.type !== 'decl' || decl.important) fail('invalid keyframe declaration');
      const value = resolve(decl.value);
      if (decl.prop === 'opacity') properties.opacity = numeric(value,0,1);
      else if (decl.prop === 'transform') properties.transform = transforms(value);
      else if (decl.prop === 'animation-timing-function') properties.easing = easing(value);
      else if (decl.prop === 'background-position') properties.backgroundPosition = backgroundPosition(value);
      else fail(`unsupported animated property ${decl.prop}`);
    }
    for (const selector of rule.selector.split(',').map(v=>v.trim())) {
      const offset = selector === 'from' ? 0 : selector === 'to' ? 1 :
        selector.endsWith('%') ? numeric(selector.slice(0,-1),0,100)/100 : fail('invalid keyframe offset');
      frames.set(offset,{...frames.get(offset),...properties});
      if (frames.size > 64) fail('keyframe budget exceeded');
    }
  }
  // A missing `from`/`to` is the identity (opacity 1, no transform), as for an unstyled box.
  for (const property of ['opacity','transform']) {
    const keyed = [...frames].filter(([,frame])=>Object.hasOwn(frame,property));
    if (!keyed.length) continue;
    for (const end of [0,1]) if (!keyed.some(([offset])=>offset===end))
      frames.set(end,{...frames.get(end),[property]:property === 'opacity' ? 1 : null});
  }
  const sorted = [...frames].sort(([a],[b])=>a-b);
  // A keyframe's timing function eases the segment that starts there.
  const ease = offset => frames.get(offset)?.easing;
  const key = (offset, value) => ({offset, value, ...(ease(offset) && offset < 1 ? {easing: ease(offset)} : {})});
  const track = property => sorted.filter(([,frame])=>Object.hasOwn(frame,property)).map(([offset,frame])=>key(offset,frame[property]));
  const opacity = track('opacity'), transform = track('transform'), sweep = track('backgroundPosition');
  if (sweep.length && (sweep[0].offset !== 0 || sweep[sweep.length - 1].offset !== 1)) fail('background-position keyframes need from and to');
  const pattern = transform.find(key=>key.value !== null)?.value ?? [];
  const transformTracks = pattern.map(op => op.kind === 'rotate' ? {kind:'rotate',angle:[]} : {kind:op.kind,x:[],y:[],...(op.relative ? {relative:true} : {})});
  for (const {offset,value,easing} of transform) {
    const ops = value ?? pattern.map(({kind})=>kind === 'rotate' ? {kind,angle:0} : {kind,x:kind==='scale'?1:0,y:kind==='scale'?1:0,relative:pattern.find(op=>op.kind===kind)?.relative});
    if (ops.length !== pattern.length || ops.some((op,i)=>op.kind !== pattern[i].kind || (op.kind === 'translate' && value && !!op.relative !== !!pattern[i].relative)))
      fail('transform lists must preserve operation order');
    const extra = easing ? {easing} : {};
    ops.forEach((op,i)=>{
      if (op.kind === 'rotate') transformTracks[i].angle.push({offset,value:op.angle,...extra});
      else for(const axis of ['x','y'])transformTracks[i][axis].push({offset,value:op[axis],...extra});
    });
  }
  const result = {...timing};
  if (!opacity.length && !transformTracks.length && !sweep.length) fail('empty animation');
  Object.assign(result,{opacity:opacity.length?opacity:null,transforms:transformTracks});
  if (sweep.length) result.backgroundPosition = {
    x: sweep.map(({value:[x],...key})=>({...key,value:x})), y: sweep.map(({value:[,y],...key})=>({...key,value:y})),
  };
  if (JSON.stringify(result).length > 32768) fail('effect budget exceeded');
  return result;
}

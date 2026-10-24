// Build-time lowering only. Authoring stays gated until native scheduling is wired.
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
function easing(value) {
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
    } else {
      if (!/^[-_a-zA-Z][-_a-zA-Z0-9]*$/.test(part) || /^(none|normal|reverse|alternate|alternate-reverse|forwards|backwards|both|running|paused|initial|inherit|unset|revert)$/.test(part)) fail(`unsupported animation token ${part}`);
      if (name) fail('multiple names or unsupported animation mode');
      name = part;
    }
  }
  if (!name || name.length > 128 || result.durationMs < 1 || result.durationMs > 86400000 || result.delayMs > 86400000) fail('invalid name or timing');
  return {name, ...result};
}
function length(value) {
  if (/^[-+]?0(?:\.0*)?$/.test(value)) return 0;
  const match = /^(-?(?:\d+(?:\.\d*)?|\.\d+))(px|rem)$/.exec(value);
  if (!match) fail(`unsupported translation ${value}`);
  const result = numeric(match[1], -16384,16384) * (match[2] === 'rem' ? 16 : 1);
  if (Math.abs(result) > 16384) fail('translation outside bounds');
  return result;
}
function transforms(value) {
  if (value === 'none') return null;
  const result = []; let end = 0;
  for (const match of value.matchAll(/([a-zA-Z]+)\(([^()]*)\)/g)) {
    if (value.slice(end,match.index).trim()) fail('unsupported transform expression');
    end = match.index + match[0].length;
    const [name, body] = [match[1],match[2]];
    const args = body.trim().split(/\s*,\s*|\s+/);
    const scale = /^scale(?:X|Y)?$/.test(name), translate = /^translate(?:X|Y)?$/.test(name);
    if ((!scale && !translate) || args.length > (/[XY]$/.test(name) ? 1 : 2)) fail(`unsupported transform ${name}`);
    const values = args.map(v => scale ? numeric(v,Number.MIN_VALUE,16) : length(v));
    const identity = scale ? 1 : 0;
    const x = name.endsWith('Y') ? identity : values[0];
    const y = name.endsWith('X') ? identity : values[1] ?? (scale || name.endsWith('Y') ? values[0] : 0);
    result.push({kind:scale?'scale':'translate',x,y});
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
      else fail(`unsupported animated property ${decl.prop}`);
    }
    for (const selector of rule.selector.split(',').map(v=>v.trim())) {
      const offset = selector === 'from' ? 0 : selector === 'to' ? 1 :
        selector.endsWith('%') ? numeric(selector.slice(0,-1),0,100)/100 : fail('invalid keyframe offset');
      frames.set(offset,{...frames.get(offset),...properties});
      if (frames.size > 64) fail('keyframe budget exceeded');
    }
  }
  const sorted = [...frames].sort(([a],[b])=>a-b);
  const track = property => {
    const keys = sorted.filter(([,frame])=>Object.hasOwn(frame,property)).map(([offset,frame])=>({offset,value:frame[property]}));
    if (keys.length && (keys.length < 2 || keys[0].offset !== 0 || keys.at(-1).offset !== 1)) fail(`explicit endpoints required for ${property}`);
    return keys;
  };
  const opacity = track('opacity'), transform = track('transform');
  const pattern = transform.find(key=>key.value !== null)?.value ?? [];
  const transformTracks = pattern.map(({kind},index)=>({kind,x:[],y:[]}));
  for (const {offset,value} of transform) {
    const ops = value ?? pattern.map(({kind})=>({kind,x:kind==='scale'?1:0,y:kind==='scale'?1:0}));
    if (ops.length !== pattern.length || ops.some((op,i)=>op.kind !== pattern[i].kind)) fail('transform lists must preserve operation order');
    ops.forEach((op,i)=>{for(const axis of ['x','y'])transformTracks[i][axis].push({offset,value:op[axis]});});
  }
  const result = {...timing};
  if (!opacity.length && !transformTracks.length) fail('empty animation');
  Object.assign(result,{opacity:opacity.length?opacity:null,transforms:transformTracks});
  if (JSON.stringify(result).length > 32768) fail('effect budget exceeded');
  return result;
}

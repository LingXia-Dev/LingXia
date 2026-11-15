import { pressChange } from './press.js';
import { elements, admission, limits, commonProps } from '../generated/contract.js';
import { resolveClasses, classGeneration } from '../style/classes.js';
import { classElementProblem, gridProblem, gridRefuses, tagHint } from '../style/hints.js';
import { lowerStyle, paintOnly, mergeStyle } from '../style/lower.js';
import { h, readNode, isNode, isFragment, entries } from './nodes.js';
import { render, page } from './state.js';
import { cellOf, envOf } from './hooks.js';
import { located } from './errors.js';
import { pageFamily } from './fonts.js';
import { has, validInputText } from './data.js';
import { fieldOptions, field as fieldIds, ignorableFields } from '../generated/wire.js';
import * as check from './validate.js';
import { defaultSeries } from '../style/color.js';
import { keyPositions } from './keys.js';
import { expand, invokerHandler, interestTrigger, popoverScope, dragClose, scrimMark } from './toplayer.js';
import { recall, enter, leave, remember } from './memo.js';
import { isProvider, provide } from './context.js';
import { playerName, videoStatus, fullscreenHandler } from './video.js';
import { boxBinding } from './box.js';
import { shownInDialog } from './fullscreen.js';
import { fieldText } from './calendar.js';

const videoFlags = [['videoMuted', 'muted'], ['videoLoop', 'loop']];
const videoEvents = [['loadedmetadata', 'onLoadedMetadata'], ['play', 'onPlay'], ['playing', 'onPlaying'], ['pause', 'onPause'],
  ['waiting', 'onWaiting'], ['seeked', 'onSeeked'], ['timeupdate', 'onTimeUpdate'], ['ended', 'onEnded'], ['error', 'onError'],
  ['ratechange', 'onRateChange'], ['volumechange', 'onVolumeChange'], ['qualitychange', 'onQualityChange']];

const common = new Set(commonProps);
const semanticStates = { pressed: 'statePressed', selected: 'stateSelected', expanded: 'stateExpanded', busy: 'stateBusy', invalid: 'stateInvalid', required: 'stateRequired', checked: 'stateChecked' };
const accepts = (field, tag) => admission.fields[field].includes(elements[tag].wire);
// An older host would refuse a field it lacks; an accessibility-only one is left out instead.
const ignorable = new Set(ignorableFields);
const warned = new Set();
function dropped(key) {
  const host = globalThis.__lingxiaViewHost;
  if (!ignorable.has(key) || !host || fieldIds[key] <= host.maxField) return false;
  if (!warned.has(key) && typeof globalThis.__lingxiaWarn === 'function') {
    warned.add(key);
    globalThis.__lingxiaWarn(`UI_COMPAT: the native host predates ${key}; it is left out of this page's semantics`);
  }
  return true;
}
// Elements that take input of their own; they may sit inside a pressable control.
const interactiveTags = new Set(['button', 'link', 'checkbox', 'switch', 'radio', 'input', 'textarea', 'select', 'slider']);
const bindCodes = { input: 'UI_INPUT', textarea: 'UI_INPUT', checkbox: 'UI_CHECK', switch: 'UI_CHECK', slider: 'UI_SLIDER', radio: 'UI_RADIO', select: 'UI_SELECT' };
const optionStyle = ['fontSize', 'fontWeight', 'fontFamily', 'lineHeight', 'color', 'background'];
// A component's id keys its local state and nodes. The compiler stamps each top-level component
// with `__lxid` (module path and declaration name), which survives minification and hot swaps; a
// component without one falls back to its function name, numbered only when names repeat.
const componentIds = new WeakMap(), componentNames = new Map();
let nextHandle = 1;
function componentId(tag) {
  if (typeof tag.__lxid === 'string') return tag.__lxid;
  let id = componentIds.get(tag);
  if (id !== undefined) return id;
  const name = tag.name || 'anonymous';
  const count = (componentNames.get(name) ?? 0) + 1;
  componentNames.set(name, count);
  id = count === 1 ? name : `${name}#${count}`;
  componentIds.set(tag, id);
  return id;
}

const handle = () => nextHandle++;
/** Handles issued so far: a row that issued none holds no callback. */
export const handlesIssued = () => nextHandle;
/** Marks callbacks as issued without issuing one: a recalled subtree carries its own. */
export const skipHandle = () => { nextHandle++; };
const pointerCrossing = ['onPointerEnter', 'onPointerLeave'];
const boxTags = new Set(['view', 'text', 'vector', 'plot', 'scroll']);
const pointerTags = new Set(['view', 'button', 'vector']), pointerAxes = new Set(['x', 'y', 'both']);
const callbackOf = (callback, kind = 'void', isolated = false, press = undefined) => ({ callback, kind, isolated, press });

const dataProp = /^data-[a-z][a-z0-9-]*$/;

/** The element's data-* props as `data-*` variants read them: present unless false or nullish. */
function dataOf(props) {
  let data;
  for (const key in props) {
    const value = props[key];
    if (!key.startsWith('data-') || !dataProp.test(key) || value === false || value == null) continue;
    (data ??= {})[key.slice(5)] = value === true ? '' : String(value);
  }
  return data;
}

function admit(tag, element, props) {
  let data = 0;
  for (const key in props) {
    if (common.has(key) || has(element.props, key)) continue;
    const value = props[key];
    if (dataProp.test(key)) {
      if (!['string', 'number', 'boolean'].includes(typeof value) && value != null) throw Error(`UI_PROP: ${key} must be a string, number or boolean`);
      if (typeof value === 'string' && value.length > 256) throw Error(`UI_PROP: ${key} exceeds 256 characters`);
      if (++data > 16) throw Error('UI_PROP: more than 16 data-* props');
      continue;
    }
    if (element.binding && (key === 'bind' || key === 'value' || key === element.binding.event)) continue;
    throw Error(`UI_PROP: ${key} is not a property of <${tag}>`);
  }
  if (props.key !== undefined && typeof props.key !== 'string') throw Error('UI_PROP: key must be a string');
  if (props['data-testid'] !== undefined && typeof props['data-testid'] !== 'string') throw Error('UI_PROP: data-testid must be a string');
  if (props.disabled !== undefined && typeof props.disabled !== 'boolean') throw Error('UI_PROP: disabled');
  if (props.style !== undefined && (!props.style || typeof props.style !== 'object' || Array.isArray(props.style))) throw Error('UI_PROP: style');
}

function bind(tag, element, props) {
  const { type, event } = element.binding;
  const code = bindCodes[tag];
  if (props.bind !== undefined) {
    const cell = cellOf(props.bind);
    if (!cell || render.candidate.locals.get(cell.key) !== cell.cell || typeof props.bind.value !== type || props.value !== undefined || props[event] !== undefined)
      throw Error(`${code}: bind requires a mounted ${type} useLocal cell without value/${event}`);
    const binding = props.bind;
    props.value = binding.value; props[event] = value => binding.set(value);
  }
  if (props[event] !== undefined && typeof props[event] !== 'function') throw Error(`${code}: ${event}`);
}

// The lowered style of class-only nodes repeats across a page; it depends on
// nothing but these inputs, so one lowering serves every node that shares them.
const lowered = new Map();
let loweredEnv = null, loweredClasses = classGeneration;
// Keys lowered since the last `takeLowered`, and the lowerings a previous isolate of this module took.
const fresh = new Set();
let seeded = null;
// The only env fields lowering may read: each is in the key, so a seed from another isolate holds.
const loweringFields = ['theme', 'viewport', 'pointer', 'reducedMotion', 'insets', 'keyboard'];
const loweringEnv = env => Object.fromEntries(loweringFields.map(name => [name, env?.[name]]));
// A view's environment is frozen, so its key is computed once per environment, not per node.
let keyedEnv = {}, keyedEnvKey;
function envKey(env) {
  if (env !== keyedEnv) {
    keyedEnv = env;
    keyedEnvKey = JSON.stringify(loweringFields.map(name => env?.[name]));
  }
  return keyedEnvKey;
}

/** Lowered styles for the current env once any is new since the last take, as JSON; '' otherwise. */
export function takeLowered() {
  if (!fresh.size) return '';
  fresh.clear();
  // A set this large would not be kept, so it is not serialized either.
  if (lowered.size > 400) return '';
  return JSON.stringify({ env: loweredEnv, entries: [...lowered].filter(([, style]) => portable(style)) });
}

// Whether JSON carries a style unchanged. The only fields it may drop are these, read solely by value.
const droppable = new Set(['children', 'every', 'release']);
function portable(style) {
  return Object.entries(style).every(([name, value]) => value === undefined ? droppable.has(name)
    : name === 'typography' ? Object.entries(value).every(([field, size]) => size === undefined ? field === 'lineHeight' : plainJson(size))
    : plainJson(value));
}

function plainJson(value) {
  if (value === null || typeof value === 'string' || typeof value === 'boolean') return true;
  if (typeof value === 'number') return Number.isFinite(value) && !Object.is(value, -0);
  if (Array.isArray(value)) return value.every(plainJson);
  if (typeof value !== 'object' || Object.getPrototypeOf(value) !== Object.prototype) return false;
  return Object.values(value).every(plainJson);
}

/** Lowerings a previous isolate of this module took; false when `text` is not such a list. */
export function seedLowered(text) {
  let value;
  try { value = JSON.parse(text); } catch { return false; }
  const entry = item => Array.isArray(item) && item.length === 2 && typeof item[0] === 'string' && item[1] !== null && typeof item[1] === 'object';
  if (typeof value?.env !== 'string' || !Array.isArray(value.entries) || !value.entries.every(entry)) return false;
  seeded = value;
  // A module configures its class table before it is seeded, and the seed was taken under that table.
  if (loweredClasses !== classGeneration) {
    loweredClasses = classGeneration;
    loweredEnv = null;
  }
  if (value.env === loweredEnv) for (const [key, style] of value.entries) if (!lowered.has(key)) lowered.set(key, style);
  return true;
}

function nodeStyle(tag, props, data, extra, inherited, direction, structure, every) {
  if (props.style !== undefined || extra !== undefined || every !== undefined || props.className === undefined) return lowerNodeStyle(tag, props, data, extra, inherited, direction, structure, every);
  const environment = envKey(envOf(render.view));
  if (loweredClasses !== classGeneration) {
    // A new class table: what was lowered, here or by a previous isolate, read the old one.
    loweredClasses = classGeneration;
    seeded = null;
    loweredEnv = null;
  }
  if (environment !== loweredEnv) {
    lowered.clear();
    fresh.clear();
    loweredEnv = environment;
    if (seeded?.env === environment) for (const [key, style] of seeded.entries) lowered.set(key, style);
  }
  // An inherited line height may be a multiplier object; it keys by value, not as "[object Object]".
  const key = `${tag}\u0000${props.className}\u0000${data ? JSON.stringify(data) : ''}\u0000${props.disabled === true}\u0000${checkedSwitch(tag, props)}\u0000${direction}\u0000${(structure ?? []).join(",")}\u0000${inherited.fontSize}\u0000${JSON.stringify(inherited.lineHeight)}`;
  let style = lowered.get(key);
  if (!style) {
    style = lowerNodeStyle(tag, props, data, extra, inherited, direction, structure);
    fresh.add(key);
    if (lowered.size >= 2048) lowered.delete(lowered.keys().next().value);
    lowered.set(key, style);
  }
  // Callers may edit the fields of their node (an image's fit moves to its own field).
  return { ...style, fields: { ...style.fields } };
}

// The class token that set `key`, for an error that names what the author wrote.
function authoredClass(className, key, env) {
  for (const token of className.split(/\s+/).filter(Boolean)) {
    const one = resolveClasses(token, env);
    if (one?.base && key in one.base || one?.disabled && key in one.disabled) return token;
  }
  return className;
}

const fieldTags = new Set(['input', 'textarea', 'select']);
const runTags = new Set(['span', 'link']);
const disabledField = { color: 'text-muted', backgroundColor: 'surface-sunken', borderColor: 'line' };
// A player's box is black before its first frame and behind letterboxing, as native players are.
const videoBacking = { backgroundColor: '#000000' };

// `checked:` before a switch part follows the switch's own value.
const checkedSwitch = (tag, props) => tag === 'switch' && props.value === true;

// Classes, then the parent's child rule, then inline style; `disabled:` replaces the class cascade.
function lowerNodeStyle(tag, props, data, extra, inherited, direction, structure, every) {
  const env = { ...loweringEnv(envOf(render.view)), direction, structure, data, checked: checkedSwitch(tag, props) };
  const classes = resolveClasses(props.className, env);
  // A run takes the size of `text-sm` but not the line height paired with it.
  if (classes?.pairedLeading && runTags.has(tag)) delete classes.base.lineHeight;
  const disabled = props.disabled === true;
  // A disabled editor without `disabled:` classes still reads as disabled: muted roles over its classes.
  const muted = disabled && !classes?.disabled && fieldTags.has(tag) ? disabledField : undefined;
  const base = disabled && classes?.disabled ? classes.disabled : classes?.base;
  const stray = base && Object.keys(base).find(key => !elements[tag].style.includes(key) && !(props.style && key in props.style));
  if (stray) throw Error(classElementProblem(authoredClass(props.className, stray, env), stray, tag));
  const author = mergeStyle(direction, tag === 'video' ? videoBacking : undefined, extra, base, every, muted, props.style);
  const flexOnly = author?.display === 'grid' && base && gridRefuses(Object.keys(base).filter(key => !(props.style && key in props.style)));
  if (flexOnly) throw Error(gridProblem(authoredClass(props.className, flexOnly, env), flexOnly));
  // A box is a flex column; `flex` alone asks for a row, as `display: flex` does in CSS.
  if ((tag === 'view' || tag === 'page') && (author?.display === 'flex' || author?.display === 'inline-flex') && author.flexDirection === undefined)
    author.flexDirection = 'row';
  const { fields, typography } = lowerStyle(tag, author, env, inherited);
  const states = {};
  const state = (name, wire, allowed, requirement) => {
    const delta = classes?.[name];
    if (!delta || !Object.keys(delta).length) return;
    if (!allowed) throw Error(`UI_STYLE: ${name}: requires ${requirement}, not <${tag}>`);
    paintOnly(name, tag, delta);
    states[wire] = lowerStyle(tag, delta, env, { fontSize: typography.fontSize }).fields;
  };
  state('focus', 'focusStyle', accepts('focusStyle', tag), 'an editor or <select>');
  state('focusVisible', 'focusVisibleStyle', accepts('focusVisibleStyle', tag), 'an element that takes focus');
  if (!disabled) {
    state('active', 'pressedStyle', accepts('pressedStyle', tag), 'a control');
    state('hover', 'hoverStyle', true);
  }
  // Presence: the paint an element enters from and leaves toward, with its own transition.
  state('starting', 'startingStyle', true);
  state('exit', 'exitStyle', true);
  state('focusWithin', 'focusWithinStyle', true);
  state('groupActive', 'groupPressedStyle', true);
  if (classes?.backdrop && tag !== 'dialog') throw Error(`UI_STYLE: backdrop: styles a <dialog>'s backdrop, not <${tag}>`);
  // `has-checked:`/`has-disabled:` depend on descendants, known once the children resolve.
  const descendants = {};
  for (const [name, kind] of [['hasChecked', 'checked'], ['hasDisabled', 'disabled']]) {
    const delta = classes?.[name];
    if (!delta || !Object.keys(delta).length) continue;
    paintOnly(name.replace(/^has/, 'has-').toLowerCase(), tag, delta);
    descendants[kind] = lowerStyle(tag, delta, env, { fontSize: typography.fontSize }).fields;
  }
  return { fields, states, typography, author, children: classes?.children, every: classes?.every, descendants, release: classes?.release };
}

/** The tag `List` renders; page code reaches the list element only through it. */
export const listTag = Object.freeze({});

export function resolveNode(node, identity, depth = 0, parent = {}) {
  if (depth > limits.depth) throw Error('UI_LIMIT: children too deep');
  if (node == null || typeof node === 'boolean' || node === '') return null;
  if (typeof node === 'string' || typeof node === 'number') node = h('text', {}, node);
  // A component's fragment (or array): its parent lays the children out in its place.
  if (Array.isArray(node)) return { fragment: entries(node) };
  if (isFragment(node)) return { fragment: entries([node]).map(entry => entry.slot ? { ...entry, slot: entry.slot.slice(2) } : entry) };
  if (!isNode(node)) throw Error('UI_ROOT: expected one node');
  const record = readNode(node);
  const authored = record[0], original = record[1], children = record[2], location = record[3];
  // `<List>` renders the list element, which page code cannot name.
  if (authored === 'list') throw located(Error('UI_TAG: <list> is not an element; use <List> from @lingxia/view'), `<list>${location ? ` (${location})` : ''}`);
  const tag = authored === listTag ? 'list' : authored;
  // Where a render that never returns was, for UI_TIMEOUT.
  if (location) render.site = `${typeof tag === 'function' ? tag.name || '<anonymous>' : `<${tag}>`} (${location})`;
  if (typeof tag === 'function') {
    const previous = render.scope, scope = `${identity}/component:${componentId(tag)}`;
    render.scope = scope;
    const supplied = children.length === 0 ? undefined : children.length === 1 ? children[0] : children;
    try {
      if (isProvider(tag)) return provide(tag, original, children, parent.contexts,
        (child, contexts) => resolveNode(child, scope, depth + 1, { ...parent, contexts }));
      const recalled = recall(scope, tag, original, children, parent);
      if (recalled !== undefined) return recalled;
      const frame = enter(scope, parent.contexts);
      const outerContexts = render.contexts;
      let result;
      try {
        render.contexts = parent.contexts;
        const output = tag({ ...original, children: supplied });
        render.contexts = outerContexts;
        result = resolveNode(output, scope, depth + 1, parent);
      }
      finally { render.contexts = outerContexts; leave(frame); }
      remember(frame, tag, original, children, parent, result);
      return result;
    }
    catch (error) { throw located(error, `${tag.name || '<anonymous>'}${location ? ` (${location})` : ''}`); }
    finally { render.scope = previous; }
  }
  try {
    // An open <dialog> or popover becomes a top-layer box; a closed one renders nothing.
    const expansion = expand(tag, original, children, identity, parent);
    if (expansion === null) return null;
    if (expansion) {
      const result = resolveNode(expansion.node, identity, depth + 1, { ...parent, extra: undefined, every: undefined, sibling: undefined });
      expansion.finish(result);
      return result;
    }
    return resolveElement(tag, original, children, identity, depth, parent);
  }
  catch (error) { throw located(error, `<${tag}>${location ? ` (${location})` : ''}`); }
}

// The first strong directional character of authored content: Hebrew, Arabic and the other
// right-to-left scripts are rtl, any other letter ltr; `undefined` when there is none.
const rtlScripts = /[\u0590-\u08FF\uFB1D-\uFDFF\uFE70-\uFEFF]/u;
function strongDirection(children, depth = 0) {
  for (const child of Array.isArray(children) ? children : [children]) {
    if (typeof child === 'string') {
      const letter = child.match(/\p{L}/u)?.[0];
      if (letter) return rtlScripts.test(letter) ? 'rtl' : 'ltr';
    } else if (Array.isArray(child) && depth < 32) {
      const found = strongDirection(child, depth + 1);
      if (found) return found;
    } else if (isNode(child) && depth < 32) {
      const found = strongDirection(readNode(child)[2], depth + 1);
      if (found) return found;
    }
  }
  return undefined;
}

// DOM-named accessibility props as the runtime carries them.
const ariaNames = [['aria-label', 'accessibilityLabel'], ['aria-hidden', 'accessibilityHidden'], ['aria-live', 'accessibilityLiveRegion'],
  ['aria-valuenow', 'ariaValueNow'], ['aria-valuemin', 'ariaValueMin'], ['aria-valuemax', 'ariaValueMax'], ['aria-valuetext', 'ariaValueText'],
  ['aria-level', 'ariaLevel']];
const ariaStates = [['aria-pressed', 'pressed'], ['aria-selected', 'selected'], ['aria-expanded', 'expanded'], ['aria-busy', 'busy'],
  ['aria-invalid', 'invalid'], ['aria-required', 'required'], ['aria-checked', 'checked']];
function ariaProps(props) {
  for (let i = 0; i < ariaNames.length; i++) {
    const authored = ariaNames[i][0];
    if (authored in props) { props[ariaNames[i][1]] = props[authored]; delete props[authored]; }
  }
  let state;
  for (let i = 0; i < ariaStates.length; i++) {
    const authored = ariaStates[i][0];
    if (props[authored] === undefined) continue;
    (state ??= {})[ariaStates[i][1]] = props[authored];
    delete props[authored];
  }
  if (state) props.accessibilityState = state;
}

const dragProps = ['dragSnapPoints', 'bind', 'onDragStart', 'onDragEnd'];
function resolveElement(tag, original, children, identity, depth, parent) {
  if (typeof tag !== 'string' || !has(elements, tag)) throw Error(`UI_TAG: ${String(tag)}${tagHint(tag)}`);
  const element = elements[tag];
  const props = { ...original };
  admit(tag, element, props);
  // `data-testid` names the node for automation; it is not one of the data-* variant attributes.
  if (props['data-testid'] !== undefined) { props.testId = props['data-testid']; delete props['data-testid']; }
  ariaProps(props);
  // Controls nest inside a pressable control and take presses in their own bounds; scrolls do not.
  if (parent.inControl && (tag === 'scroll' || tag === 'list')) throw Error('UI_BUTTON: a scroll or list cannot sit inside a pressable control');
  if (element.binding) bind(tag, element, props);
  const editor = tag === 'input' || tag === 'textarea';
  if (props.dir !== undefined && !['ltr', 'rtl', 'auto'].includes(props.dir)) throw Error('UI_PROP: dir must be "ltr", "rtl" or "auto"');
  // `dir` overrides the direction inherited from the environment for its subtree. `auto` takes the
  // direction of the content's first strong character, as in HTML; an editor follows its live text.
  const own = props.dir === 'auto' && !editor ? strongDirection(children) : props.dir === 'auto' ? undefined : props.dir;
  if (props.dir === 'auto' && !editor) props.dir = own;
  const direction = own ?? parent.direction ?? (envOf(render.view)?.direction === 'rtl' ? 'rtl' : 'ltr');
  const authored = entries(children), flat = authored.map(entry => entry.child);
  // The data-* props as variants and automation read them; nothing below changes them.
  const data = dataOf(props);
  // The root takes the theme's `--font-sans`, as preflight gives the document's root; its classes and style still win.
  const rootFamily = !parent.typography && pageFamily && element.style.includes('fontFamily') ? { fontFamily: pageFamily } : undefined;
  const style = nodeStyle(tag, props, data, parent.extra ?? rootFamily, parent.typography ?? { fontSize: 14 }, direction, structure(parent.sibling, flat), parent.every);
  const inline = child => typeof child === 'string' || typeof child === 'number';
  if ((tag === 'span' || tag === 'link') && !parent.inText) throw Error(`UI_TEXT: <${tag}> belongs inside <text>`);
  if (tag === 'link' && parent.inLink) throw Error('UI_TEXT: links do not nest');
  const composed = element.children === 'content' && flat.some(child => !inline(child));
  const runs = element.children === 'runs' && flat.some(child => !inline(child));
  const leaf = element.children === 'text' || (element.children === 'content' && !composed) || (element.children === 'runs' && !runs);
  if (leaf && flat.some(child => typeof child !== 'string' && typeof child !== 'number')) throw Error(`UI_CHILD: <${tag}> accepts only text`);
  if (element.children === 'none' && flat.length) throw Error(`UI_CHILD: <${tag}> takes no children`);
  const disabled = props.disabled === true;
  // A search field draws its magnifier and search keyboard natively, as its own wire tag.
  const result = { tag: tag === 'input' && props.type === 'search' ? 'search' : element.wire, element: tag, props, fields: style.fields, states: style.states, typography: style.typography,
    identity, text: leaf ? flat.join('') : '', wire: [], handlers: [], children: [] };
  const wire = (key, value) => { if (!dropped(key)) result.wire.push([key, value]); };
  const on = (key, callback, kind, isolated) => result.handlers.push([key, handle(), callbackOf(callback, kind, isolated)]);
  if (disabled) wire('enabled', 0);
  // Authored data-* props reach automation (getAttribute, [data-x] selectors), never semantics.
  if (data) wire('dataAttributes', JSON.stringify(data));
  if (props.dir !== undefined) wire('direction', props.dir);
  semantics(tag, props, wire);
  if (props.drag !== undefined) dragElement(tag, props, wire, on, style.release);
  else for (let i = 0; i < dragProps.length; i++) {
    const key = dragProps[i];
    if (props[key] !== undefined && !element.binding && !(key === 'bind' && tag === 'scroll')) throw Error(`UI_PROP: ${key} belongs to an element with drag`);
  }
  // A snapping scroll's `bind`: the index of the snap point it rests on, two-way.
  if (tag === 'scroll' && props.bind !== undefined) {
    const cell = props.bind, bound = cellOf(cell);
    if (!bound || render.candidate.locals.get(bound.key) !== bound.cell || !Number.isInteger(cell.value) || cell.value < 0)
      throw Error('UI_PROP: bind on a <scroll> takes a mounted useLocal cell holding a snap index');
    wire('snapIndex', cell.value);
    on('onSnapChange', payload => {
      const index = Number(payload);
      if (Number.isInteger(index) && index >= 0 && cell.value !== index) cell.set(index);
    }, 'text');
  }
  if (props[scrimMark]) wire('dragScrim', 1);
  // A fine pointer crossing the box; a finger never hovers, so touch fires neither.
  for (const key of pointerCrossing) {
    if (props[key] === undefined) continue;
    if (tag !== 'view' && tag !== 'button') throw Error(`UI_PROP: ${key} belongs to a <view> or <button>`);
    if (typeof props[key] !== 'function') throw Error(`UI_PROP: ${key} takes a function`);
    on(key, props[key]);
  }
  // Where a pointer is over the element; it observes, so a button inside still gets its click.
  if (props.onPointer !== undefined || props.pointerAxis !== undefined) {
    if (!pointerTags.has(tag)) throw Error('UI_PROP: onPointer belongs to a <view>, <button> or <vector>');
    const { onPointer, pointerAxis = 'both' } = props;
    if (typeof onPointer !== 'function') throw Error('UI_PROP: onPointer takes a function; pointerAxis goes with it');
    if (!pointerAxes.has(pointerAxis)) throw Error('UI_PROP: pointerAxis must be "x", "y" or "both"');
    if (!dropped('onPointer')) {
      on('onPointer', payload => onPointer(Object.freeze(JSON.parse(payload))), 'text');
      if (pointerAxis !== 'both') wire('pointerAxis', pointerAxis);
    }
  }
  // The element's content box once laid out, and after each change; an older host never reports one.
  if (props.onLayout !== undefined || props.box !== undefined) {
    if (!boxTags.has(tag)) throw Error('UI_PROP: onLayout and box belong to a <view>, <text>, <vector>, <plot> or <scroll>');
    const { onLayout } = props;
    if (onLayout !== undefined && typeof onLayout !== 'function') throw Error('UI_PROP: onLayout takes a function');
    const binding = props.box === undefined ? undefined : boxBinding(props.box);
    if (props.box !== undefined && !binding) throw Error('UI_PROP: box takes the handle useBox(name) returns');
    if (!dropped('onLayout')) on('onLayout', payload => {
      const { width, height } = JSON.parse(payload);
      const size = { width, height }, held = binding?.value;
      if (binding && !(held && held.width === width && held.height === height)) binding.set(size);
      onLayout?.(size);
    }, 'text');
  }
  if (props['aria-actions'] !== undefined) {
    // The ids of the controls behind a swipe row: assistive tech offers them as the row's own actions.
    const ids = typeof props['aria-actions'] === 'string' ? props['aria-actions'].trim().split(/\s+/) : [];
    if (props.drag === undefined) throw Error('UI_PROP: aria-actions belongs to an element with drag');
    if (!ids.length || ids.length > 8 || ids.some(id => !id || id.length > 256)) throw Error('UI_PROP: aria-actions lists 1..8 element ids');
    wire('ariaActions', ids.join(' '));
  }
  if (props['aria-describedby'] !== undefined) {
    // Elements whose text is read after this one's name: a hint, or the error of an invalid field.
    const ids = typeof props['aria-describedby'] === 'string' ? props['aria-describedby'].trim().split(/\s+/) : [];
    if (!ids.length || ids.length > 8 || ids.some(id => !id || id.length > 256)) throw Error('UI_PROP: aria-describedby lists 1..8 element ids');
    wire('ariaDescribedBy', ids.join(' '));
  }
  if (props.id !== undefined) {
    if (tag === 'span' || tag === 'link') throw Error(`UI_ID: <${tag}> is a run of its <text>; put the id on the <text>`);
    if (typeof props.id !== 'string' || !props.id.length || props.id.length > 256 || /\s/.test(props.id)) throw Error('UI_ID: an id is 1..256 characters without spaces');
    wire('elementId', props.id);
  }
  if (tag === 'dialog') {
    if (props.role !== undefined && props.role !== 'alertdialog') throw Error('UI_PROP: a <dialog> keeps its own role; only role="alertdialog" marks one that interrupts');
    if (!props.accessibilityLabel) throw Error('UI_A11Y: <dialog> needs an accessible name; add aria-label="…"');
    wire('role', props.role ?? 'dialog');
  }
  switch (tag) {
    case 'link':
      check.label(props.accessibilityLabel, 'UI_TEXT');
      if (props.onClick !== undefined && typeof props.onClick !== 'function') throw Error('UI_PROP: onClick');
      if (props.accessibilityLabel !== undefined) wire('accessibilityLabel', props.accessibilityLabel);
      if (props.onClick && !disabled) on('onClick', props.onClick);
      break;
    case 'button': {
      check.label(props.accessibilityLabel, 'UI_BUTTON');
      if (props.onClick !== undefined && typeof props.onClick !== 'function') throw Error('UI_PROP: onClick');
      if (props.accessibilityLabel !== undefined) wire('accessibilityLabel', props.accessibilityLabel);
      const click = invokerHandler(props, identity, parent.popovers ?? []);
      interestTrigger(props, identity, parent.popovers ?? []);
      if (props.interestFor !== undefined) wire('interestFor', props.interestFor);
      if (click && !disabled) on('onClick', click);
      break;
    }
    case 'checkbox': case 'switch':
      check.label(props.accessibilityLabel, 'UI_CHECK');
      if (props.value !== undefined && typeof props.value !== 'boolean') throw Error('UI_CHECK: value must be boolean');
      if (props.accessibilityLabel !== undefined) wire('accessibilityLabel', props.accessibilityLabel);
      wire('checked', +(props.value ?? false));
      if (props.onChange && !disabled) { const { onChange, value } = props; on('onCheckedChange', () => onChange(!(value ?? false))); }
      break;
    case 'slider':
      check.slider(props);
      for (const [key, name] of [['sliderValue', 'value'], ['sliderMin', 'min'], ['sliderMax', 'max'], ['sliderStep', 'step'], ['accessibilityLabel', 'accessibilityLabel']])
        wire(key, props[name]);
      if (props.secondaryValue !== undefined) wire('sliderSecondary', check.sliderSecondary(props));
      if (props.onChange && !disabled) on('onSliderChange', value => {
        const next = check.sliderValue(props, value);
        if (next !== props.value) props.onChange(next);
      }, 'number');
      if (props.onPressChange !== undefined) {
        if (typeof props.onPressChange !== 'function') throw Error('UI_SLIDER: onPressChange takes a function');
        const report = props.onPressChange;
        // Kept while disabled: a press that a disable ends still reaches its current callback.
        if (!dropped('onSliderPress')) result.handlers.push(['onSliderPress', handle(),
          callbackOf(() => pressChange(identity, report), 'void', false, { identity, callback: report })]);
      }
      break;
    case 'input': case 'textarea':
      check.label(props.accessibilityLabel, 'UI_INPUT');
      if (!validInputText(props.value === undefined ? '' : props.value) || !validInputText(props.placeholder === undefined ? '' : props.placeholder)) throw Error('UI_INPUT: invalid text');
      if (props.autoFocus !== undefined && typeof props.autoFocus !== 'boolean') throw Error('UI_INPUT: autoFocus');
      const { keyboard, secure } = check.editorKind(tag, props);
      const attributes = check.editor(tag, props, keyboard);
      if (props.onSubmit !== undefined && typeof props.onSubmit !== 'function') throw Error('UI_INPUT: onSubmit');
      if (props.accessibilityLabel !== undefined) wire('accessibilityLabel', props.accessibilityLabel);
      wire('inputValue', props.value ?? '');
      if (props.placeholder !== undefined) wire('placeholder', props.placeholder);
      if (keyboard !== undefined) wire('keyboard', keyboard);
      if (keyboard === 'date' || keyboard === 'time') {
        // Shown in the user's clock and sized to its widest value; the bound value stays ISO.
        const env = envOf(render.view);
        const shown = fieldText(keyboard, props.value ?? '', {
          seconds: typeof props.step === 'number' && props.step < 60,
          language: env?.contentLanguage ?? env?.displayLanguage,
          hourCycle: env?.hourCycle,
        });
        wire('displayValue', shown.displayValue);
        wire('sizeText', shown.sizeText);
      }
      if (secure) wire('secure', 1);
      if (props.autoFocus !== undefined) wire('autoFocus', +props.autoFocus);
      // A textarea whose Return key names an action submits instead of breaking the line.
      if (tag === 'textarea' && ['done', 'go', 'search', 'send'].includes(props.enterKeyHint)) wire('submitOnEnter', 1);
      if (props.bind !== undefined && cellOf(props.bind).cell.focus) wire('focusRequest', cellOf(props.bind).cell.focus);
      for (const [key, value] of attributes) wire(key, value);
      if (tag === 'textarea') {
        const { rows, maxRows } = props;
        const valid = value => value === undefined || (Number.isInteger(value) && value >= 1 && value <= 100);
        if (!valid(rows) || !valid(maxRows) || (maxRows !== undefined && (rows === undefined || maxRows < rows)))
          throw Error('UI_INPUT: rows and maxRows are integers 1..100, maxRows needs rows and is at least rows');
        if (rows !== undefined) wire('rows', rows);
        if (maxRows !== undefined) wire('maxRows', maxRows);
      }
      if (!disabled) for (const name of ['onInput', 'onSubmit']) if (props[name]) on(name, props[name], 'text');
      break;
    case 'select': {
      check.label(props.accessibilityLabel, 'UI_SELECT');
      const options = selectOptions(props);
      if (props.value !== undefined && typeof props.value !== 'string') throw Error('UI_SELECT: value must be a string');
      if (props.placeholder !== undefined && !validInputText(props.placeholder)) throw Error('UI_SELECT: invalid placeholder');
      const chosen = options.find(option => option.value === props.value);
      // The box shows the chosen label, else the placeholder in placeholder colour.
      result.text = chosen?.label ?? props.placeholder ?? '';
      if (props.accessibilityLabel !== undefined) wire('accessibilityLabel', props.accessibilityLabel);
      wire('inputValue', chosen ? chosen.value : '');
      if (props.placeholder !== undefined) wire('placeholder', props.placeholder);
      wire('selectOptions', JSON.stringify(options));
      if (props.onChange && !disabled) on('onSelect', value => { if (value !== props.value) props.onChange(value); }, 'text');
      break;
    }
    case 'image':
      check.image(props);
      wire('imageSrc', props.src);
      if (props.alt !== undefined) wire('imageAlt', props.alt);
      if (props.retryKey !== undefined) wire('imageRetryKey', props.retryKey);
      if (props.placeholder !== undefined) wire('imagePlaceholder', props.placeholder);
      if (props.onLoad) on('onImageLoad', props.onLoad, 'void', true);
      if (props.onError) on('onImageError', props.onError, 'void', true);
      break;
    case 'video': {
      check.video(props);
      const name = props.player === undefined ? undefined : playerName(props.player);
      if (props.player !== undefined && name === undefined) throw Error('UI_VIDEO: player takes the handle useVideo(name) returns');
      const source = props.src ?? props.sources;
      if (source === undefined && name === undefined) throw Error('UI_VIDEO: a <video> needs src or sources, or a player another <video> gives one');
      if (props.src !== undefined) wire('videoSrc', props.src);
      if (props.sources !== undefined) wire('videoSources', JSON.stringify(props.sources.map(({ src, label }) => ({ src, label }))));
      // A named player is the page's; a bare <video> owns one keyed by its place in the tree.
      wire('videoPlayer', name === undefined ? `el:${identity}` : `name:${name}`);
      if (name !== undefined && parent.inDialog) shownInDialog(name);
      if (props.poster !== undefined) wire('videoPoster', props.poster);
      // Without autoPlay a player waits on its first frame, as HTML's; `paused` wins over it.
      if (source !== undefined) wire('videoPaused', +(props.paused ?? !props.autoPlay));
      for (const [key, prop] of videoFlags) if (props[prop] !== undefined) wire(key, +props[prop]);
      if (props.playbackRate !== undefined) wire('videoRate', props.playbackRate);
      if (props.volume !== undefined) wire('videoVolume', props.volume);
      if (props.accessibilityLabel !== undefined) { check.label(props.accessibilityLabel, 'UI_VIDEO'); wire('accessibilityLabel', props.accessibilityLabel); }
      if (name !== undefined && props.onFullscreenChange !== undefined) {
        if (typeof props.onFullscreenChange !== 'function') throw Error('UI_VIDEO: onFullscreenChange takes a function');
        fullscreenHandler(name, props.onFullscreenChange);
      }
      const handlers = videoEvents.filter(([, prop]) => props[prop]);
      if (name !== undefined || handlers.length) on('onVideoEvent', payload => {
        const { status, events } = JSON.parse(payload);
        if (name !== undefined) videoStatus(name, status);
        for (const { type, ...detail } of events) {
          const handler = props[videoEvents.find(([event]) => event === type)?.[1]];
          if (handler) handler(type === 'error' ? Object.freeze({ ...status.error }) : Object.freeze(detail));
        }
      }, 'text', true);
      break;
    }
    case 'vector': {
      const fit = result.fields.imageFit; delete result.fields.imageFit;
      wire('vectorSource', check.vector(props, fit));
      break;
    }
    case 'plot': {
      check.plot(props);
      wire('plotValues', JSON.stringify(props.values)); wire('plotKind', props.kind);
      wire('plotColors', JSON.stringify(props.colors ?? defaultSeries)); wire('accessibilityLabel', props.accessibilityLabel);
      for (const name of ['smooth', 'area']) if (props[name] !== undefined) wire(`plot${name[0].toUpperCase()}${name.slice(1)}`, +props[name]);
      for (const [key, name] of [['plotDomainMin', 'domainMin'], ['plotDomainMax', 'domainMax'], ['plotMarkerEmphasisScale', 'markerEmphasisScale'], ['plotSelectedIndex', 'selectedIndex']])
        if (props[name] !== undefined) wire(key, props[name]);
      if (props.pieLayout !== undefined) wire('plotPieLayout', JSON.stringify(props.pieLayout));
      if (props.onSelect) { const { onSelect, values } = props; on('onPlotSelect', index => {
        if (Number.isInteger(index) && index >= -1 && index < values.length) onSelect(index);
      }, 'number'); }
      const count = props.values.length, points = (count - 1) * (props.smooth ? 8 : 1) + 1;
      result.plotWork = props.kind === 'pie' ? 256 + count : props.kind === 'line'
        ? points * (props.area ? 2 : 1) + 2 * count + (props.area ? 2 : 0) : 8 * count;
      break;
    }
    case 'radio':
      check.label(props.accessibilityLabel, 'UI_RADIO');
      if (props.value !== undefined && (typeof props.value !== 'string' || props.value.length > 256)) throw Error('UI_RADIO: invalid value');
      if (props.accessibilityLabel !== undefined) wire('accessibilityLabel', props.accessibilityLabel);
      result.children = radioOptions(props, identity, result.fields, disabled);
      break;
  }
  if (tag === 'scroll' || tag === 'list') for (const name of ['onReachStart', 'onReachEnd']) {
    if (props[name] === undefined) continue;
    if (typeof props[name] !== 'function') throw Error(`UI_PROP: ${name}`);
    on(name, props[name]);
  }
  if (tag === 'list') {
    check.listKeys(props);
    result.list = listState(props, identity, direction, parent.contexts);
    render.lists++;
  } else if (runs) {
    // Nested spans flatten into one run list; inner runs layer over the outer span's style.
    const extra = tag === 'span' || tag === 'link' ? style.author : undefined;
    result.children = flat.flatMap((child, position) => {
      const at = authored[position], part = at.slot === undefined ? `key:${JSON.stringify(at.key)}` : `position:${at.slot}`;
      const run = resolveNode(inline(child) ? h('span', {}, child) : child, `${identity}/${tag}/${part}`, depth + 1, {
        tag, typography: style.typography, direction, extra, inText: true, inControl: parent.inControl, inLink: parent.inLink || tag === 'link',
        contexts: parent.contexts,
      });
      if (run === null) return [];
      if (run.fragment) throw Error(`UI_TEXT: a component inside <${tag}> returns a fragment; return one <span>`);
      return (run.element === 'span' || run.element === 'link') && run.children.length ? run.children : [run];
    });
    // A styled link becomes consecutive link pieces sharing one handle, label and pressed style.
    if (tag === 'link') result.children = result.children.map((piece, i) => ({
      ...piece, tag: 'link', element: 'link', states: result.states, handlers: result.handlers.map(entry => [...entry]),
      // The first piece stands for the whole link, so it answers to the link's test id.
      props: i === 0 && props.testId !== undefined ? { ...piece.props, testId: props.testId } : piece.props,
      wire: [...piece.wire, ...result.wire.filter(([key]) => i === 0 || key !== 'accessibilityLabel')],
    }));
    if (tag === 'text' && result.children.length > limits.runs) throw Error(`UI_TEXT: more than ${limits.runs} runs`);
  } else if (!leaf && tag !== 'radio') {
    // Placed children and, for unkeyed ones, the slot they were authored at.
    const keys = new Set(), last = flat.length - 1, placed = [], slots = [];
    const place = ({ child, slot: at, key }, position, nesting) => {
      if (nesting > 128) throw Error('UI_LIMIT: children too deep');
      if (key !== undefined && typeof key !== 'string') throw Error('UI_KEY: expected string');
      if (key !== undefined && keys.has(key)) throw Error('UI_KEY: duplicate sibling');
      if (key !== undefined) keys.add(key);
      const part = key === undefined ? `position:${at}` : `key:${JSON.stringify(key)}`;
      const node = resolveNode(child, `${identity}/${tag}/${part}`, depth + 1, {
        tag, typography: style.typography, direction, extra: position < last ? style.children : undefined, every: style.every,
        sibling: { index: position, count: flat.length },
        inControl: parent.inControl || admission.tags[element.wire].children === 'content',
        inDialog: parent.inDialog || tag === 'dialog',
        popovers: props[popoverScope] ?? parent.popovers, contexts: parent.contexts,
      });
      // A component's fragment opens in its place, named under the component's slot or key.
      if (node?.fragment) {
        for (const inner of node.fragment) place(key === undefined
          ? { ...inner, slot: inner.slot === undefined ? undefined : `${at}.${inner.slot}`, key: inner.key }
          : { child: inner.child, key: JSON.stringify([key, inner.key ?? `#${inner.slot}`]) }, position, nesting + 1);
        return;
      }
      if (node === null) return;
      if (key === undefined) { placed.push(node); slots.push(at); return; }
      // Inside a keyed fragment the wire key is the composed one native names the node by.
      placed.push(node.props.key === key ? node : { ...node, props: { ...node.props, key } });
      slots.push(undefined);
    };
    for (let position = 0; position < authored.length; position++) place(authored[position], position, 0);
    result.children = placed;
    // A pressable control around nothing but another control adds a second target for one action.
    if ((tag === 'button' || tag === 'checkbox') && result.children.length === 1 && interactiveTags.has(result.children[0].tag))
      throw Error(`UI_BUTTON: <${tag}> holds only a <${result.children[0].tag}>; put the action on that control`);
    // Native names an unkeyed child by its rendered position; where a hole moved it, send the slot.
    for (let ordinal = 0; ordinal < placed.length; ordinal++) {
      const at = slots[ordinal];
      if (at !== undefined && at !== String(ordinal)) placed[ordinal] = { ...placed[ordinal], slot: at };
    }
  }
  for (const kind in style.descendants)
    if (result.children.some(child => holds(child, kind))) Object.assign(result.fields, style.descendants[kind]);
  return result;
}

// Whether a resolved node or one below it is checked (a checkbox, switch or answered radio) or disabled.
function holds(node, kind) {
  const own = kind === 'disabled' ? node.props?.disabled === true
    : (['checkbox', 'switch'].includes(node.element) && node.props.value === true) || (node.element === 'radio' && node.props.value != null);
  return own || (node.children ?? []).some(child => holds(child, kind));
}

// The structural pseudo-classes a node matches: position among its siblings, or no content.
function structure(sibling, flat) {
  const out = flat.length ? [] : ['empty'];
  if (!sibling) return out;
  const { index, count } = sibling;
  if (index === 0) out.push('first');
  if (index === count - 1) out.push('last');
  if (count === 1) out.push('only');
  out.push(index % 2 === 0 ? 'odd' : 'even');
  return out;
}

// A progressbar's amount: `aria-valuenow` within min (0) and max (100); without it, indeterminate.
function progressValues(role, props, wire, tag) {
  const { ariaValueNow: now, ariaValueMin: min, ariaValueMax: max, ariaValueText: text } = props;
  if (now === undefined && min === undefined && max === undefined && text === undefined) return;
  // A slider has its own value and range; it may only say how the value reads.
  if (tag === 'slider' && now === undefined && min === undefined && max === undefined) {
    if (typeof text !== 'string') throw Error('UI_PROP: aria-valuetext must be a string');
    wire('ariaValueText', text);
    return;
  }
  if (role !== 'progressbar') throw Error('UI_PROP: aria-valuenow, -valuemin, -valuemax and -valuetext belong on role="progressbar"');
  const number = (value, name) => {
    if (value !== undefined && (typeof value !== 'number' || !Number.isFinite(value))) throw Error(`UI_PROP: ${name} must be a finite number`);
  };
  number(now, 'aria-valuenow'); number(min, 'aria-valuemin'); number(max, 'aria-valuemax');
  if ((min ?? 0) >= (max ?? 100)) throw Error('UI_PROP: aria-valuemin must be below aria-valuemax');
  if (text !== undefined && typeof text !== 'string') throw Error('UI_PROP: aria-valuetext must be a string');
  if (now !== undefined) wire('ariaValueNow', now);
  if (min !== undefined) wire('ariaValueMin', min);
  if (max !== undefined) wire('ariaValueMax', max);
  if (text !== undefined) wire('ariaValueText', text);
}

// Accessibility roles and states any element may declare; controls keep their own role.
// Composite items take focus and activate, so they are buttons; their containers are views.
const itemRoles = new Set(['tab', 'radio', 'option', 'menuitem', 'menuitemcheckbox', 'menuitemradio', 'treeitem']);
const containerRoles = new Set(['tablist', 'tabpanel', 'radiogroup', 'toolbar', 'tree', 'row']);
const checkedRoles = new Set(['radio', 'menuitemcheckbox', 'menuitemradio']);
function semantics(tag, props, wire) {
  const { role, headingLevel, ariaLevel, accessibilityHidden, accessibilityLiveRegion } = props;
  if (role !== undefined) {
    if (role === 'dialog') throw Error('UI_PROP: role="dialog" is the <dialog> element');
    if (role === 'alertdialog' && tag !== 'dialog') throw Error('UI_PROP: role="alertdialog" goes on a <dialog>');
    if (!fieldOptions.role.includes(role)) throw Error(`UI_PROP: role must be one of ${fieldOptions.role.join(', ')}`);
    if (itemRoles.has(role) && role !== 'option' && tag !== 'button') throw Error(`UI_PROP: role="${role}" belongs on a <button>`);
    if (containerRoles.has(role) && tag !== 'view') throw Error(`UI_PROP: role="${role}" belongs on a <view>`);
    if (role === 'img' && (!['view', 'text'].includes(tag) || typeof props.accessibilityLabel !== 'string' || !props.accessibilityLabel.trim()))
      throw Error('UI_PROP: role="img" goes on a <view> or <text> with an aria-label, which is spoken in place of its content');
    // A <dialog> wires its own role.
    if (tag !== 'dialog') wire('role', role);
  }
  progressValues(role, props, wire, tag);
  if (headingLevel !== undefined) {
    if (role !== 'heading' || !Number.isInteger(headingLevel) || headingLevel < 1 || headingLevel > 6)
      throw Error('UI_PROP: headingLevel is an integer 1..6 on role="heading"');
    wire('headingLevel', headingLevel);
  }
  if (ariaLevel !== undefined) {
    if (role !== 'treeitem' || !Number.isInteger(ariaLevel) || ariaLevel < 1 || ariaLevel > 32)
      throw Error('UI_PROP: aria-level is an integer 1..32 on role="treeitem"');
    wire('headingLevel', ariaLevel);
  }
  if (accessibilityHidden !== undefined) {
    if (typeof accessibilityHidden !== 'boolean') throw Error('UI_PROP: aria-hidden must be boolean');
    if (accessibilityHidden) wire('accessibilityHidden', 1);
  }
  if (accessibilityLiveRegion !== undefined) {
    if (!fieldOptions.accessibilityLiveRegion.includes(accessibilityLiveRegion)) throw Error('UI_PROP: aria-live must be off, polite or assertive');
    wire('accessibilityLiveRegion', accessibilityLiveRegion);
  }
  const { accessibilityState: state } = props;
  if (state !== undefined) {
    if (!state || typeof state !== 'object' || Array.isArray(state)) throw Error('UI_PROP: accessibilityState must be an object');
    for (const [name, value] of Object.entries(state)) {
      if (!has(semanticStates, name)) throw Error(`UI_PROP: accessibilityState.${name}; known: ${Object.keys(semanticStates).join(', ')}`);
      if (value === undefined) continue;
      if (typeof value !== 'boolean') throw Error(`UI_PROP: aria-${name} must be boolean`);
      if (name === 'checked' && !checkedRoles.has(role)) throw Error('UI_PROP: aria-checked goes on role="radio", "menuitemcheckbox" or "menuitemradio"; a checkbox has its own checked');
      wire(semanticStates[name], +value);
    }
  }
  // Containers and text take a label here; controls wire their own.
  if (['view', 'page', 'text', 'dialog'].includes(tag) && props.accessibilityLabel !== undefined) {
    check.label(props.accessibilityLabel, 'UI_PROP');
    wire('accessibilityLabel', props.accessibilityLabel);
  }
}

// `drag="x|y"`: the host moves the element with the finger between its snap points (offsets
// from where layout put it, px or % of its own size) and springs it to one on release.
const dragPoint = /^-?\d+(\.\d+)?%$/;
function dragElement(tag, props, wire, on, release) {
  const { drag: axis, dragSnapPoints: points = [0], bind: cell, onDragStart, onDragEnd } = props;
  if (axis !== 'x' && axis !== 'y') throw Error('UI_PROP: drag is "x" or "y"');
  if (!Array.isArray(points) || !points.length || points.length > limits.dragSnapPoints ||
      points.some(point => typeof point === 'number' ? !Number.isFinite(point) || Math.abs(point) > 16384
        : typeof point !== 'string' || !dragPoint.test(point) || Math.abs(parseFloat(point)) > 1000))
    throw Error(`UI_PROP: dragSnapPoints are 1..${limits.dragSnapPoints} offsets in px or %`);
  if (tag !== 'dialog' && tag !== 'button' && !props.accessibilityLabel)
    throw Error('UI_A11Y: an element with drag needs aria-label, so assistive tech can move it');
  for (const [name, value] of [['onDragStart', onDragStart], ['onDragEnd', onDragEnd]])
    if (value !== undefined && typeof value !== 'function') throw Error(`UI_PROP: ${name}`);
  let index = 0;
  if (cell !== undefined) {
    const bound = cellOf(cell);
    if (!bound || render.candidate.locals.get(bound.key) !== bound.cell || typeof cell.value !== 'number')
      throw Error('UI_PROP: bind on a drag element takes a mounted number useLocal cell');
    index = cell.value;
  }
  if (!Number.isInteger(index) || index < 0 || index >= points.length) throw Error('UI_PROP: the bound snap index is out of range');
  wire('drag', JSON.stringify({ axis, points, index, ...(release ? { release } : {}) }));
  const close = props[dragClose];
  on('onDragEvent', payload => {
    const event = JSON.parse(payload);
    if (event.phase === 'start') onDragStart?.();
    else if (event.phase === 'end') onDragEnd?.({ index: event.index, velocity: event.velocity });
    else if (event.phase === 'settle' && Number.isInteger(event.index) && event.index >= 0 && event.index < points.length) {
      if (cell !== undefined && cell.value !== event.index) cell.set(event.index);
      // A dialog that came to rest wholly off screen is dismissed.
      if (event.hidden === true) close?.();
    }
  }, 'text');
}

// A select's options, validated like a radio group's and sent whole.
function selectOptions(props) {
  if (!Array.isArray(props.options) || !props.options.length || props.options.length > 64) throw Error('UI_SELECT: expected 1..64 options');
  const values = new Set();
  return props.options.map(option => {
    if (!option || typeof option !== 'object' || Object.keys(option).some(k => !['value', 'label', 'disabled', 'data-testid'].includes(k)) ||
        typeof option.value !== 'string' || !option.value.length || option.value.length > 256 || values.has(option.value) ||
        typeof option.label !== 'string' || !validInputText(option.label) || (option.disabled !== undefined && typeof option.disabled !== 'boolean'))
      throw Error('UI_SELECT: invalid or duplicate option');
    values.add(option.value);
    return option.disabled ? { value: option.value, label: option.label, disabled: true } : { value: option.value, label: option.label };
  });
}

function radioOptions(props, identity, fields, disabled) {
  if (!Array.isArray(props.options) || !props.options.length || props.options.length > 64) throw Error('UI_RADIO: expected 1..64 options');
  const values = new Set();
  const style = Object.fromEntries(Object.entries(fields).filter(([name]) => optionStyle.includes(name)));
  return props.options.map(option => {
    if (!option || typeof option !== 'object' || Object.keys(option).some(k => !['value', 'label', 'disabled', 'data-testid'].includes(k)) ||
        typeof option.value !== 'string' || !option.value.length || option.value.length > 256 || values.has(option.value) ||
        typeof option.label !== 'string' || (option.disabled !== undefined && typeof option.disabled !== 'boolean') ||
        (option['data-testid'] !== undefined && typeof option['data-testid'] !== 'string')) throw Error('UI_RADIO: invalid or duplicate option');
    values.add(option.value);
    const { value } = option, selected = props.value === value, enabled = !disabled && option.disabled !== true;
    const child = { tag: 'radio-option', element: 'radio-option', identity: `${identity}/radio/key:${JSON.stringify(value)}`,
      props: { key: value, testId: option['data-testid'] }, fields: style, states: {}, text: option.label,
      wire: [...(enabled ? [] : [['enabled', 0]]), ['checked', +selected]], handlers: [], children: [] };
    if (enabled && props.onChange) child.handlers.push(['onCheckedChange', handle(), callbackOf(() => { if (!selected) return props.onChange(value); })]);
    return child;
  });
}

/** The committed list's pure rows a full render may reuse, when nothing they were built from changed:
 *  over new items (a patched data array), a row whose frozen item is the same object stands. */
function reusableRows(old, props, rowEnv, direction) {
  const source = props.row?.__lxRows, before = old?.rowSource;
  if (!source || !before || before.site !== source.site || old.count !== props.count ||
      old.rowEnv !== rowEnv || old.direction !== direction || !old.pure?.size) return null;
  const same = before.items === source.items;
  const rows = new Map();
  for (const i of old.pure) {
    if (!old.cache.has(i)) continue;
    const item = source.items[i];
    if (same || (item === before.items[i] && (item === null || typeof item !== 'object' || Object.isFrozen(item)))) rows.set(i, old.cache.get(i));
  }
  return rows.size ? { rows, typography: old.typography } : null;
}

// Rows render after resolution, so the list keeps the direction and providers they sit under.
function listState(props, identity, direction, contexts) {
  const old = page.committed?.lists.get(identity);
  const rowSource = props.row?.__lxRows, rowEnv = rowSource ? JSON.stringify(envOf(render.view) ?? null) : undefined;
  const anchor = props.anchor === undefined ? null : JSON.stringify(props.anchor);
  // Row state is remapped by key only when there is some; a fresh list needs no positions.
  const positions = props.itemKeys && old?.states?.size ? keyPositions(props.itemKeys) : null;
  const states = positions
    ? new Map([...(old?.states ?? [])].flatMap(([, state]) => positions.has(state.key) ? [[positions.get(state.key), state]] : []))
    : new Map([...(old?.states ?? [])].filter(([i]) => i < props.count));
  const anchorChanged = Boolean(old && old.count === props.count && anchor !== null && anchor !== old.anchor);
  if (anchorChanged && !props.anchor.duration) render.candidate?.movedAnchors?.set(identity, anchor);
  return { count: props.count, previousCount: old?.count ?? props.count, anchor, anchorChanged,
    anchorCache: old?.cache ?? null,
    states, cache: new Map(), pure: new Set(), reusable: reusableRows(old, props, rowEnv, direction), rowSource, rowEnv, direction, contexts,
    window: old?.window ? { ...old.window, start: Math.min(old.window.start, props.count), end: Math.min(old.window.end, props.count) } : null };
}

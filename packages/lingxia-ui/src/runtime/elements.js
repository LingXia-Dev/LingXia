import { elements, admission, limits, commonProps } from '../generated/contract.js';
import { resolveClasses } from '../style/classes.js';
import { lowerStyle, paintOnly, mergeStyle } from '../style/lower.js';
import { h, readNode, isNode, isFragment, entries } from './nodes.js';
import { render, page } from './state.js';
import { cellOf, envOf } from './hooks.js';
import { located } from './errors.js';
import { has, validInputText } from './data.js';
import { fieldOptions } from '../generated/wire.js';
import * as check from './validate.js';
import { defaultSeries } from '../style/color.js';

const common = new Set(commonProps);
const semanticStates = { pressed: 'statePressed', selected: 'stateSelected', expanded: 'stateExpanded', busy: 'stateBusy' };
const accepts = (field, tag) => admission.fields[field].includes(elements[tag].wire);
// Elements that take input of their own; they may sit inside a pressable control.
const interactiveTags = new Set(['button', 'link', 'checkbox', 'switch', 'radio', 'input', 'textarea', 'select', 'slider']);
const bindCodes = { input: 'UI_INPUT', textarea: 'UI_INPUT', checkbox: 'UI_CHECK', switch: 'UI_CHECK', slider: 'UI_SLIDER', radio: 'UI_RADIO' };
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
const callbackOf = (callback, kind = 'void', isolated = false) => ({ callback, kind, isolated });

const dataProp = /^data-[a-z][a-z0-9-]*$/;

/** The element's data-* props as `data-*` variants read them: present unless false or nullish. */
function dataOf(props) {
  let data;
  for (const [key, value] of Object.entries(props)) {
    if (!dataProp.test(key) || value === false || value == null) continue;
    (data ??= {})[key.slice(5)] = value === true ? '' : String(value);
  }
  return data;
}

function admit(tag, element, props) {
  let data = 0;
  for (const [key, value] of Object.entries(props)) {
    if (common.has(key) || has(element.props, key)) continue;
    if (dataProp.test(key)) {
      if (!['string', 'number', 'boolean'].includes(typeof value) && value != null) throw Error(`UI_PROP: ${key} must be a string, number or boolean`);
      if (typeof value === 'string' && value.length > 256) throw Error(`UI_PROP: ${key} exceeds 256 characters`);
      if (++data > 16) throw Error('UI_PROP: more than 16 data-* props');
      continue;
    }
    if (element.binding && (key === 'bind' || key === 'value' || key === element.binding.event)) continue;
    throw Error(`UI_PROP: ${key} is not a property of <${tag}>`);
  }
  for (const key of ['key', 'data-testid']) if (props[key] !== undefined && typeof props[key] !== 'string') throw Error(`UI_PROP: ${key} must be a string`);
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
let loweredEnv = null;
function envKey(env) {
  return JSON.stringify([env?.theme, env?.viewport, env?.pointer, env?.reducedMotion, env?.insets, env?.keyboard]);
}

function nodeStyle(tag, props, extra, inherited, direction, structure) {
  if (props.style !== undefined || extra !== undefined || props.className === undefined) return lowerNodeStyle(tag, props, extra, inherited, direction, structure);
  const environment = envKey(envOf(render.view));
  if (environment !== loweredEnv) { lowered.clear(); loweredEnv = environment; }
  const data = dataOf(props);
  const key = `${tag}\u0000${props.className}\u0000${data ? JSON.stringify(data) : ''}\u0000${props.disabled === true}\u0000${direction}\u0000${(structure ?? []).join(",")}\u0000${inherited.fontSize}\u0000${inherited.lineHeight}`;
  let style = lowered.get(key);
  if (!style) {
    style = lowerNodeStyle(tag, props, extra, inherited, direction, structure);
    if (lowered.size >= 2048) lowered.delete(lowered.keys().next().value);
    lowered.set(key, style);
  }
  // Callers may edit the fields of their node (an image's fit moves to its own field).
  return { ...style, fields: { ...style.fields } };
}

const fieldTags = new Set(['input', 'textarea', 'select']);
const runTags = new Set(['span', 'link']);
const disabledField = { color: 'text-muted', backgroundColor: 'surface-sunken', borderColor: 'line' };

// Classes, then the parent's child rule, then inline style; `disabled:` replaces the class cascade.
function lowerNodeStyle(tag, props, extra, inherited, direction, structure) {
  const env = { ...envOf(render.view), direction, structure, data: dataOf(props) };
  const classes = resolveClasses(props.className, env);
  // A run takes the size of `text-sm` but not the line height paired with it.
  if (classes?.pairedLeading && runTags.has(tag)) delete classes.base.lineHeight;
  const disabled = props.disabled === true;
  // A disabled editor without `disabled:` classes still reads as disabled: muted roles over its classes.
  const muted = disabled && !classes?.disabled && fieldTags.has(tag) ? disabledField : undefined;
  const author = mergeStyle(direction, extra, disabled && classes?.disabled ? classes.disabled : classes?.base, muted, props.style);
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
  if (!disabled) {
    state('active', 'pressedStyle', accepts('pressedStyle', tag), 'a control');
    state('hover', 'hoverStyle', true);
  }
  // Presence: the paint an element enters from and leaves toward, with its own transition.
  state('starting', 'startingStyle', true);
  state('exit', 'exitStyle', true);
  state('focusWithin', 'focusWithinStyle', true);
  state('groupActive', 'groupPressedStyle', true);
  // `has-checked:`/`has-disabled:` depend on descendants, known once the children resolve.
  const descendants = {};
  for (const [name, kind] of [['hasChecked', 'checked'], ['hasDisabled', 'disabled']]) {
    const delta = classes?.[name];
    if (!delta || !Object.keys(delta).length) continue;
    paintOnly(name.replace(/^has/, 'has-').toLowerCase(), tag, delta);
    descendants[kind] = lowerStyle(tag, delta, env, { fontSize: typography.fontSize }).fields;
  }
  return { fields, states, typography, author, children: classes?.children, descendants };
}

export function resolveNode(node, identity, depth = 0, parent = {}) {
  if (depth > limits.depth) throw Error('UI_LIMIT: children too deep');
  if (node == null || typeof node === 'boolean' || node === '') return null;
  if (typeof node === 'string' || typeof node === 'number') node = h('text', {}, node);
  // A component's fragment (or array): its parent lays the children out in its place.
  if (Array.isArray(node)) return { fragment: entries(node) };
  if (isFragment(node)) return { fragment: entries([node]).map(entry => entry.slot ? { ...entry, slot: entry.slot.slice(2) } : entry) };
  if (!isNode(node)) throw Error('UI_ROOT: expected one node');
  const [tag, original, children, location] = readNode(node);
  // Where a render that never returns was, for UI_TIMEOUT.
  if (location) render.site = `${typeof tag === 'function' ? tag.name || '<anonymous>' : `<${tag}>`} (${location})`;
  if (typeof tag === 'function') {
    const previous = render.scope;
    render.scope = `${identity}/component:${componentId(tag)}`;
    const supplied = children.length === 0 ? undefined : children.length === 1 ? children[0] : children;
    try { return resolveNode(tag({ ...original, children: supplied }), render.scope, depth + 1, parent); }
    catch (error) { throw located(error, `${tag.name || '<anonymous>'}${location ? ` (${location})` : ''}`); }
    finally { render.scope = previous; }
  }
  try { return resolveElement(tag, original, children, identity, depth, parent); }
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
const ariaNames = { 'aria-label': 'accessibilityLabel', 'aria-hidden': 'accessibilityHidden', 'aria-live': 'accessibilityLiveRegion' };
function ariaProps(props) {
  for (const [authored, internal] of Object.entries(ariaNames))
    if (authored in props) { props[internal] = props[authored]; delete props[authored]; }
  let state;
  for (const name of ['pressed', 'selected', 'expanded', 'busy']) {
    if (props[`aria-${name}`] === undefined) continue;
    (state ??= {})[name] = props[`aria-${name}`];
    delete props[`aria-${name}`];
  }
  if (state) props.accessibilityState = state;
}

function resolveElement(tag, original, children, identity, depth, parent) {
  if (typeof tag !== 'string' || !has(elements, tag)) throw Error(`UI_TAG: ${String(tag)}`);
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
  const authored = entries(children), flat = authored.map(entry => entry.child), slot = authored.map(entry => entry.slot);
  const style = nodeStyle(tag, props, parent.extra, parent.typography ?? { fontSize: 14 }, direction, structure(parent.sibling, flat));
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
  const wire = (key, value) => result.wire.push([key, value]);
  const on = (key, callback, kind, isolated) => result.handlers.push([key, handle(), callbackOf(callback, kind, isolated)]);
  if (disabled) wire('enabled', 0);
  // Authored data-* props reach automation (getAttribute, [data-x] selectors), never semantics.
  const data = dataOf(props);
  if (data) wire('dataAttributes', JSON.stringify(data));
  if (props.dir !== undefined) wire('direction', props.dir);
  semantics(tag, props, wire);
  switch (tag) {
    case 'link':
      check.label(props.accessibilityLabel, 'UI_TEXT');
      if (props.onClick !== undefined && typeof props.onClick !== 'function') throw Error('UI_PROP: onClick');
      if (props.accessibilityLabel !== undefined) wire('accessibilityLabel', props.accessibilityLabel);
      if (props.onClick && !disabled) on('onClick', props.onClick);
      break;
    case 'button':
      check.label(props.accessibilityLabel, 'UI_BUTTON');
      if (props.onClick !== undefined && typeof props.onClick !== 'function') throw Error('UI_PROP: onClick');
      if (props.accessibilityLabel !== undefined) wire('accessibilityLabel', props.accessibilityLabel);
      if (props.onClick && !disabled) on('onClick', props.onClick);
      break;
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
      if (props.onChange && !disabled) on('onSliderChange', value => {
        const next = check.sliderValue(props, value);
        if (next !== props.value) props.onChange(next);
      }, 'number');
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
    result.list = listState(props, identity);
  } else if (runs) {
    // Nested spans flatten into one run list; inner runs layer over the outer span's style.
    const extra = tag === 'span' || tag === 'link' ? style.author : undefined;
    result.children = flat.flatMap((child, position) => {
      const at = authored[position], part = at.slot === undefined ? `key:${JSON.stringify(at.key)}` : `position:${at.slot}`;
      const run = resolveNode(inline(child) ? h('span', {}, child) : child, `${identity}/${tag}/${part}`, depth + 1, {
        tag, typography: style.typography, direction, extra, inText: true, inControl: parent.inControl, inLink: parent.inLink || tag === 'link',
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
    const keys = new Set(), last = flat.length - 1;
    const place = ({ child, slot: at, key }, position, nesting) => {
      if (nesting > 128) throw Error('UI_LIMIT: children too deep');
      if (key !== undefined && typeof key !== 'string') throw Error('UI_KEY: expected string');
      if (key !== undefined && keys.has(key)) throw Error('UI_KEY: duplicate sibling');
      if (key !== undefined) keys.add(key);
      const part = key === undefined ? `position:${at}` : `key:${JSON.stringify(key)}`;
      const node = resolveNode(child, `${identity}/${tag}/${part}`, depth + 1, {
        tag, typography: style.typography, direction, extra: position < last ? style.children : undefined,
        sibling: { index: position, count: flat.length },
        inControl: parent.inControl || admission.tags[element.wire].children === 'content',
      });
      // A component's fragment opens in its place, named under the component's slot or key.
      if (node?.fragment) return node.fragment.flatMap(inner => place(key === undefined
        ? { ...inner, slot: inner.slot === undefined ? undefined : `${at}.${inner.slot}`, key: inner.key }
        : { child: inner.child, key: JSON.stringify([key, inner.key ?? `#${inner.slot}`]) }, position, nesting + 1));
      if (node === null) return [];
      if (key === undefined) return [{ ...node, slot: at }];
      // Inside a keyed fragment the wire key is the composed one native names the node by.
      return [node.props.key === key ? node : { ...node, props: { ...node.props, key } }];
    };
    result.children = authored.flatMap((entry, position) => place(entry, position, 0));
    // A pressable control around nothing but another control adds a second target for one action.
    if ((tag === 'button' || tag === 'checkbox') && result.children.length === 1 && interactiveTags.has(result.children[0].tag))
      throw Error(`UI_BUTTON: <${tag}> holds only a <${result.children[0].tag}>; put the action on that control`);
    // Native names an unkeyed child by its rendered position; where a hole moved it, send the slot.
    result.children = result.children.map((child, ordinal) => {
      if (child.slot === undefined) return child;
      const { slot: at, ...rest } = child;
      return at === String(ordinal) ? rest : { ...rest, slot: at };
    });
  }
  for (const [kind, fields] of Object.entries(style.descendants))
    if (result.children.some(child => holds(child, kind))) Object.assign(result.fields, fields);
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

// Accessibility roles and states any element may declare; controls keep their own role.
function semantics(tag, props, wire) {
  const { role, headingLevel, accessibilityHidden, accessibilityLiveRegion } = props;
  if (role !== undefined) {
    if (!fieldOptions.role.includes(role)) throw Error(`UI_PROP: role must be one of ${fieldOptions.role.join(', ')}`);
    wire('role', role);
  }
  if (headingLevel !== undefined) {
    if (role !== 'heading' || !Number.isInteger(headingLevel) || headingLevel < 1 || headingLevel > 6)
      throw Error('UI_PROP: headingLevel is an integer 1..6 on role="heading"');
    wire('headingLevel', headingLevel);
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
      wire(semanticStates[name], +value);
    }
  }
  // Containers and text take a label here; controls wire their own.
  if (['view', 'column', 'row', 'page', 'text'].includes(tag) && props.accessibilityLabel !== undefined) {
    check.label(props.accessibilityLabel, 'UI_PROP');
    wire('accessibilityLabel', props.accessibilityLabel);
  }
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

function listState(props, identity) {
  const old = page.committed?.lists.get(identity);
  const anchor = props.anchor === undefined ? null : JSON.stringify(props.anchor);
  const positions = props.itemKeys && new Map(props.itemKeys.map((key, i) => [`key:${JSON.stringify(key)}`, i]));
  const states = positions
    ? new Map([...(old?.states ?? [])].flatMap(([, state]) => positions.has(state.key) ? [[positions.get(state.key), state]] : []))
    : new Map([...(old?.states ?? [])].filter(([i]) => i < props.count));
  return { count: props.count, previousCount: old?.count ?? props.count, anchor,
    anchorChanged: Boolean(old && old.count === props.count && anchor !== null && anchor !== old.anchor),
    anchorCache: old?.cache ?? null,
    states, cache: new Map(), window: old?.window ? { ...old.window, start: Math.min(old.window.start, props.count), end: Math.min(old.window.end, props.count) } : null };
}

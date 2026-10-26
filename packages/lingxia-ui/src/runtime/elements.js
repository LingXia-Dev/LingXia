import { elements, admission, limits } from '../generated/contract.js';
import { resolveClasses } from '../style/classes.js';
import { lowerStyle, paintOnly, mergeStyle } from '../style/lower.js';
import { h, readNode, isNode, nodeKey, flatten } from './nodes.js';
import { render, page } from './state.js';
import { cellOf } from './hooks.js';
import { located } from './errors.js';
import { has, validInputText } from './data.js';
import { fieldOptions } from '../generated/wire.js';
import * as check from './validate.js';

const common = new Set(['key', 'testId', 'style', 'className', 'children', 'dir', 'role', 'headingLevel', 'accessibilityHidden', 'accessibilityLiveRegion']);
const accepts = (field, tag) => admission.fields[field].includes(elements[tag].wire);
const bindCodes = { input: 'UI_INPUT', textarea: 'UI_INPUT', search: 'UI_INPUT', checkbox: 'UI_CHECK', switch: 'UI_CHECK', slider: 'UI_SLIDER', radio: 'UI_RADIO' };
const optionStyle = ['fontSize', 'fontWeight', 'fontFamily', 'lineHeight', 'color', 'background'];
const componentIds = new WeakMap();
let nextComponent = 1, nextHandle = 1;

const handle = () => nextHandle++;
const callbackOf = (callback, kind = 'void', isolated = false) => ({ callback, kind, isolated });

function admit(tag, element, props) {
  for (const key of Object.keys(props)) {
    if (common.has(key) || has(element.props, key)) continue;
    if (element.binding && (key === 'bind' || key === 'value' || key === element.binding.event)) continue;
    throw Error(`UI_PROP: ${key} is not a property of <${tag}>`);
  }
  for (const key of ['key', 'testId']) if (props[key] !== undefined && typeof props[key] !== 'string') throw Error(`UI_PROP: ${key}`);
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

// Classes, then the parent's child rule, then inline style; `disabled:` replaces the class cascade.
function nodeStyle(tag, props, extra, inherited, direction, structure) {
  const env = { ...render.view?.env, direction, structure };
  const classes = resolveClasses(props.className, env);
  const disabled = props.disabled === true;
  const author = mergeStyle(direction, extra, disabled && classes?.disabled ? classes.disabled : classes?.base, props.style);
  const { fields, typography } = lowerStyle(tag, author, env, inherited);
  const states = {};
  const state = (name, wire, allowed, requirement) => {
    const delta = classes?.[name];
    if (!delta || !Object.keys(delta).length) return;
    if (!allowed) throw Error(`UI_STYLE: ${name}: requires ${requirement}, not <${tag}>`);
    paintOnly(name, tag, delta);
    states[wire] = lowerStyle(tag, delta, env, { fontSize: typography.fontSize }).fields;
  };
  state('focus', 'focusStyle', accepts('focusStyle', tag), 'an editor');
  if (!disabled) {
    state('active', 'pressedStyle', accepts('pressedStyle', tag), 'a control');
    state('hover', 'hoverStyle', true);
  }
  return { fields, states, typography, author, children: classes?.children };
}

export function resolveNode(node, identity, depth = 0, parent = {}) {
  if (depth > limits.depth) throw Error('UI_LIMIT: children too deep');
  if (node == null || typeof node === 'boolean' || node === '') return null;
  if (typeof node === 'string' || typeof node === 'number') node = h('text', {}, node);
  if (!isNode(node)) throw Error('UI_ROOT: expected one node');
  const [tag, original, children, location] = readNode(node);
  if (typeof tag === 'function') {
    if (!componentIds.has(tag)) componentIds.set(tag, nextComponent++);
    const previous = render.scope;
    render.scope = `${identity}/component:${componentIds.get(tag)}`;
    const supplied = children.length === 0 ? undefined : children.length === 1 ? children[0] : children;
    try { return resolveNode(tag({ ...original, children: supplied }), render.scope, depth + 1, parent); }
    catch (error) { throw located(error, `${tag.name || '<anonymous>'}${location ? ` (${location})` : ''}`); }
    finally { render.scope = previous; }
  }
  try { return resolveElement(tag, original, children, identity, depth, parent); }
  catch (error) { throw located(error, `<${tag}>${location ? ` (${location})` : ''}`); }
}

function resolveElement(tag, original, children, identity, depth, parent) {
  if (typeof tag !== 'string' || !has(elements, tag)) throw Error(`UI_TAG: ${String(tag)}`);
  const element = elements[tag];
  const props = { ...original };
  admit(tag, element, props);
  if (parent.inControl && !admission.tags[element.wire].content) throw Error('UI_BUTTON: descendants must be noninteractive content');
  if (element.binding) bind(tag, element, props);
  const editor = ['input', 'textarea', 'search'].includes(tag);
  if (props.dir !== undefined && !(['ltr', 'rtl'].includes(props.dir) || (editor && props.dir === 'auto')))
    throw Error(`UI_PROP: dir must be "ltr" or "rtl"${editor ? ' or "auto"' : ''}`);
  // `dir` overrides the direction inherited from the environment for its subtree; `auto` keeps the box's.
  const direction = (props.dir === 'auto' ? undefined : props.dir) ?? parent.direction ?? (render.view?.env?.direction === 'rtl' ? 'rtl' : 'ltr');
  const flat = flatten(children);
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
  const result = { tag: element.wire, element: tag, props, fields: style.fields, states: style.states, typography: style.typography,
    identity, text: leaf ? flat.join('') : '', wire: [], handlers: [], children: [] };
  const wire = (key, value) => result.wire.push([key, value]);
  const on = (key, callback, kind, isolated) => result.handlers.push([key, handle(), callbackOf(callback, kind, isolated)]);
  if (disabled) wire('enabled', 0);
  if (props.dir !== undefined) wire('direction', props.dir);
  semantics(tag, props, wire);
  switch (tag) {
    case 'link':
      check.label(props.accessibilityLabel, 'UI_TEXT');
      if (props.onPress !== undefined && typeof props.onPress !== 'function') throw Error('UI_PROP: onPress');
      if (props.accessibilityLabel !== undefined) wire('accessibilityLabel', props.accessibilityLabel);
      if (props.onPress && !disabled) on('onClick', props.onPress);
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
    case 'input': case 'textarea': case 'search':
      check.label(props.accessibilityLabel, 'UI_INPUT');
      if (!validInputText(props.value === undefined ? '' : props.value) || !validInputText(props.placeholder === undefined ? '' : props.placeholder)) throw Error('UI_INPUT: invalid text');
      for (const name of ['secure', 'autoFocus', 'submitOnEnter']) if (props[name] !== undefined && typeof props[name] !== 'boolean') throw Error(`UI_INPUT: ${name}`);
      const attributes = check.editor(tag, props);
      if (props.onSubmit !== undefined && typeof props.onSubmit !== 'function') throw Error('UI_INPUT: onSubmit');
      if (props.accessibilityLabel !== undefined) wire('accessibilityLabel', props.accessibilityLabel);
      wire('inputValue', props.value ?? '');
      if (props.placeholder !== undefined) wire('placeholder', props.placeholder);
      if (tag === 'search') wire('keyboard', 'search');
      else if (props.keyboard !== undefined) wire('keyboard', props.keyboard);
      for (const name of ['secure', 'autoFocus', 'submitOnEnter']) if (props[name] !== undefined) wire(name, +props[name]);
      for (const [key, value] of attributes) wire(key, value);
      if (!disabled) for (const name of ['onInput', 'onSubmit']) if (props[name]) on(name, props[name], 'text');
      break;
    case 'image':
      check.image(props);
      wire('imageSrc', props.src);
      if (props.alt !== undefined) wire('imageAlt', props.alt);
      if (props.retryKey !== undefined) wire('imageRetryKey', props.retryKey);
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
      wire('plotColors', JSON.stringify(props.colors)); wire('accessibilityLabel', props.accessibilityLabel);
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
      const run = resolveNode(inline(child) ? h('span', {}, child) : child, `${identity}/${tag}/position:${position}`, depth + 1, {
        tag, typography: style.typography, direction, extra, inText: true, inControl: parent.inControl, inLink: parent.inLink || tag === 'link',
      });
      if (run === null) return [];
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
    result.children = flat.map((child, position) => {
      const key = nodeKey(child);
      if (key !== undefined && typeof key !== 'string') throw Error('UI_KEY: expected string');
      if (key !== undefined && keys.has(key)) throw Error('UI_KEY: duplicate sibling');
      if (key !== undefined) keys.add(key);
      const part = key === undefined ? `position:${position}` : `key:${JSON.stringify(key)}`;
      return resolveNode(child, `${identity}/${tag}/${part}`, depth + 1, {
        tag, typography: style.typography, direction, extra: position < last ? style.children : undefined,
        sibling: { index: position, count: flat.length },
        inControl: parent.inControl || admission.tags[element.wire].children === 'content',
      });
    }).filter(child => child !== null);
  }
  return result;
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
    if (typeof accessibilityHidden !== 'boolean') throw Error('UI_PROP: accessibilityHidden must be boolean');
    if (accessibilityHidden) wire('accessibilityHidden', 1);
  }
  if (accessibilityLiveRegion !== undefined) {
    if (!fieldOptions.accessibilityLiveRegion.includes(accessibilityLiveRegion)) throw Error('UI_PROP: accessibilityLiveRegion must be off, polite or assertive');
    wire('accessibilityLiveRegion', accessibilityLiveRegion);
  }
  // Containers and text take a label here; controls wire their own.
  if (['view', 'column', 'row', 'page', 'text'].includes(tag) && props.accessibilityLabel !== undefined) {
    check.label(props.accessibilityLabel, 'UI_PROP');
    wire('accessibilityLabel', props.accessibilityLabel);
  }
}

function radioOptions(props, identity, fields, disabled) {
  if (!Array.isArray(props.options) || !props.options.length || props.options.length > 64) throw Error('UI_RADIO: expected 1..64 options');
  const values = new Set();
  const style = Object.fromEntries(Object.entries(fields).filter(([name]) => optionStyle.includes(name)));
  return props.options.map(option => {
    if (!option || typeof option !== 'object' || Object.keys(option).some(k => !['value', 'label', 'disabled', 'testId'].includes(k)) ||
        typeof option.value !== 'string' || !option.value.length || option.value.length > 256 || values.has(option.value) ||
        typeof option.label !== 'string' || (option.disabled !== undefined && typeof option.disabled !== 'boolean') ||
        (option.testId !== undefined && typeof option.testId !== 'string')) throw Error('UI_RADIO: invalid or duplicate option');
    values.add(option.value);
    const { value } = option, selected = props.value === value, enabled = !disabled && option.disabled !== true;
    const child = { tag: 'radio-option', element: 'radio-option', identity: `${identity}/radio/key:${JSON.stringify(value)}`,
      props: { key: value, testId: option.testId }, fields: style, states: {}, text: option.label,
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

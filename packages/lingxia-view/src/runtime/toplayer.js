// HTML's top layer: an open <dialog> or popover lays out against the whole page and paints above
// it in open order, wherever it sits in the tree. Native routes `topLayer` boxes to the root.
import { h } from './nodes.js';
import { render, page } from './state.js';
import { envOf } from './hooks.js';
import { resolveClasses } from '../style/classes.js';
import { limits } from '../generated/contract.js';
import { field as fieldIds } from '../generated/wire.js';
import { report } from './actions.js';

/** Marks the panel a <dialog> or popover expands to, so resolution does not expand it again. */
export const panelMark = Symbol('top-layer panel');
/** A dialog's close, for its panel to call when dragged off screen. */
export const dragClose = Symbol('drag close');
/** Marks a backdrop that fades with how much of its dragged dialog is on screen. */
export const scrimMark = Symbol('drag scrim');
/** The popover ids a node sits inside, innermost last. */
export const popoverScope = Symbol('popover scope');

const popoverTags = new Set(['view']);
const closers = new Set(['any', 'closerequest', 'none']);
// Below Logic's feedback and the pickers, which the View draws over everything.
const layerZ = 9000;

let uncontrolled = new Set(), anchors = new Map(), opened = new Map(), refused = new Set();
let invoker = null, sequence = 0;

export function resetTopLayer() {
  uncontrolled = new Set(); anchors = new Map(); opened = new Map(); refused = new Set();
  invoker = null; sequence = 0;
}

function collected() {
  return render.candidate.topLayer ??= { entries: [], popovers: new Map(), invokers: [], interests: [] };
}

// An open overlay takes a place in the layer, or is refused past the budget and stays closed.
function admitted(identity) {
  const top = collected();
  if (top.entries.length >= limits.topLayer) {
    if (!refused.has(identity)) { refused.add(identity); report(Error(`UI_BUDGET: more than ${limits.topLayer} dialogs and popovers open; ${identity} stays closed`)); }
    return false;
  }
  refused.delete(identity);
  if (!opened.has(identity)) opened.set(identity, ++sequence);
  return true;
}

// HTML's default dialog and popover box: centred by auto margins the author's classes override.
function placement(props, env) {
  const set = { ...resolveClasses(props.className, env)?.base, ...props.style };
  const margins = {};
  for (const side of ['Top', 'Bottom', 'Left', 'Right']) {
    const inline = side === 'Left' || side === 'Right';
    if (set[`margin${side}`] === undefined && !(inline && (set.marginInlineStart !== undefined || set.marginInlineEnd !== undefined)))
      margins[`margin${side}`] = 'auto';
  }
  return margins;
}

function layer(identity, env, ...children) {
  return layerWith({}, identity, env, ...children);
}

function layerWith(style, identity, env, ...children) {
  return h('view', { style: { position: 'absolute', top: 0, left: 0, right: 0, bottom: env?.keyboard?.height ?? 0, zIndex: layerZ + opened.get(identity) % 1000, ...style } }, ...children);
}

// A popover placed by its classes lays out in the part of the surface the bars leave clear; its
// backdrop, absolutely placed, still covers the whole layer.
function clearOfBars(env) {
  const insets = env?.insets;
  if (!insets) return {};
  return { paddingTop: insets.top, paddingRight: insets.right, paddingLeft: insets.left,
    paddingBottom: Math.max(0, insets.bottom - (env.keyboard?.height ?? 0)) };
}

const backdropFade = { opacity: 1, transition: { properties: ['opacity'], durationMs: 200, delayMs: 0, easing: [0, 0, 0.58, 1] } };

/**
 * `<dialog>` and popovers expand to a top-layer box holding a backdrop and the panel; `null`
 * when closed, `undefined` for any other node. `finish` decorates the resolved layer.
 */
const popoverProps = ['open', 'onToggle', 'closeAfter'];
export function expand(tag, props, children, identity, parent) {
  if (props[panelMark]) return undefined;
  if (tag === 'dialog') return dialog(props, children, identity, parent);
  if (popoverTags.has(tag) && props.popover !== undefined && props.popover !== false) return popover(tag, props, children, identity, parent);
  if (popoverTags.has(tag)) for (let i = 0; i < popoverProps.length; i++)
    if (props[popoverProps[i]] !== undefined) throw Error(`UI_PROP: ${popoverProps[i]} belongs to a popover; add popover`);
  return undefined;
}

function common(props, parent, what) {
  if (parent.inControl) throw Error(`UI_BUTTON: a ${what} cannot sit inside a pressable control`);
  if (props.open !== undefined && typeof props.open !== 'boolean') throw Error('UI_PROP: open must be boolean');
}

function dialog(props, children, identity, parent) {
  common(props, parent, '<dialog>');
  const { open, onClose, closedby = 'closerequest', ...rest } = props;
  if (onClose !== undefined && typeof onClose !== 'function') throw Error('UI_PROP: onClose');
  if (!closers.has(closedby)) throw Error('UI_PROP: closedby must be "any", "closerequest" or "none"');
  if (!open || !admitted(identity)) return null;
  const close = () => onClose?.();
  collected().entries.push({ identity, kind: 'dialog', sequence: opened.get(identity), close: closedby === 'none' ? null : close });
  const env = envOf(render.view);
  const backdrop = { ...backdropFade, backgroundColor: 'scrim', ...resolveClasses(props.className, env)?.backdrop };
  const testId = props['data-testid'];
  return {
    node: layer(identity, env,
      h('button', { key: 'backdrop', 'aria-hidden': true, 'data-testid': testId && `${testId}-backdrop`, onClick: closedby === 'any' ? close : () => {},
        ...(rest.drag !== undefined ? { [scrimMark]: true } : {}),
        style: { position: 'absolute', top: 0, left: 0, right: 0, bottom: 0, ...backdrop } }),
      h('dialog', { ...rest, key: 'panel', [panelMark]: true, [popoverScope]: parent.popovers ?? [], ...(closedby === 'none' ? {} : { [dragClose]: close }), style: { ...placement(rest, env), ...rest.style } }, ...children)),
    finish: result => { result.wire.push(['topLayer', 1]); },
  };
}

function popover(tag, props, children, identity, parent) {
  common(props, parent, 'popover');
  const { popover: type, id, open, onToggle, closeAfter, ...rest } = props;
  const kind = type === true || type === '' || type === 'auto' ? 'auto' : type === 'manual' || type === 'hint' ? type : null;
  if (!kind) throw Error('UI_PROP: popover is "auto", "manual" or "hint"');
  if (typeof id !== 'string' || !id.length || id.length > 256) throw Error('UI_ID: a popover needs an id of 1..256 characters');
  if (onToggle !== undefined && typeof onToggle !== 'function') throw Error('UI_PROP: onToggle');
  if (closeAfter !== undefined && (kind !== 'manual' || !Number.isInteger(closeAfter) || closeAfter < 1 || closeAfter > 60000))
    throw Error('UI_PROP: closeAfter takes 1..60000 ms on a popover="manual"');
  const top = collected();
  if (top.popovers.has(id)) throw Error(`UI_ID: two popovers have the id "${id}"`);
  const controlled = open !== undefined;
  const shown = controlled ? open : uncontrolled.has(id);
  const scope = parent.popovers ?? [];
  const entry = { kind, controlled, onToggle, open: false, ancestors: scope };
  top.popovers.set(id, entry);
  if (!shown || !admitted(identity)) { anchors.delete(id); return null; }
  entry.open = true;
  // The invoker that opened it anchors it; one opened from code is placed by its classes.
  if (!anchors.has(id)) anchors.set(id, invoker?.id === id ? invoker.identity : null);
  const anchor = anchors.get(id);
  top.entries.push({ identity, kind, sequence: opened.get(identity), close: kind === 'manual' ? null : () => toggle(id, false) });
  const env = envOf(render.view);
  const style = anchor ? { ...rest.style, position: 'absolute', top: 0, left: 0 } : { ...placement(rest, env), ...rest.style };
  const testId = props['data-testid'];
  const node = layerWith(anchor ? {} : clearOfBars(env), identity, env,
    kind === 'auto' && h('button', { key: 'backdrop', 'aria-hidden': true, 'data-testid': testId && `${testId}-backdrop`, onClick: () => toggle(id, false),
      style: { position: 'absolute', top: 0, left: 0, right: 0, bottom: 0, backgroundColor: 'transparent' } }),
    h(tag, { ...rest, id, key: 'panel', [panelMark]: true, [popoverScope]: [...scope, id], style }, ...children));
  return {
    node,
    finish: result => {
      // A manual popover is in the top layer but takes no close request; a hint takes Escape.
      result.wire.push(['topLayer', kind === 'manual' ? 2 : 1]);
      // The core closes a hint on a press elsewhere; it has no backdrop to take that press.
      // A host older than the flag keeps hints open until their invoker or Escape closes them.
      const host = globalThis.__lingxiaViewHost;
      if (kind === 'hint' && !(host && fieldIds.hint > host.maxField)) result.wire.push(['hint', 1]);
      // The host counts it down; an older host leaves the popover open until the page closes it.
      if (closeAfter !== undefined && !(host && fieldIds.closeAfter > host.maxField)) result.wire.push(['closeAfter', closeAfter]);
      const panel = result.children.find(child => child.props?.key === 'panel');
      if (anchor && panel) panel.wire.push(['anchorTo', anchorName(id)]);
    },
  };
}

const anchorName = id => `--lx-popover-${id}`;

/** Open or close popover `id` as its invoker or a light dismiss asks; outside render. */
export function toggle(id, next, from) {
  const top = page.committed?.topLayer;
  const entry = top?.popovers.get(id);
  if (!entry || entry.open === next) return false;
  if (next && entry.kind !== 'manual') {
    // Opening an auto popover closes the other autos and hints; a hint closes only other hints.
    // Those it sits in or opens from stay.
    for (const [other, state] of top.popovers)
      if (other !== id && state.open && (state.kind === 'hint' || (entry.kind === 'auto' && state.kind === 'auto'))
        && !entry.ancestors.includes(other) && !(from?.scope ?? []).includes(other))
        toggle(other, false);
  }
  if (next) invoker = from ? { id, identity: from.identity } : null;
  entry.open = next;
  if (!entry.controlled) { if (next) uncontrolled.add(id); else uncontrolled.delete(id); page.invalidated = true; }
  entry.onToggle?.(next);
  return true;
}

/** The host's `closeAfter` count for popover `id` ran out: it closes as a light dismiss does. */
export function closeAfterExpired(id) {
  const entry = page.committed?.topLayer?.popovers.get(id);
  return entry?.kind === 'manual' && entry.open ? toggle(id, false) : false;
}

/** A press landed away from the hints in `keep`: every other open hint closes, whatever opened it. */
export function dismissHints(keep) {
  const top = page.committed?.topLayer;
  if (!top || !Array.isArray(keep)) return false;
  let changed = false;
  for (const [id, state] of top.popovers) if (state.kind === 'hint' && state.open && !keep.includes(id)) changed = toggle(id, false) || changed;
  return changed;
}

/** A button's `popoverTarget`: its click toggles the popover; `role="menuitem"` closes its menu. */
export function invokerHandler(props, identity, scope) {
  const { popoverTarget: target, onClick, role } = props;
  if (target !== undefined) {
    if (typeof target !== 'string' || !target.length) throw Error('UI_ID: popoverTarget names a popover id');
    collected().invokers.push({ target, identity, scope });
  }
  if (role === 'menuitem' && !scope.length) throw Error('UI_PROP: role="menuitem" belongs inside a popover menu');
  if (target === undefined && role !== 'menuitem') return onClick;
  return () => {
    onClick?.();
    if (target !== undefined) toggle(target, !page.committed?.topLayer?.popovers.get(target)?.open, { identity, scope });
    if (role === 'menuitem') toggle(scope[scope.length - 1], false);
  };
}

/** A button's `interestFor`: hovering it or focusing it from a key shows that hint, as the core times it. */
export function interestTrigger(props, identity, scope) {
  const target = props.interestFor;
  if (target === undefined) return;
  if (typeof target !== 'string' || !target.length || target.length > 256 || /\s/.test(target)) throw Error('UI_ID: interestFor names a popover id');
  collected().interests.push({ target, identity, scope });
}

/** The core's interest timer ran out for the `nth` trigger, in tree order, of hint `target`: show or hide it. True when it changed. */
export function interest(target, nth, show) {
  const entry = page.committed?.topLayer?.interests.filter(candidate => candidate.target === target)[nth];
  return entry ? toggle(target, show, { identity: entry.identity, scope: entry.scope }) : false;
}

/** After resolution: invokers name popovers that exist and announce whether they are open. */
export function finishTopLayer(tree) {
  const top = render.candidate.topLayer;
  for (const identity of [...opened.keys()]) if (!top?.entries.some(entry => entry.identity === identity)) opened.delete(identity);
  if (top) wireInvokers(top, tree);
}

/** A list row renders while the page encodes, after `finishTopLayer`: its invokers are wired here. */
export function finishTopLayerRow(row) {
  const top = render.candidate?.topLayer;
  if (top) wireInvokers(top, row);
}

function wireInvokers(top, tree) {
  const invokers = new Map(top.invokers.map(entry => [entry.identity, entry]));
  const interests = new Map(top.interests.map(entry => [entry.identity, entry]));
  const walk = node => {
    const interest = interests.get(node.identity);
    if (interest) {
      const target = top.popovers.get(interest.target);
      if (!target) throw Error(`UI_ID: interestFor "${interest.target}" names no popover on this page`);
      // While it shows, the hint describes its trigger; the trigger keeps its own description if it has one.
      if (target.open && node.props['aria-describedby'] === undefined) node.wire.push(['ariaDescribedBy', interest.target]);
      if (!invokers.has(node.identity) && target.open && anchors.get(interest.target) === node.identity)
        node.wire.push(['anchorName', anchorName(interest.target)]);
    }
    const entry = invokers.get(node.identity);
    if (entry && node.element === 'button') {
      const target = top.popovers.get(entry.target);
      if (!target) throw Error(`UI_ID: popoverTarget "${entry.target}" names no popover on this page`);
      if (node.props.accessibilityState?.expanded === undefined) node.wire.push(['stateExpanded', +target.open]);
      // The invoker that opened the popover carries the CSS anchor name it sits against.
      if (target.open && anchors.get(entry.target) === node.identity) node.wire.push(['anchorName', anchorName(entry.target)]);
    }
    for (const child of node.children ?? []) walk(child);
  };
  walk(tree);
}

/** Escape, Android back and the screen reader's dismiss close the topmost closable overlay. */
export function closeTopmost() {
  // Manual popovers take no close request; a dialog that refuses one shields those below it.
  const entries = (page.committed?.topLayer?.entries ?? []).filter(entry => entry.kind !== 'manual');
  const top = [...entries].sort((a, b) => b.sequence - a.sequence)[0];
  if (!top?.close) return false;
  top.close();
  return true;
}

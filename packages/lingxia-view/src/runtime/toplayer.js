// HTML's top layer: an open <dialog> or popover lays out against the whole page and paints above
// it in open order, wherever it sits in the tree. Native routes `topLayer` boxes to the root.
import { h } from './nodes.js';
import { render, page } from './state.js';
import { envOf } from './hooks.js';
import { resolveClasses } from '../style/classes.js';
import { limits } from '../generated/contract.js';
import { report } from './actions.js';

/** Marks the panel a <dialog> or popover expands to, so resolution does not expand it again. */
export const panelMark = Symbol('top-layer panel');
/** A dialog's close, for its panel to call when dragged off screen. */
export const dragClose = Symbol('drag close');
/** Marks a backdrop that fades with how much of its dragged dialog is on screen. */
export const scrimMark = Symbol('drag scrim');
/** The popover ids a node sits inside, innermost last. */
export const popoverScope = Symbol('popover scope');

const popoverTags = new Set(['view', 'column', 'row', 'stack', 'grid']);
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
  return render.candidate.topLayer ??= { entries: [], popovers: new Map(), invokers: [] };
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
  return h('column', { style: { position: 'absolute', top: 0, left: 0, right: 0, bottom: env?.keyboard?.height ?? 0, zIndex: layerZ + opened.get(identity) % 1000 } }, ...children);
}

const backdropFade = { opacity: 1, transition: { properties: ['opacity'], durationMs: 200, delayMs: 0, easing: [0, 0, 0.58, 1] } };

/**
 * `<dialog>` and popovers expand to a top-layer box holding a backdrop and the panel; `null`
 * when closed, `undefined` for any other node. `finish` decorates the resolved layer.
 */
const popoverProps = ['open', 'onToggle', 'id'];
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
  const { popover: type, id, open, onToggle, ...rest } = props;
  const kind = type === true || type === '' || type === 'auto' ? 'auto' : type === 'manual' ? 'manual' : null;
  if (!kind) throw Error('UI_PROP: popover is "auto" or "manual"');
  if (typeof id !== 'string' || !id.length || id.length > 256) throw Error('UI_ID: a popover needs an id of 1..256 characters');
  if (onToggle !== undefined && typeof onToggle !== 'function') throw Error('UI_PROP: onToggle');
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
  top.entries.push({ identity, kind, sequence: opened.get(identity), close: kind === 'auto' ? () => toggle(id, false) : null });
  const env = envOf(render.view);
  const style = anchor ? { ...rest.style, position: 'absolute', top: 0, left: 0 } : { ...placement(rest, env), ...rest.style };
  const testId = props['data-testid'];
  return {
    node: layer(identity, env,
      kind === 'auto' && h('button', { key: 'backdrop', 'aria-hidden': true, 'data-testid': testId && `${testId}-backdrop`, onClick: () => toggle(id, false),
        style: { position: 'absolute', top: 0, left: 0, right: 0, bottom: 0, backgroundColor: 'transparent' } }),
      h(tag, { ...rest, key: 'panel', [panelMark]: true, [popoverScope]: [...scope, id], style }, ...children)),
    finish: result => {
      // A manual popover is in the top layer but takes no close request.
      result.wire.push(['topLayer', kind === 'auto' ? 1 : 2]);
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
  if (next && entry.kind === 'auto') {
    // Opening one auto popover closes the others, except those it sits in or opens from.
    for (const [other, state] of top.popovers)
      if (other !== id && state.open && state.kind === 'auto' && !entry.ancestors.includes(other) && !(from?.scope ?? []).includes(other))
        toggle(other, false);
  }
  if (next) invoker = from ? { id, identity: from.identity } : null;
  entry.open = next;
  if (!entry.controlled) { if (next) uncontrolled.add(id); else uncontrolled.delete(id); page.invalidated = true; }
  entry.onToggle?.(next);
  return true;
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

/** After resolution: invokers name popovers that exist and announce whether they are open. */
export function finishTopLayer(tree) {
  const top = render.candidate.topLayer;
  for (const identity of [...opened.keys()]) if (!top?.entries.some(entry => entry.identity === identity)) opened.delete(identity);
  if (!top) return;
  const invokers = new Map(top.invokers.map(entry => [entry.identity, entry]));
  const walk = node => {
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

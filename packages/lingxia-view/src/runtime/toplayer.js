// HTML's top layer: an open <dialog> or popover lays out against the whole page and paints above
// it in open order, wherever it sits in the tree. Native routes `topLayer` boxes to the root.
import { h } from './nodes.js';
import { render, page } from './state.js';
import { envOf, cellOf } from './hooks.js';
import { noteEnv } from './memo.js';
import { resolveClasses } from '../style/classes.js';
import { limits } from '../generated/contract.js';
import { report } from './actions.js';

/** Marks the panel a <dialog> or popover expands to, so resolution does not expand it again. */
export const panelMark = Symbol('top-layer panel');
/** A dialog's close, for its panel to call when dragged off screen. */
export const dragClose = Symbol('drag close');
/** Marks a backdrop that fades with how much of its dragged dialog is on screen. */
export const scrimMark = Symbol('drag scrim');
/** The `{ starting, exit }` paint a dialog's backdrop enters from and leaves toward. */
export const presenceStyle = Symbol('backdrop presence');
/** Marks a panel whose `backdrop:` classes style its backdrop: a dialog's, or a light-dismiss popover's. */
export const backdropOwner = Symbol('backdrop owner');
/** The popover ids a node sits inside, innermost last. */
export const popoverScope = Symbol('popover scope');

const popoverTags = new Set(['view']);
const closers = new Set(['any', 'closerequest', 'none']);
// Below Logic's feedback and the pickers, which the View draws over everything.
const layerZ = 9000;

let uncontrolled = new Set(), anchors = new Map(), opened = new Map(), refused = new Set();
// An open dialog's bound drag cell and the detent it opened at, put back however it closes.
let resting = new Map();
let invoker = null, sequence = 0;
// A submenu and the menu whose item opened it.
let submenus = new Map();

export function resetTopLayer() {
  uncontrolled = new Set(); anchors = new Map(); opened = new Map(); refused = new Set(); resting = new Map();
  invoker = null; sequence = 0; submenus = new Map();
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

const rtlOf = (parent, env) => (parent.direction ?? env?.direction) === 'rtl';

function layerWith(style, identity, env, ...children) {
  return h('view', { style: { position: 'absolute', top: 0, left: 0, right: 0, bottom: env?.keyboard?.height ?? 0, zIndex: layerZ + opened.get(identity) % 1000, ...style } }, ...children);
}

const sides = ['Top', 'Right', 'Bottom', 'Left'];

// The safe inset on each side of the layer; over an open keyboard the bottom is the keyboard's.
function barInsets(env) {
  const insets = env?.insets;
  if (!insets) return null;
  return { Top: insets.top ?? 0, Right: insets.right ?? 0, Left: insets.left ?? 0,
    Bottom: Math.max(0, (insets.bottom ?? 0) - (env.keyboard?.height ?? 0)) };
}

const zero = value => value === 0 || (Array.isArray(value) && value[1] === 0 && value[2] === 0);

// The physical field an inline side reads: the logical one when the panel sets it, as layout does.
function inlineField(set, prefix, side, rtl) {
  const logical = `${prefix}InlineStart`, other = `${prefix}InlineEnd`;
  const mapped = (side === 'Left') !== rtl ? logical : other;
  return set[mapped] !== undefined ? mapped : `${prefix}${side}`;
}

// The edges a classes-placed panel is docked to: one it meets with a zero margin (or a zero inset,
// placed absolutely) while it spans the screen along it, as a sheet or drawer does; or one its
// uncapped full size reaches. A small box set in a corner is not docked.
function docked(set, env, rtl) {
  const viewport = env?.viewport;
  const full = (size, max, extent) => set[size] === '100%'
    && (set[max] === undefined || (typeof set[max] === 'number' && extent !== undefined && set[max] >= extent));
  const reaches = { Top: full('height', 'maxHeight', viewport?.height), Bottom: full('height', 'maxHeight', viewport?.height),
    Left: full('width', 'maxWidth', viewport?.width), Right: full('width', 'maxWidth', viewport?.width) };
  const inline = side => side === 'Left' || side === 'Right';
  const inset = side => set[side.toLowerCase()] ?? (inline(side) ? set[(side === 'Left') !== rtl ? 'start' : 'end'] : undefined);
  const meets = side => zero(set[inline(side) ? inlineField(set, 'margin', side, rtl) : `margin${side}`])
    || (set.position === 'absolute' && zero(inset(side)));
  const spans = side => inline(side)
    ? set.height === '100%' || (set.position === 'absolute' && zero(inset('Top')) && zero(inset('Bottom')))
    : set.width === '100%' || (set.position === 'absolute' && zero(inset('Left')) && zero(inset('Right')));
  return new Set(sides.filter(side => reaches[side] || (meets(side) && spans(side))));
}

// A panel placed by its classes lays out in the part of the surface the bars leave clear, except
// on the edges it is docked to: there its surface runs to the screen's edge and its content is
// padded by the inset, as a native sheet's is. Its backdrop, absolutely placed, covers the whole layer.
function dock(props, env, rtl) {
  const insets = barInsets(env);
  if (!insets) return { layer: {}, panel: {} };
  const set = { ...resolveClasses(props.className, { ...env, direction: rtl ? 'rtl' : 'ltr' })?.base, ...props.style };
  const edges = docked(set, env, rtl);
  const layer = {}, panel = {};
  for (const side of sides) {
    if (!edges.has(side)) { layer[`padding${side}`] = insets[side]; continue; }
    if (!insets[side]) continue;
    const field = side === 'Left' || side === 'Right' ? inlineField(set, 'padding', side, rtl) : `padding${side}`;
    const own = set[field] ?? set.padding ?? 0;
    if (typeof own === 'number') panel[field] = own + insets[side];
    else if (Array.isArray(own)) panel[field] = [own[0], own[1], own[2] + insets[side]];
  }
  return { layer, panel };
}

const backdropTiming = { durationMs: 200, delayMs: 0, easing: [0, 0, 0.58, 1] };

// The backdrop fades in and out with the dialog, on its transition timing: only when the dialog
// itself has `starting:`/`exit:` paint, so a dialog that appears or leaves at once takes its
// backdrop with it. `backdrop:` classes override its look, timing and presence.
function backdropOf(classes) {
  const own = classes?.base?.transition;
  const timing = { ...(own && typeof own === 'object' ? { durationMs: own.durationMs, delayMs: own.delayMs, easing: own.easing } : backdropTiming), ...classes?.backdrop?.timing };
  const fades = name => classes?.[name] && Object.keys(classes[name]).length ? { opacity: 0 } : {};
  return {
    style: { opacity: 1, backgroundColor: 'scrim', transition: { properties: ['opacity'], ...timing }, ...classes?.backdrop?.base },
    presence: { starting: { ...fades('starting'), ...classes?.backdrop?.starting }, exit: { ...fades('exit'), ...classes?.backdrop?.exit } },
  };
}

/**
 * `<dialog>` and popovers expand to a top-layer box holding a backdrop and the panel; `null`
 * when closed, `undefined` for any other node. `finish` decorates the resolved layer.
 */
export function expand(tag, props, children, identity, parent) {
  if (props[panelMark]) return undefined;
  if (tag === 'dialog') return dialog(props, children, identity, parent);
  if (popoverTags.has(tag) && props.popover !== undefined && props.popover !== false) return popover(tag, props, children, identity, parent);
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
  const opening = !opened.has(identity);
  if (!open || !admitted(identity)) return null;
  const bound = rest.drag !== undefined && rest.bind !== undefined ? cellOf(rest.bind) : undefined;
  if (opening && bound && typeof bound.cell.value === 'number') resting.set(identity, { ...bound, index: bound.cell.value });
  const close = () => onClose?.();
  collected().entries.push({ identity, kind: 'dialog', sequence: opened.get(identity), close: closedby === 'none' ? null : close });
  // Placement and docking read the bars, insets and viewport.
  noteEnv('*');
  const env = envOf(render.view);
  const backdrop = backdropOf(resolveClasses(props.className, env));
  const testId = props['data-testid'];
  const docking = dock(rest, env, rtlOf(parent, env));
  return {
    node: layerWith(docking.layer, identity, env,
      h('button', { key: 'backdrop', 'aria-hidden': true, 'data-testid': testId && `${testId}-backdrop`, onClick: closedby === 'any' ? close : () => {},
        ...(rest.drag !== undefined ? { [scrimMark]: true } : {}), [presenceStyle]: backdrop.presence,
        style: { position: 'absolute', top: 0, left: 0, right: 0, bottom: 0, ...backdrop.style } }),
      h('dialog', { ...rest, key: 'panel', [panelMark]: true, [backdropOwner]: true, [popoverScope]: parent.popovers ?? [], ...(closedby === 'none' ? {} : { [dragClose]: close }), style: { ...placement(rest, env), ...rest.style, ...docking.panel } }, ...children)),
    finish: result => { result.wire.push(['topLayer', 1]); },
  };
}

function popover(tag, props, children, identity, parent) {
  common(props, parent, 'popover');
  const { popover: type, id, open, onToggle, onCloseRequest, closeAfter, anchor: anchorProp, ...rest } = props;
  const kind = type === true || type === '' || type === 'auto' ? 'auto' : type === 'manual' || type === 'hint' ? type : null;
  if (!kind) throw Error('UI_PROP: popover is "auto", "manual" or "hint"');
  if (typeof id !== 'string' || !id.length || id.length > 256) throw Error('UI_ID: a popover needs an id of 1..256 characters');
  if (onToggle !== undefined && typeof onToggle !== 'function') throw Error('UI_PROP: onToggle');
  if (onCloseRequest !== undefined && typeof onCloseRequest !== 'function') throw Error('UI_PROP: onCloseRequest takes a function');
  if (anchorProp !== undefined && (typeof anchorProp !== 'string' || !anchorProp.length || anchorProp.length > 256 || /\s/.test(anchorProp)))
    throw Error('UI_ID: anchor names an element id');
  // `editor:caret` or `editor:selection`: the core places it against that part of the editor, each frame.
  const [, anchorId, anchorPart] = anchorProp === undefined ? [] : /^(.+?)(?::(caret|selection))?$/.exec(anchorProp);
  if (closeAfter !== undefined && (kind !== 'manual' || !Number.isInteger(closeAfter) || closeAfter < 1 || closeAfter > 60000))
    throw Error('UI_PROP: closeAfter takes 1..60000 ms on a popover="manual"');
  const top = collected();
  if (top.popovers.has(id)) throw Error(`UI_ID: two popovers have the id "${id}"`);
  const controlled = open !== undefined;
  const shown = controlled ? open : uncontrolled.has(id);
  const scope = parent.popovers ?? [];
  const entry = { kind, controlled, onToggle, open: false, ancestors: scope, anchorId };
  top.popovers.set(id, entry);
  if (!shown || !admitted(identity)) { anchors.delete(id); submenus.delete(id); return null; }
  entry.open = true;
  // The invoker that opened it anchors it; one opened from code is placed by its classes,
  // unless it names the element it sits against.
  if (!anchors.has(id)) anchors.set(id, invoker?.id === id ? invoker.identity : null);
  const anchor = anchorId ?? anchors.get(id);
  top.entries.push({ identity, kind, sequence: opened.get(identity), close: kind === 'manual' ? null : () => { if (onCloseRequest?.() !== false) toggle(id, false); } });
  noteEnv('*');
  const env = envOf(render.view);
  const docking = anchor ? { layer: {}, panel: {} } : dock(rest, env, rtlOf(parent, env));
  // A light-dismiss popover's backdrop is clear unless its `backdrop:` classes paint it, as a sheet's scrim.
  const lit = kind === 'auto' && !submenus.has(id);
  const classes = lit ? resolveClasses(rest.className, env) : undefined;
  const backdrop = classes?.backdrop ? backdropOf(classes) : null;
  const style = anchor ? { ...rest.style, position: 'absolute', top: 0, left: 0 } : { ...placement(rest, env), ...rest.style, ...docking.panel };
  const testId = props['data-testid'];
  const node = layerWith(docking.layer, identity, env,
    lit && h('button', { key: 'backdrop', 'aria-hidden': true, 'data-testid': testId && `${testId}-backdrop`, onClick: () => toggle(id, false),
      ...(backdrop ? { [presenceStyle]: backdrop.presence } : {}),
      style: { position: 'absolute', top: 0, left: 0, right: 0, bottom: 0, ...(backdrop ? { ...backdrop.style, backgroundColor: 'transparent', ...classes.backdrop.base } : { backgroundColor: 'transparent' }) } }),
    h(tag, { ...rest, id, key: 'panel', [panelMark]: true, ...(lit ? { [backdropOwner]: true } : {}), [popoverScope]: [...scope, id], style }, ...children));
  return {
    node,
    finish: result => {
      // A manual popover is in the top layer but takes no close request; a hint takes Escape.
      result.wire.push(['topLayer', kind === 'manual' ? 2 : 1]);
      // The core closes a hint on a press elsewhere; it has no backdrop to take that press.
      if (kind === 'hint') result.wire.push(['hint', 1]);
      // The host counts it down.
      if (closeAfter !== undefined) result.wire.push(['closeAfter', closeAfter]);
      const panel = result.children.find(child => child.props?.key === 'panel');
      if (anchor && panel) panel.wire.push(['anchorTo', anchorName(id) + (anchorPart ? `:${anchorPart}` : '')]);
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
    // Those it sits in or opens from stay, and so do the menus above the one it opens from.
    const kept = new Set([...entry.ancestors, ...(from?.scope ?? []), ...menuChain(from?.menu)]);
    for (const [other, state] of top.popovers)
      if (other !== id && state.open && (state.kind === 'hint' || (entry.kind === 'auto' && state.kind === 'auto')) && !kept.has(other))
        toggle(other, false);
  }
  if (next) {
    invoker = from ? { id, identity: from.identity } : null;
    if (from?.menu !== undefined) submenus.set(id, from.menu); else submenus.delete(id);
  } else {
    // A menu takes its submenus with it, innermost first.
    for (const [child, parent] of [...submenus]) if (parent === id) toggle(child, false);
    submenus.delete(id);
  }
  entry.open = next;
  if (!entry.controlled) { if (next) uncontrolled.add(id); else uncontrolled.delete(id); page.invalidated = true; }
  entry.onToggle?.(next);
  return true;
}

// Menu `id` and the menus it was opened from, innermost first.
function menuChain(id) {
  const chain = [];
  for (let at = id; at !== undefined && !chain.includes(at); at = submenus.get(at)) chain.push(at);
  return chain;
}

/** The host's `closeAfter` count for popover `id` ran out: it closes as a light dismiss does. */
export function closeAfterExpired(id) {
  const entry = page.committed?.topLayer?.popovers.get(id);
  return entry?.kind === 'manual' && entry.open ? toggle(id, false) : false;
}

/** The page was covered or left inside the app: its open auto popovers and hints close as a tap
 *  outside would. True when one did. */
export function closePassing() {
  const top = page.committed?.topLayer;
  let changed = false;
  // Innermost first, so a nested popover's onToggle runs before its parent's.
  for (const [id, state] of [...(top?.popovers ?? [])].reverse())
    if (state.open && state.kind !== 'manual') changed = toggle(id, false) || changed;
  return changed;
}

/** The open popovers an invoker or interest trigger opened; one opened from code has no owner. */
export function ownedPopovers() {
  const top = render.candidate?.topLayer ?? page.committed?.topLayer;
  return top ? [...top.popovers].filter(([id, entry]) => entry.open && (entry.anchorId || anchors.get(id))).map(([id]) => id) : [];
}

/** The controls that opened popovers `ids` left the page: each closes, whatever its kind. */
export function closeOrphans(ids) {
  const top = render.candidate?.topLayer ?? page.committed?.topLayer;
  for (const id of Array.isArray(ids) ? ids : []) {
    const entry = top?.popovers.get(id);
    if (!entry?.open) continue;
    entry.open = false;
    submenus.delete(id);
    if (!entry.controlled) { uncontrolled.delete(id); page.invalidated = true; }
    entry.onToggle?.(false);
  }
}

/** A press landed away from the hints in `keep`: every other open hint closes, whatever opened it. */
export function dismissHints(keep) {
  const top = page.committed?.topLayer;
  if (!top || !Array.isArray(keep)) return false;
  let changed = false;
  for (const [id, state] of top.popovers) if (state.kind === 'hint' && state.open && !keep.includes(id)) changed = toggle(id, false) || changed;
  return changed;
}

/**
 * A button's `popoverTarget`: its click toggles the popover. A `role="menuitem"` closes its menu
 * and the menus that one opened from; one that opens a submenu or has `keepOpen` keeps them open.
 */
export function invokerHandler(props, identity, scope) {
  const { popoverTarget: target, onClick, role, keepOpen } = props;
  if (keepOpen !== undefined && typeof keepOpen !== 'boolean') throw Error('UI_PROP: keepOpen is a boolean');
  if (target !== undefined) {
    if (typeof target !== 'string' || !target.length) throw Error('UI_ID: popoverTarget names a popover id');
    collected().invokers.push({ target, identity, scope });
  }
  if (role === 'menuitem' && !scope.length) throw Error('UI_PROP: role="menuitem" belongs inside a popover menu');
  if (target === undefined && (role !== 'menuitem' || keepOpen)) return onClick;
  return event => {
    onClick?.(event);
    const menu = role === 'menuitem' ? scope[scope.length - 1] : undefined;
    if (target !== undefined) toggle(target, !page.committed?.topLayer?.popovers.get(target)?.open, { identity, scope, menu });
    else for (const id of menuChain(menu)) toggle(id, false);
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
  for (const identity of [...opened.keys()]) {
    if (top?.entries.some(entry => entry.identity === identity)) continue;
    opened.delete(identity);
    // Closed by a request, a drag or the page's own `open={false}`: the next open starts where this one did.
    const rest = resting.get(identity);
    if (rest) { resting.delete(identity); (render.candidate.dragResets ??= []).push(rest); }
  }
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
  // Open popovers that name their anchor element, by that element's id.
  const named = new Map();
  for (const [id, entry] of top.popovers) if (entry.open && entry.anchorId) named.set(entry.anchorId, [...(named.get(entry.anchorId) ?? []), id]);
  const walk = node => {
    // One element may anchor several popovers: it carries every name, a line each.
    const names = [];
    const anchored = named.size && typeof node.props?.id === 'string' ? named.get(node.props.id) : undefined;
    if (anchored !== undefined) for (const id of anchored) names.push(anchorName(id));
    const interest = interests.get(node.identity);
    if (interest) {
      const target = top.popovers.get(interest.target);
      if (!target) throw Error(`UI_ID: interestFor "${interest.target}" names no popover on this page`);
      // While it shows, the hint describes its trigger; the trigger keeps its own description if it has one.
      if (target.open && node.props['aria-describedby'] === undefined) node.wire.push(['ariaDescribedBy', interest.target]);
      if (!invokers.has(node.identity) && target.open && !target.anchorId && anchors.get(interest.target) === node.identity)
        names.push(anchorName(interest.target));
    }
    const entry = invokers.get(node.identity);
    if (entry && (node.element === 'button' || node.element === 'link')) {
      const target = top.popovers.get(entry.target);
      if (!target) throw Error(`UI_ID: popoverTarget "${entry.target}" names no popover on this page`);
      if (node.props.accessibilityState?.expanded === undefined) node.wire.push(['stateExpanded', +target.open]);
      // The invoker that opened the popover carries the CSS anchor name it sits against.
      if (target.open && !target.anchorId && anchors.get(entry.target) === node.identity) names.push(anchorName(entry.target));
    }
    if (names.length) node.wire.push(['anchorName', [...new Set(names)].join('\n')]);
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

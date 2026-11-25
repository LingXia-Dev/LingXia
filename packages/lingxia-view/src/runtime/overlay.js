// Feedback the Logic layer asks the page to present (`lx.ui.toast`, `alert`,
// `confirm`, `actionSheet`), drawn by the View itself on desktop GPU pages. Each call answers
// through the same bridge reply the WebView polyfill sends, so Logic sees no difference.
import { h } from './nodes.js';
import { lowerStyle } from '../style/lower.js';
import { closeTopmost } from './toplayer.js';
import { word } from './words.js';

let toast = null, modals = [], sheets = [], timers = [], nextToken = 1, wakes = new Map();
const ENTER_MS = 200;

const reply = (id, ok, payload) => ok ? { kind: 'res', id, ok: true, result: payload ?? null }
  : { kind: 'res', id, ok: false, error: payload };
const text = (value, fallback = '') => typeof value === 'string' ? value.slice(0, 4096) : fallback;

/** A request from Logic, `{id, method, params}`: `{changed, replies}`, replies being the
 *  messages to send back. */
export function viewCall(call) {
  const { id, method } = call, params = call.params ?? {};
  const replies = [];
  let changed = false;
  switch (method) {
    case 'ui.showToast': {
      const icon = ['success', 'error', 'loading'].includes(params.icon) ? params.icon : 'none';
      const position = ['top', 'bottom'].includes(params.position) ? params.position : 'center';
      const mask = params.mask === true;
      const duration = Number.isFinite(params.duration) ? Math.max(0, Math.min(params.duration, 600000)) : 1500;
      // A repeat with the same shape updates the live toast in place ("1/4" → "2/4").
      const same = toast && toast.icon === icon && toast.position === position && toast.mask === mask;
      toast = { token: nextToken++, title: text(params.title), icon, position, mask, key: same ? toast.key : nextToken };
      if (duration > 0) timers.push([toast.token, Math.round(duration)]);
      replies.push(reply(id, true, null));
      changed = true;
      break;
    }
    case 'ui.hideToast':
      changed = toast !== null;
      toast = null;
      replies.push(reply(id, true, null));
      break;
    case 'ui.dismissDialog':
      // Only the front modal and sheet are on screen; queued ones show next.
      if (modals.length) { replies.push(reply(modals.shift().id, true, { confirm: false, cancel: true })); changed = true; }
      if (sheets.length) { replies.push(reply(sheets.shift().id, true, { tapIndex: -1 })); changed = true; }
      replies.push(reply(id, true, null));
      break;
    case 'ui.showModal':
      modals.push({ id, key: nextToken++, title: text(params.title), content: text(params.content), showCancel: params.showCancel !== false,
        cancelText: text(params.cancelText, null), confirmText: text(params.confirmText, null) });
      changed = modals.length === 1;
      break;
    case 'ui.showActionSheet': {
      const items = Array.isArray(params.itemList) ? params.itemList.slice(0, 32).map(item => text(item)) : [];
      if (!items.length) { replies.push(reply(id, false, { code: 'E_INVALID_ARGUMENT', message: 'showActionSheet needs itemList' })); break; }
      sheets.push({ id, key: nextToken++, items, cancelText: text(params.cancelText, null) });
      changed = sheets.length === 1;
      break;
    }
    default:
      replies.push(reply(id, false, { code: 'E_UNSUPPORTED', message: `${String(method)} is not served by a GPU View` }));
  }
  return { changed, replies };
}

/** The toast timer `token` ran out; true when it hid the toast still showing. */
export function overlayTimer(token) {
  const wake = wakes.get(token);
  if (wake) { wakes.delete(token); return wake() === true; }
  if (!toast || toast.token !== token) return false;
  toast = null;
  return true;
}

/** Escape: the innermost overlay goes, as its cancel would. True when one did. */
export function escapeOverlay() {
  if (closeTopmost()) return true;
  if (toast) { toast = null; return true; }
  return false;
}

/** Run `fire` after `ms` on the host's toast clock; true from it repaints the page. */
export function requestWake(ms, fire) {
  const token = nextToken++;
  wakes.set(token, fire);
  timers.push([token, Math.round(ms)]);
}

export function takeOverlayTimers() { const result = timers; timers = []; return JSON.stringify(result); }

let outbound = [];
/** Replies a tap produced (a modal button, a sheet item), taken with the action requests. */
export function takeOverlayReplies() { const result = outbound; outbound = []; return result; }

function settleModal(confirm) {
  const modal = modals.shift();
  if (modal) outbound.push(reply(modal.id, true, { confirm, cancel: !confirm }));
}
function settleSheet(tapIndex) {
  const sheet = sheets.shift();
  if (sheet) outbound.push(reply(sheet.id, true, { tapIndex }));
}

/** The page was covered or left: its action sheets cancel, as a tap outside does. */
export function cancelSheets() {
  const had = sheets.length > 0;
  while (sheets.length) settleSheet(-1);
  return had;
}

/** The page went away; its connection, and Logic's calls waiting on it, went with it. */
export function resetOverlays() {
  toast = null; modals = []; sheets = []; timers = []; outbound = []; wakes = new Map();
}

export const hasOverlay = () => Boolean(toast || modals.length || sheets.length);

// The mobile hosts' native toast (iOS LxAppToast): a dark HUD in every theme, filled status icons.
const RING = 'M14 0a14 14 0 0 1 14 14a14 14 0 0 1-14 14a14 14 0 0 1-14-14a14 14 0 0 1 14-14z';
const icons = {
  success: [{ d: RING, fill: 'success' }, { d: 'M8.5 14.5l3.5 3.5 7.5-7.5', fill: 'none', stroke: '#ffffff', strokeWidth: 2.5, strokeLinecap: 'round', strokeLinejoin: 'round' }],
  error: [{ d: RING, fill: 'danger-solid' }, { d: 'M10 10l8 8M18 10l-8 8', fill: 'none', stroke: 'on-danger-solid', strokeWidth: 2.5, strokeLinecap: 'round' }],
  loading: [{ d: 'M14 3a11 11 0 0 1 11 11', fill: 'none', stroke: 'accent', strokeWidth: 2.5, strokeLinecap: 'round' }],
};
const spin = { name: 'lx-overlay-spin', durationMs: 800, delayMs: 0, iterations: null, whenReduced: 'base', easing: 'linear', opacity: null,
  transforms: [{ kind: 'rotate', angle: [{ offset: 0, value: 0 }, { offset: 1, value: 360 }] }] };
const fade = { properties: ['opacity', 'transform'], durationMs: ENTER_MS, delayMs: 0, easing: [0.4, 0, 0.2, 1] };
const toastFade = { properties: ['opacity', 'transform'], durationMs: 200, delayMs: 0, easing: [0, 0, 0.58, 1] };
const fill = { position: 'absolute', top: 0, left: 0, right: 0, bottom: 0 };
// Children of a one-cell grid overlap in that cell.
const cell = { gridColumn: 1, gridRow: 1 };

function toastNode() {
  const { title, icon, position, mask } = toast;
  const bare = icon === 'none';
  const box = h('view', { 'data-testid': 'lx-toast', key: `toast-${toast.key}`, role: 'status', 'aria-live': 'polite', 'aria-label': title,
    style: { alignItems: 'center', justifyContent: 'center', gap: 12, minWidth: 120, maxWidth: 280, minHeight: bare ? 60 : 100,
      paddingLeft: 20, paddingRight: 20, paddingTop: bare ? 16 : 20, paddingBottom: bare ? 16 : 20, borderRadius: 12,
      backgroundColor: 'shadow/80', transition: toastFade } },
    !bare && h('vector', { 'data-testid': 'lx-toast-icon', 'aria-hidden': true, paths: icons[icon], viewBox: [0, 0, 28, 28], style: { width: 28, height: 28 } }),
    h('text', { style: { color: '#ffffff', fontSize: 16, fontWeight: 500, textAlign: 'center', lineClamp: bare ? 3 : 2 } }, title));
  // A manual popover: no light dismiss; with `mask` it covers the page and takes its taps.
  return h('view', { id: 'lx-toast', popover: 'manual', open: true, style: { display: 'grid', ...fill, marginTop: 0, marginBottom: 0, marginLeft: 0, marginRight: 0 } },
    mask && h('button', { 'data-testid': 'lx-toast-mask', 'aria-hidden': true, onClick() {}, style: { ...fill, ...cell, zIndex: 0, backgroundColor: 'shadow/30' } }),
    h('view', { style: { ...fill, ...cell, zIndex: 1, alignItems: 'center', justifyContent: position === 'top' ? 'start' : position === 'bottom' ? 'end' : 'center',
      paddingTop: position === 'top' ? 80 : 0, paddingBottom: position === 'bottom' ? 80 : 0 } }, box));
}

function modalNode(env) {
  const modal = modals[0];
  const language = env?.language;
  const button = (testId, label, confirm, color) => h('button', { 'data-testid': testId, onClick: () => settleModal(confirm), style: {
    flexGrow: 1, flexBasis: 0, minWidth: 0, paddingTop: 12, paddingBottom: 12, backgroundColor: 'transparent', color, fontSize: 16, fontWeight: confirm ? 600 : 400 } }, label);
  return h('dialog', { 'data-testid': 'lx-modal', role: 'alertdialog', key: `modal-${modal.key}`, open: true, onClose: () => settleModal(false),
    'aria-label': modal.title || modal.content || word('@view.dialog', language),
    style: { width: 320, maxWidth: '86%', borderRadius: 14, backgroundColor: 'surface-overlay', overflow: 'hidden', transition: fade } },
    h('view', { style: { padding: 20, gap: 8, alignItems: 'center' } },
      modal.title && h('text', { role: 'heading', style: { fontSize: 17, fontWeight: 600, textAlign: 'center', color: 'text' } }, modal.title),
      modal.content && h('text', { style: { fontSize: 15, textAlign: 'center', color: 'text-muted' } }, modal.content)),
    h('view', { style: { flexDirection: 'row', borderTopWidth: 1, borderColor: 'line' } },
      modal.showCancel && button('lx-modal-cancel', modal.cancelText ?? word('@view.cancel', language), false, 'text-muted'),
      button('lx-modal-confirm', modal.confirmText ?? word('@view.ok', language), true, 'accent')));
}

function sheetNode(env) {
  const sheet = sheets[0];
  // Either side compact under touch: a phone in landscape has compact height, as on iOS.
  const compact = (env?.sizeClass ?? 'compact') === 'compact' || (env?.viewport?.height ?? 0) < 600;
  const bottom = compact && env?.pointer !== 'fine';
  const item = (testId, label, index, color) => h('button', { 'data-testid': testId, onClick: () => settleSheet(index), style: {
    paddingTop: 14, paddingBottom: 14, backgroundColor: 'transparent', color, fontSize: 17 } }, label);
  // A centred card elsewhere; a backdrop tap cancels.
  const place = bottom ? { marginTop: 'auto', marginBottom: 8, marginLeft: 8, marginRight: 8 } : { width: 320 };
  return h('dialog', { 'data-testid': 'lx-action-sheet', key: `sheet-${sheet.key}`, open: true, closedby: 'any', onClose: () => settleSheet(-1),
    'aria-label': word('@view.actions', env?.language), style: { ...place, gap: 8, transition: fade } },
    h('view', { style: { borderRadius: 14, backgroundColor: 'surface-overlay', overflow: 'hidden' } },
      ...sheet.items.map((label, index) => h('view', { key: `item-${index}`, style: index ? { borderTopWidth: 1, borderColor: 'line' } : {} },
        item(`lx-action-sheet-item-${index}`, label, index, 'accent')))),
    h('view', { style: { borderRadius: 14, backgroundColor: 'surface-overlay', overflow: 'hidden' } },
      item('lx-action-sheet-cancel', sheet.cancelText ?? word('@view.cancel', env?.language), -1, 'text')));
}

/** The overlay above the page this render, or null. */
export function overlayNode(env) {
  if (!hasOverlay()) return null;
  // Each one draws in the top layer; this box only holds their place in the tree.
  return h('view', { key: 'lx-overlay', style: { position: 'absolute', top: 0, left: 0, width: 0, height: 0 } },
    sheets.length > 0 && sheetNode(env), modals.length > 0 && modalNode(env), toast && toastNode());
}

/** Presence and spin, set on resolved nodes: enter from and leave toward a faded, smaller box. */
export function decorateOverlay(node, env) {
  const enter = lowerStyle('view', { opacity: 0, scale: [0.94, 0.94] }, env).fields;
  const toastEnter = lowerStyle('view', { opacity: 0, scale: [0.8, 0.8] }, env).fields;
  const walk = item => {
    if (['lx-modal', 'lx-action-sheet'].includes(item.props?.testId)) item.states = { ...item.states, startingStyle: enter, exitStyle: enter };
    if (item.props?.testId === 'lx-toast') item.states = { ...item.states, startingStyle: toastEnter, exitStyle: toastEnter };
    if (item.props?.testId === 'lx-toast-icon' && toast?.icon === 'loading') item.fields = { ...item.fields, animation: spin };
    for (const child of item.children ?? []) walk(child);
  };
  walk(node);
  return node;
}

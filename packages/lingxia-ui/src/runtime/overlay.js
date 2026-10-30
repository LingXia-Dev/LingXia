// Feedback the Logic layer asks the page to present (`lx.showToast`, `showModal`,
// `showActionSheet`), drawn by the View itself on desktop GPU pages. Each call answers
// through the same bridge reply the WebView polyfill sends, so Logic sees no difference.
import { h } from './nodes.js';
import { lowerStyle } from '../style/lower.js';
import { closeTopmost } from './toplayer.js';

let toast = null, modals = [], sheets = [], timers = [], nextToken = 1;
const ENTER_MS = 200;

const reply = (id, ok, payload) => JSON.stringify(ok ? { v: 2, kind: 'res', id, ok: true, result: payload ?? null }
  : { v: 2, kind: 'res', id, ok: false, error: payload });
const text = (value, fallback = '') => typeof value === 'string' ? value.slice(0, 4096) : fallback;

/** A view call from Logic: `{changed, replies}`; `replies` are bridge frames to send. */
export function viewRequest(frame) {
  const { id, method } = frame, params = frame.params ?? {};
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
    case 'ui.showModal':
      modals.push({ id, key: nextToken++, title: text(params.title), content: text(params.content), showCancel: params.showCancel !== false,
        cancelText: text(params.cancelText, 'Cancel'), confirmText: text(params.confirmText, 'OK') });
      changed = modals.length === 1;
      break;
    case 'ui.showActionSheet': {
      const items = Array.isArray(params.itemList) ? params.itemList.slice(0, 32).map(item => text(item)) : [];
      if (!items.length) { replies.push(reply(id, false, { code: 'E_INVALID_ARGUMENT', message: 'showActionSheet needs itemList' })); break; }
      sheets.push({ id, key: nextToken++, items, cancelText: text(params.cancelText, 'Cancel') });
      changed = sheets.length === 1;
      break;
    }
    default:
      replies.push(reply(id, false, { code: 'E_UNSUPPORTED', message: `${String(method)} is not served by a GPU View` }));
  }
  return JSON.stringify({ changed, replies });
}

/** The toast timer `token` ran out; true when it hid the toast still showing. */
export function overlayTimer(token) {
  if (!toast || toast.token !== token) return false;
  toast = null;
  return true;
}

/** Escape: the innermost overlay goes, as its cancel would. True when one did. */
export function escapeOverlay() {
  if (sheets.length) { settleSheet(-1); return true; }
  if (modals.length) { settleModal(false); return true; }
  if (toast) { toast = null; return true; }
  return closeTopmost();
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

/** The page went away; its connection, and Logic's calls waiting on it, went with it. */
export function resetOverlays() {
  toast = null; modals = []; sheets = []; timers = []; outbound = [];
}

export const hasOverlay = () => Boolean(toast || modals.length || sheets.length);

// The mobile hosts' native toast (iOS LxAppToast): a dark HUD in every theme, filled status icons.
const RING = 'M14 0a14 14 0 0 1 14 14a14 14 0 0 1-14 14a14 14 0 0 1-14-14a14 14 0 0 1 14-14z';
const icons = {
  success: [{ d: RING, fill: 'success' }, { d: 'M8.5 14.5l3.5 3.5 7.5-7.5', fill: 'none', stroke: '#ffffff', strokeWidth: 2.5, strokeLinecap: 'round', strokeLinejoin: 'round' }],
  error: [{ d: RING, fill: 'danger' }, { d: 'M10 10l8 8M18 10l-8 8', fill: 'none', stroke: '#ffffff', strokeWidth: 2.5, strokeLinecap: 'round' }],
  loading: [{ d: 'M14 3a11 11 0 0 1 11 11', fill: 'none', stroke: 'accent', strokeWidth: 2.5, strokeLinecap: 'round' }],
};
const spin = { name: 'lx-overlay-spin', durationMs: 800, delayMs: 0, iterations: null, whenReduced: 'base', easing: 'linear', opacity: null,
  transforms: [{ kind: 'rotate', angle: [{ offset: 0, value: 0 }, { offset: 1, value: 360 }] }] };
const fade = { properties: ['opacity', 'transform'], durationMs: ENTER_MS, delayMs: 0, easing: [0.4, 0, 0.2, 1] };
const toastFade = { properties: ['opacity', 'transform'], durationMs: 200, delayMs: 0, easing: [0, 0, 0.58, 1] };
const layer = { position: 'absolute', top: 0, left: 0, right: 0, bottom: 0, zIndex: 10000 };

function toastNode(env) {
  const { title, icon, position, mask } = toast;
  const bare = icon === 'none';
  const box = h('column', { 'data-testid': 'lx-toast', key: `toast-${toast.key}`, role: 'status', 'aria-live': 'polite', 'aria-label': title,
    style: { alignItems: 'center', justifyContent: 'center', gap: 12, minWidth: 120, maxWidth: 280, minHeight: bare ? 60 : 100,
      paddingLeft: 20, paddingRight: 20, paddingTop: bare ? 16 : 20, paddingBottom: bare ? 16 : 20, borderRadius: 12,
      backgroundColor: 'shadow/80', transition: toastFade } },
    !bare && h('vector', { 'data-testid': 'lx-toast-icon', 'aria-hidden': true, paths: icons[icon], viewBox: [0, 0, 28, 28], style: { width: 28, height: 28 } }),
    h('text', { style: { color: '#ffffff', fontSize: 16, fontWeight: 500, textAlign: 'center', lineClamp: bare ? 3 : 2 } }, title));
  return h('stack', { style: layer },
    mask && h('button', { 'data-testid': 'lx-toast-mask', 'aria-hidden': true, onClick() {}, style: { ...layer, zIndex: 0, backgroundColor: 'shadow/30' } }),
    h('column', { style: { ...layer, zIndex: 1, alignItems: 'center', justifyContent: position === 'top' ? 'start' : position === 'bottom' ? 'end' : 'center',
      paddingTop: position === 'top' ? 80 : 0, paddingBottom: position === 'bottom' ? 80 : 0 } }, box));
}

function modalNode() {
  const modal = modals[0];
  const button = (testId, label, confirm, color) => h('button', { 'data-testid': testId, onClick: () => settleModal(confirm), style: {
    flexGrow: 1, flexBasis: 0, minWidth: 0, paddingTop: 12, paddingBottom: 12, backgroundColor: 'transparent', color, fontSize: 16, fontWeight: confirm ? 600 : 400 } }, label);
  return h('column', { style: { ...layer, alignItems: 'center', justifyContent: 'center', backgroundColor: 'scrim', padding: 24 } },
    h('column', { 'data-testid': 'lx-modal', key: `modal-${modal.key}`, role: 'dialog', 'aria-label': modal.title || modal.content || 'Dialog',
      style: { width: 320, maxWidth: '100%', borderRadius: 14, backgroundColor: 'surface-raised', overflow: 'hidden', transition: fade } },
      h('column', { style: { padding: 20, gap: 8, alignItems: 'center' } },
        modal.title && h('text', { role: 'heading', style: { fontSize: 17, fontWeight: 600, textAlign: 'center', color: 'text' } }, modal.title),
        modal.content && h('text', { style: { fontSize: 15, textAlign: 'center', color: 'text-muted' } }, modal.content)),
      h('row', { style: { borderTopWidth: 1, borderColor: 'line' } },
        modal.showCancel && button('lx-modal-cancel', modal.cancelText, false, 'text-muted'),
        button('lx-modal-confirm', modal.confirmText, true, 'accent'))));
}

function sheetNode(env) {
  const sheet = sheets[0];
  const phone = (env?.formFactor ?? 'phone') === 'phone';
  const item = (testId, label, index, color) => h('button', { 'data-testid': testId, onClick: () => settleSheet(index), style: {
    paddingTop: 14, paddingBottom: 14, backgroundColor: 'transparent', color, fontSize: 17 } }, label);
  return h('stack', { style: layer },
    h('button', { 'data-testid': 'lx-action-sheet-backdrop', 'aria-hidden': true, onClick: () => settleSheet(-1), style: { ...layer, zIndex: 0, backgroundColor: 'scrim' } }),
    h('column', { style: { ...layer, zIndex: 1, alignItems: 'center', justifyContent: phone ? 'end' : 'center', padding: phone ? 8 : 24 } },
      h('column', { 'data-testid': 'lx-action-sheet', key: `sheet-${sheet.key}`, role: 'dialog', 'aria-label': 'Actions',
        style: { width: phone ? '100%' : 320, gap: 8, transition: fade } },
        h('column', { style: { borderRadius: 14, backgroundColor: 'surface-raised', overflow: 'hidden' } },
          ...sheet.items.map((label, index) => h('column', { key: `item-${index}`, style: index ? { borderTopWidth: 1, borderColor: 'line' } : {} },
            item(`lx-action-sheet-item-${index}`, label, index, 'accent')))),
        h('column', { style: { borderRadius: 14, backgroundColor: 'surface-raised', overflow: 'hidden' } },
          item('lx-action-sheet-cancel', sheet.cancelText, -1, 'text')))));
}

/** The overlay above the page this render, or null. */
export function overlayNode(env) {
  if (!hasOverlay()) return null;
  return h('stack', { key: 'lx-overlay', style: layer },
    sheets.length > 0 && sheetNode(env), modals.length > 0 && modalNode(), toast && toastNode(env));
}

/** Presence and spin, set on resolved nodes: enter from and leave toward a faded, smaller box. */
export function decorateOverlay(node, env) {
  const enter = lowerStyle('column', { opacity: 0, scale: [0.94, 0.94] }, env).fields;
  const toastEnter = lowerStyle('column', { opacity: 0, scale: [0.8, 0.8] }, env).fields;
  const walk = item => {
    if (['lx-modal', 'lx-action-sheet'].includes(item.props?.testId)) item.states = { ...item.states, startingStyle: enter, exitStyle: enter };
    if (item.props?.testId === 'lx-toast') item.states = { ...item.states, startingStyle: toastEnter, exitStyle: toastEnter };
    if (item.props?.testId === 'lx-toast-icon' && toast?.icon === 'loading') item.fields = { ...item.fields, animation: spin };
    for (const child of item.children ?? []) walk(child);
  };
  walk(node);
  return node;
}

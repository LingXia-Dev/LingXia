// Feedback the Logic layer asks the page to present (`lx.showToast`, `showModal`,
// `showActionSheet`), drawn by the View itself on desktop GPU pages. Each call answers
// through the same bridge reply the WebView polyfill sends, so Logic sees no difference.
import { h } from './nodes.js';
import { lowerStyle } from '../style/lower.js';

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

const icons = {
  success: [{ d: 'M18 1a17 17 0 1 1 0 34a17 17 0 1 1 0-34z', fill: 'none', stroke: 'currentColor', strokeWidth: 2 },
    { d: 'M11 18l5 5 9-9', fill: 'none', stroke: 'currentColor', strokeWidth: 2.5, strokeLinecap: 'round', strokeLinejoin: 'round' }],
  error: [{ d: 'M18 1a17 17 0 1 1 0 34a17 17 0 1 1 0-34z', fill: 'none', stroke: 'currentColor', strokeWidth: 2 },
    { d: 'M13 13l10 10M23 13l-10 10', fill: 'none', stroke: 'currentColor', strokeWidth: 2.5, strokeLinecap: 'round' }],
  loading: [{ d: 'M18 4a14 14 0 0 1 14 14', fill: 'none', stroke: 'currentColor', strokeWidth: 2.5, strokeLinecap: 'round' }],
};
const spin = { name: 'lx-overlay-spin', durationMs: 800, delayMs: 0, iterations: null, whenReduced: 'base', easing: 'linear', opacity: null,
  transforms: [{ kind: 'rotate', angle: [{ offset: 0, value: 0 }, { offset: 1, value: 360 }] }] };
const fade = { properties: ['opacity', 'transform'], durationMs: ENTER_MS, delayMs: 0, easing: [0.4, 0, 0.2, 1] };
const layer = { position: 'absolute', top: 0, left: 0, right: 0, bottom: 0, zIndex: 10000 };

function toastNode(env) {
  const { title, icon, position, mask } = toast;
  const spacing = icon === 'none' ? { paddingTop: 12, paddingBottom: 12, paddingLeft: 20, paddingRight: 20 } : { minWidth: 120, padding: 20 };
  const box = h('column', { testId: 'lx-toast', key: `toast-${toast.key}`, role: 'status', accessibilityLiveRegion: 'polite', accessibilityLabel: title,
    style: { alignItems: 'center', gap: 10, maxWidth: 280, ...spacing, borderRadius: 12, backgroundColor: 'text/80', transition: fade } },
    icon !== 'none' && h('vector', { testId: 'lx-toast-icon', accessibilityHidden: true, paths: icons[icon], style: { width: 36, height: 36, color: 'canvas' } }),
    h('text', { style: { color: 'canvas', fontSize: 16, fontWeight: 500, textAlign: 'center' } }, title));
  return h('stack', { style: layer },
    mask && h('button', { testId: 'lx-toast-mask', accessibilityHidden: true, onClick() {}, style: { ...layer, zIndex: 0, backgroundColor: 'transparent' } }),
    h('column', { style: { ...layer, zIndex: 1, alignItems: 'center', justifyContent: position === 'top' ? 'start' : position === 'bottom' ? 'end' : 'center',
      paddingTop: position === 'top' ? 80 : 0, paddingBottom: position === 'bottom' ? 80 : 0 } }, box));
}

function modalNode() {
  const modal = modals[0];
  const button = (testId, label, confirm, color) => h('button', { testId, onClick: () => settleModal(confirm), style: {
    flexGrow: 1, flexBasis: 0, paddingTop: 12, paddingBottom: 12, backgroundColor: 'transparent', color, fontSize: 16, fontWeight: confirm ? 600 : 400 } }, label);
  return h('column', { style: { ...layer, alignItems: 'center', justifyContent: 'center', backgroundColor: 'scrim', padding: 24 } },
    h('column', { testId: 'lx-modal', key: `modal-${modal.key}`, role: 'dialog', accessibilityLabel: modal.title || modal.content || 'Dialog',
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
  const item = (testId, label, index, color) => h('button', { testId, onClick: () => settleSheet(index), style: {
    paddingTop: 14, paddingBottom: 14, backgroundColor: 'transparent', color, fontSize: 17 } }, label);
  return h('stack', { style: layer },
    h('button', { testId: 'lx-action-sheet-backdrop', accessibilityHidden: true, onClick: () => settleSheet(-1), style: { ...layer, zIndex: 0, backgroundColor: 'scrim' } }),
    h('column', { style: { ...layer, zIndex: 1, alignItems: 'center', justifyContent: phone ? 'end' : 'center', padding: phone ? 8 : 24 } },
      h('column', { testId: 'lx-action-sheet', key: `sheet-${sheet.key}`, role: 'dialog', accessibilityLabel: 'Actions',
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
  const walk = item => {
    if (['lx-toast', 'lx-modal', 'lx-action-sheet'].includes(item.props?.testId)) item.states = { ...item.states, startingStyle: enter, exitStyle: enter };
    if (item.props?.testId === 'lx-toast-icon' && toast?.icon === 'loading') item.fields = { ...item.fields, animation: spin };
    for (const child of item.children ?? []) walk(child);
  };
  walk(node);
  return node;
}

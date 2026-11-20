// `useWebView(name)` and `<webview>` runtime state: shared page-scoped web sessions across
// inline boxes and `<dialog>` / sheet overlays, plus bare `<dialog>` fallback for HTML5 fullscreen.
import { h } from './nodes.js';
import { local, cellOf } from './hooks.js';
import { render, page, outsideRender } from './state.js';
import { report } from './actions.js';
import { sited } from './errors.js';

const fields = ['url', 'title', 'error', 'fullscreen'];
const empty = Object.freeze({ url: '', title: '', error: null, fullscreen: false });
const sessions = new WeakMap();
const readers = new Map();
const latest = new Map();
const latestSource = new Map();
const latestLabel = new Map();
const NAME = /^[A-Za-z0-9_.-]{1,64}$/;
const bareSessionMark = Symbol('bare fullscreen webview session');

// One `<webview>` session shown full screen at a time (`"name:<n>"` or `"el:<id>"`), or null.
let fullscreenSession = null;

export function resetWebViews() {
  fullscreenSession = null;
  latest.clear();
  latestSource.clear();
  latestLabel.clear();
}

export const webSessionName = handle => sessions.get(handle);

const same = (a, b) => Object.is(a, b)
  || (typeof a === 'object' && typeof b === 'object' && a !== null && b !== null && JSON.stringify(a) === JSON.stringify(b));

function updateStatus(name, patch) {
  const prev = latest.get(name) ?? empty;
  const session = `name:${name}`;
  const next = Object.freeze({
    url: patch.url !== undefined ? patch.url : prev.url,
    title: patch.title !== undefined ? patch.title : prev.title,
    error: patch.error !== undefined ? (patch.error ? Object.freeze({ ...patch.error }) : null) : prev.error,
    fullscreen: fullscreenSession === session,
  });
  latest.set(name, next);
  const named = readers.get(name);
  for (const reader of named ?? []) {
    const { key, cell } = cellOf(reader.binding);
    if (page.locals.get(key) !== cell) { named.delete(reader); continue; }
    const current = reader.binding.value;
    if (page.updates.has(cell) || [...reader.read].some(field => !same(current[field], next[field]))) reader.binding.set(next);
    else cell.value = next;
  }
}

function setWebFullscreen(session) {
  if (fullscreenSession === session) return;
  const touched = [fullscreenSession, session].filter(s => s !== null);
  fullscreenSession = session;
  page.invalidated = true;
  for (const each of touched) {
    if (each.startsWith('name:')) {
      const name = each.slice(5);
      const prev = latest.get(name) ?? empty;
      const next = Object.freeze({ ...prev, fullscreen: fullscreenSession === each });
      latest.set(name, next);
      for (const reader of readers.get(name) ?? []) {
        const { key, cell } = cellOf(reader.binding);
        if (page.locals.get(key) === cell) reader.binding.set(next);
      }
    }
    for (const handler of page.committed?.webFullscreenHandlers?.get(each) ?? []) {
      try { handler(Object.freeze({ fullscreen: fullscreenSession === each })); }
      catch (error) { report(sited(error)); }
    }
  }
}

export function useWebView(name) {
  if (typeof name !== 'string' || !NAME.test(name))
    throw Error('UI_WEBVIEW: useWebView takes a name of 1..64 letters, digits, "_", "." or "-"');
  const session = `name:${name}`;
  const binding = local(name, latest.get(name) ?? empty, 'webview');
  const { cell } = cellOf(binding);
  let reader = cell.webReader;
  if (!reader) {
    reader = cell.webReader = { binding, read: new Set() };
    if (!readers.has(name)) readers.set(name, new Set());
    readers.get(name).add(reader);
  }
  reader.read.clear();
  const handle = {};
  for (const field of fields) Object.defineProperty(handle, field, { enumerable: true, get() {
    if (render.active) reader.read.add(field);
    return field === 'fullscreen' ? fullscreenSession === session : binding.value[field];
  } });
  Object.assign(handle, {
    requestFullscreen: () => { outsideRender('webview command'); setWebFullscreen(session); return Promise.resolve(); },
    exitFullscreen: () => {
      outsideRender('webview command');
      if (fullscreenSession === session) setWebFullscreen(null);
      return Promise.resolve();
    },
  });
  Object.freeze(handle);
  sessions.set(handle, name);
  return handle;
}

export function shownWebInDialog(session) {
  (render.candidate.webFullscreenDrawn ??= new Set()).add(session);
}

export function webFullscreenDrawn() {
  return fullscreenSession !== null && render.candidate.webFullscreenDrawn?.has(fullscreenSession) === true;
}

export function resolveBareWeb(resolve) {
  const kept = render.candidate.webFullscreenDrawn;
  render.candidate.webFullscreenDrawn = new Set(kept);
  try { return resolve(); } finally { render.candidate.webFullscreenDrawn = kept; }
}

function BareWebFullscreen({ session }) {
  const src = render.candidate.webSources?.get(session) ?? latestSource.get(session);
  const label = latestLabel.get(session) ?? 'Full screen web page';
  if (!src) return null;
  return h('dialog', {
    open: true,
    closedby: 'closerequest',
    onClose: () => { if (fullscreenSession === session) setWebFullscreen(null); },
    'aria-label': label,
    'data-testid': 'lx-fullscreen-webview',
    style: {
      width: '100%', height: '100%', maxWidth: '100%', maxHeight: '100%',
      marginTop: 0, marginBottom: 0, marginLeft: 0, marginRight: 0,
      paddingTop: 0, paddingBottom: 0, paddingLeft: 0, paddingRight: 0,
      borderRadius: 0, backgroundColor: 'surface',
    },
  }, h('webview', {
    [bareSessionMark]: session,
    src,
    'aria-label': label,
    style: { width: '100%', flexGrow: 1 },
  }));
}

/** Bare full-window `<dialog>` when `<webview>` entered full screen and the page drew no dialog for it. */
export function webFullscreenFallback() {
  if (fullscreenSession === null || webFullscreenDrawn()) return null;
  return h(BareWebFullscreen, { session: fullscreenSession, key: `lx-web-fullscreen:${fullscreenSession}` });
}

export function resolveWebView(props, identity, parent, wire, on) {
  const bareSession = props[bareSessionMark];
  const name = props.web === undefined ? undefined : webSessionName(props.web);
  if (props.web !== undefined && name === undefined)
    throw Error('UI_WEBVIEW: web takes the handle useWebView(name) returns');
  const session = bareSession ?? (name === undefined ? `el:${identity}` : `name:${name}`);
  const candidateSources = render.candidate.webSources ??= new Map();
  if (props.src !== undefined) {
    candidateSources.set(session, props.src);
    latestSource.set(session, props.src);
  }
  const src = props.src ?? candidateSources.get(session) ?? latestSource.get(session);
  if (src === undefined)
    throw Error('UI_WEBVIEW: a <webview> needs src, or a web handle another <webview> gives one');
  latestLabel.set(session, props.accessibilityLabel);
  wire('webSrc', src);
  wire('webSession', session);
  if (props.handlesDrags) wire('webHandlesDrags', 1);
  if (parent.inDialog && bareSession === undefined) shownWebInDialog(session);
  if (props.onFullscreenChange !== undefined) {
    const handlers = render.candidate.webFullscreenHandlers ??= new Map();
    if (!handlers.has(session)) handlers.set(session, []);
    handlers.get(session).push(props.onFullscreenChange);
  }
  if (bareSession !== undefined) return;
  on('onWebEvent', payload => {
    const { events } = JSON.parse(payload);
    if (!Array.isArray(events)) return;
    for (const event of events) {
      try {
        switch (event.type) {
          case 'load':
            if (name !== undefined) updateStatus(name, { url: event.url, title: event.title, error: null });
            props.onLoad?.(Object.freeze({ url: event.url, title: event.title }));
            break;
          case 'title':
            if (name !== undefined) updateStatus(name, { title: event.title });
            break;
          case 'message':
            props.onMessage?.(Object.freeze({ data: JSON.parse(event.json), origin: event.origin }));
            break;
          case 'blocked':
            props.onBlocked?.(Object.freeze({ url: event.url }));
            break;
          case 'error':
            if (name !== undefined) updateStatus(name, { error: { code: event.code, message: event.message } });
            props.onError?.(Object.freeze({ code: event.code, message: event.message }));
            break;
          case 'fullscreenchange': {
            const next = Boolean(event.fullscreen);
            if (next) setWebFullscreen(session);
            else if (fullscreenSession === session) setWebFullscreen(null);
            break;
          }
        }
      } catch (error) {
        report(sited(error));
      }
    }
  }, 'text', true);
}

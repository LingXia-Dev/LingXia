import { test } from 'node:test';
import assert from 'node:assert/strict';
import { h, describe, dispatch, reset, useLocal, useWebView } from '../src/internal.js';
import { audit } from '../compiler/index.mjs';
import { decode } from './support/wire.mjs';
import { escapeOverlay } from '../src/runtime/overlay.js';

const by = (list, testId) => list.find(node => node.testId === testId);

test('compiler requires aria-label on <webview>, rejects nesting inside controls, and hints on <iframe>', () => {
  assert.deepEqual(
    audit('export function render() { return <page><webview src="https://example.com"/></page>; }', 'page.tsx'),
    ['page.tsx:1:41: UI_A11Y: <webview> needs an accessible name; add aria-label="…"'],
  );
  assert.deepEqual(
    audit('export function render() { return <page><button aria-label="Open" onClick={go}><webview src="https://example.com" aria-label="Map"/></button></page>; }', 'page.tsx'),
    ['page.tsx:1:41: UI_BUTTON: <button> holds only a <webview>; put the action on that control'],
  );
  assert.deepEqual(
    audit('export function render() { return <page><iframe/></page>; }', 'page.tsx'),
    ['page.tsx:1:41: UI_TAG: unsupported tag <iframe>; use <webview>'],
  );
  assert.deepEqual(
    audit('export function render() { return <page><webview src="https://example.com" aria-label="Map" style={{ width: "100%", height: 240 }}/></page>; }', 'page.tsx'),
    [],
  );
});

test('runtime validates <webview> and useWebView props and refuses children or pressable ancestors', () => {
  reset();
  assert.throws(() => describe(() => h('webview', { 'aria-label': 'Map' }), {}), /UI_WEBVIEW: a <webview> needs src, or a web handle/);
  assert.throws(() => describe(() => h('webview', { src: '', 'aria-label': 'Map' }), {}), /UI_WEBVIEW: invalid source/);
  assert.throws(() => describe(() => h('webview', { src: 'a\0b', 'aria-label': 'Map' }), {}), /UI_WEBVIEW: invalid source/);
  assert.throws(() => describe(() => h('webview', { src: 'https://example.com' }), {}), /UI_WEBVIEW: aria-label is required/);
  assert.throws(() => describe(() => h('webview', { src: 'https://example.com', 'aria-label': '   ' }), {}), /UI_WEBVIEW: aria-label is required/);
  assert.throws(() => describe(() => h('webview', { src: 'https://example.com', 'aria-label': 'Map', web: {} }), {}), /UI_WEBVIEW: web takes the handle useWebView/);
  assert.throws(() => describe(() => { useWebView('bad name'); return h('page', {}); }, {}), /UI_WEBVIEW: useWebView takes a name/);
  assert.throws(() => describe(() => h('webview', { src: 'https://example.com', 'aria-label': 'Map', onMessage: 42 }), {}), /UI_WEBVIEW: onMessage takes a function/);
  assert.throws(() => describe(() => h('webview', { src: 'https://example.com', 'aria-label': 'Map' }, 'child'), {}), /UI_CHILD: <webview> takes no children/);
  assert.throws(
    () => describe(() => h('button', { onClick() {} }, h('text', {}, 'Open'), h('webview', { src: 'https://example.com', 'aria-label': 'Map' })), {}),
    /UI_BUTTON: a <webview> cannot sit inside a pressable control/,
  );
});

test('<webview> fans out load, message, blocked and error events and updates useWebView readers', () => {
  reset();
  const seen = [];
  let handle;
  const page = () => {
    handle = useWebView('terms');
    return h('page', {},
      h('text', { 'data-testid': 'title' }, handle.title || '-'),
      h('webview', {
        'data-testid': 'doc',
        web: handle,
        src: 'docs/terms.html#s2',
        'aria-label': 'Terms of service',
        onLoad: e => seen.push(['load', e]),
        onMessage: e => {
          seen.push(['message', e]);
          if (e.data?.boom) throw Error('handler boom');
        },
        onBlocked: e => seen.push(['blocked', e]),
        onError: e => seen.push(['error', e]),
      }),
    );
  };
  let list = decode(describe(page, {}));
  const web = by(list, 'doc');
  assert.equal(web.tag, 'webview');
  assert.equal(web.props.webSrc, 'docs/terms.html#s2');
  assert.equal(web.props.webSession, 'name:terms');
  assert.equal(web.props.accessibilityLabel, 'Terms of service');

  assert.equal(dispatch(web.props.onWebEvent, JSON.stringify({
    events: [
      { type: 'load', url: 'docs/terms.html#s2', title: 'Terms' },
      { type: 'message', json: JSON.stringify({ boom: true }), origin: 'lxapp' },
      { type: 'message', json: JSON.stringify({ lat: 31.23 }), origin: 'https://maps.example.com' },
      { type: 'blocked', url: 'tel:12345' },
      { type: 'error', code: 'WEBVIEW_MESSAGE_DROPPED', message: 'dropped oldest' },
    ],
  })), true);

  list = decode(describe(page, {}));
  assert.equal(handle.title, 'Terms');
  assert.equal(handle.url, 'docs/terms.html#s2');
  assert.deepEqual(handle.error, { code: 'WEBVIEW_MESSAGE_DROPPED', message: 'dropped oldest' });
  assert.deepEqual(seen, [
    ['load', { url: 'docs/terms.html#s2', title: 'Terms' }],
    ['message', { data: { boom: true }, origin: 'lxapp' }],
    ['message', { data: { lat: 31.23 }, origin: 'https://maps.example.com' }],
    ['blocked', { url: 'tel:12345' }],
    ['error', { code: 'WEBVIEW_MESSAGE_DROPPED', message: 'dropped oldest' }],
  ]);
  for (const [, payload] of seen) assert.equal(Object.isFrozen(payload), true);
});

test('useWebView shares one webSession across an inline card and a 70% <dialog> sheet with no runtime close button', () => {
  reset();
  function MapPicker() {
    const map = useWebView('map');
    const open = useLocal('open', false);
    return h('page', {},
      h('view', { 'data-testid': 'card' },
        h('webview', {
          'data-testid': 'inline-map',
          web: map,
          src: 'https://maps.example.com/pick',
          'aria-label': 'Location picker',
        }),
        h('button', { 'data-testid': 'expand', onClick: () => open.set(true) }, 'Expand'),
      ),
      h('dialog', {
        open: open.value,
        closedby: 'any',
        onClose: () => open.set(false),
        'aria-label': 'Pick address',
        'data-testid': 'sheet',
        style: { width: '100%', height: '70%', marginTop: 'auto', marginBottom: 0 },
      },
        h('webview', {
          'data-testid': 'sheet-map',
          web: map,
          'aria-label': 'Location picker',
        }),
      ),
    );
  }
  let list = decode(describe(MapPicker, {}));
  assert.equal(by(list, 'sheet'), undefined);
  assert.equal(by(list, 'inline-map').props.webSession, 'name:map');

  dispatch(by(list, 'expand').props.onClick);
  list = decode(describe(MapPicker, {}));
  assert.ok(by(list, 'sheet'));
  assert.equal(by(list, 'sheet-map').props.webSession, 'name:map');
  assert.equal(by(list, 'sheet-map').props.webSrc, 'https://maps.example.com/pick');
  assert.equal(by(list, 'lx-webview-close'), undefined);
  assert.equal(by(list, 'lx-fullscreen-webview'), undefined);

  assert.equal(escapeOverlay(), true);
  list = decode(describe(MapPicker, {}));
  assert.equal(by(list, 'sheet'), undefined);
});

test('web-initiated HTML5 fullscreen uses a bare <dialog> fallback with no extra runtime controls unless the page draws its own', () => {
  reset();
  const changes = [];
  const page = () => h('page', {},
    h('webview', {
      'data-testid': 'player',
      src: 'https://video.example.com/embed',
      'aria-label': 'Embedded player',
      onFullscreenChange: ({ fullscreen }) => changes.push(fullscreen),
    }),
  );
  let list = decode(describe(page, {}));
  const web = by(list, 'player');
  assert.equal(by(list, 'lx-fullscreen-webview'), undefined);

  dispatch(web.props.onWebEvent, JSON.stringify({ events: [{ type: 'fullscreenchange', fullscreen: true }] }));
  list = decode(describe(page, {}));
  assert.ok(by(list, 'lx-fullscreen-webview'));
  assert.equal(by(list, 'lx-webview-close'), undefined);
  assert.equal(list.filter(n => n.tag === 'webview').length, 2);
  assert.equal(
    list.filter(n => n.tag === 'webview')[0].props.webSession,
    list.filter(n => n.tag === 'webview')[1].props.webSession,
  );

  assert.equal(escapeOverlay(), true);
  list = decode(describe(page, {}));
  assert.equal(by(list, 'lx-fullscreen-webview'), undefined);
  assert.deepEqual(changes, [true, false]);
});

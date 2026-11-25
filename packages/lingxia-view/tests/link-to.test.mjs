import { test } from 'node:test';
import assert from 'node:assert/strict';
import { configureActions, describe, h, dispatch, takeActionRequests, receiveActionResponse, takeActionErrors } from '../src/internal.js';
import { rendered } from './support/wire.mjs';

const link = to => () => h('text', {}, h('link', { to }, 'go'));

test('a link with to asks Logic to navigate and reports a refusal', async () => {
  configureActions({ __names: [], __modes: {} });
  const to = { page: 'detail', params: { id: 7 } };
  const { props } = rendered(link(to)).children[0];
  assert.equal(dispatch(props.onClick), true);
  const frame = JSON.parse(takeActionRequests()[0]);
  assert.equal(frame.method, 'nav.go');
  assert.deepEqual(frame.params, to);

  receiveActionResponse({ v: 2, kind: 'res', id: frame.id, ok: false, error: { code: 'E_NOT_FOUND', message: 'page name: detail' } });
  for (let i = 0; i < 4; i++) await Promise.resolve();
  assert.match(takeActionErrors()[0] ?? "", /detail/);
});

test('a malformed to fails at render', () => {
  const throwsOn = (to, pattern) => assert.throws(() => describe(link(to), {}), pattern);
  throwsOn({ page: 'a', tab: 'b' }, /exactly one/);
  throwsOn({ back: 0 }, /positive integer/);
  throwsOn({ url: '' }, /non-empty string/);
  assert.throws(() => describe(() => h('text', {}, h('link', { to: { tab: 'home' }, onClick() {} }, 'x')), {}), /not two/);
  describe(link({ app: 'other', page: 'home' }), {});
});

test('a block link with role="menuitem" navigates and closes its menu; elsewhere it keeps the link role', async () => {
  const { prepare, commit, reset } = await import('../src/internal.js');
  const { decode } = await import('./support/wire.mjs');
  reset();
  configureActions({ __names: [], __modes: {} });
  const toggled = [];
  const view = () => h('page', {},
    h('button', { popoverTarget: 'menu', 'data-testid': 'more', 'aria-label': 'More' }),
    h('view', { id: 'menu', popover: true, role: 'menu', onToggle: open => toggled.push(open), 'data-testid': 'menu' },
      h('link', { to: { page: 'settings' }, role: 'menuitem', 'data-testid': 'settings' }, h('text', {}, 'Settings'))));
  const render = () => { const nodes = decode(prepare(view, {})); commit(); return nodes; };
  let nodes = render();
  dispatch(nodes.find(node => node.testId === 'more').props.onClick);
  nodes = render();
  const row = nodes.find(node => node.testId === 'settings');
  assert.equal(row.props.role, 'menuitem');
  dispatch(row.props.onClick);
  assert.equal(JSON.parse(takeActionRequests()[0]).method, 'nav.go');
  assert.deepEqual(toggled, [true, false], 'the menu closed');
  assert.throws(() => describe(() => h('page', {}, h('link', { to: { page: 'a' }, role: 'heading' }, h('text', {}, 'A'))), {}), /a block <link> takes only role="menuitem"/);
  assert.throws(() => describe(() => h('text', {}, h('link', { to: { page: 'a' }, role: 'menuitem' }, 'A')), {}), /UI_PROP/);
});

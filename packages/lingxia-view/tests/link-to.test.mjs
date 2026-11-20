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

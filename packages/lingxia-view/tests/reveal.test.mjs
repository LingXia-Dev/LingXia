import { test } from 'node:test';
import assert from 'node:assert/strict';
import { h, prepare, commit, dispatch, reset, reveal } from '../src/internal.js';
import { takeReveals } from '../src/runtime/reveal.js';
import { decode } from './support/wire.mjs';

test('reveal queues element ids from a handler for the host to take once', () => {
  reset();
  const view = () => h('page', {}, h('button', { 'data-testid': 'go', onClick() { reveal('row-7'); reveal('row-9'); } }, 'Go'));
  const nodes = decode(prepare(view, { env: {} })); commit();
  assert.equal(takeReveals(), '[]');
  dispatch(nodes.find(node => node.testId === 'go').props.onClick);
  assert.equal(takeReveals(), '["row-7","row-9"]');
  assert.equal(takeReveals(), '[]', 'taken once');
});

test('reveal refuses a render and a value that is not an id; a reset drops what waited', () => {
  reset();
  assert.throws(() => prepare(() => { reveal('x'); return h('page', {}); }, { env: {} }), /UI_RENDER: reveal during render/);
  assert.throws(() => reveal(''), /UI_ID: reveal takes an element id/);
  assert.throws(() => reveal(7), /UI_ID: reveal takes an element id/);
  reveal('kept');
  reset();
  assert.equal(takeReveals(), '[]');
});

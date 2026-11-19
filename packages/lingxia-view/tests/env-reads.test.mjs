import { test } from 'node:test';
import assert from 'node:assert/strict';
import { h, prepare, commit, reset, createView, configureClasses } from '../src/internal.js';
import { envReads } from '../src/runtime/hooks.js';

const env = { theme: 'light', highContrast: false, sizeClass: 'compact', viewport: { width: 300, height: 500 } };
const run = page => { prepare(page, createView({}, env, 0)); commit(); return envReads(); };

test('a page of role colours reads neither the appearance nor the host context', () => {
  reset(); configureClasses([{ name: 'card', when: {}, style: { backgroundColor: 'surface' } }]);
  assert.equal(run(() => h('page', {}, h('view', { className: 'card' }), h('text', {}, 'x'))), 0);
});

test('reading env.theme or env.highContrast marks the appearance, and stays marked while the page lives', () => {
  reset(); configureClasses([]);
  assert.equal(run(({ env }) => h('page', {}, h('text', {}, env.theme))), 2);
  // A later render that does not reach the read (a recalled component would not) keeps the mark.
  assert.equal(run(() => h('page', {}, h('text', {}, 'x'))), 2);
  reset();
  assert.equal(run(({ env }) => h('page', {}, h('text', {}, String(env.highContrast)))), 2);
  reset();
  assert.equal(run(({ env }) => h('page', {}, h('text', {}, env.sizeClass))), 1, 'host context is its own bit');
  reset();
  assert.equal(run(() => h('page', {}, h('text', {}, 'x'))), 0, 'a reset page starts unmarked');
});

test('env.capsule is null when the host does not send it, and the host\'s box when it does', () => {
  reset(); configureClasses([]);
  const shown = view => { let seen; prepare(({ env }) => { seen = env.capsule; return h('page', {}); }, view); commit(); return seen; };
  assert.equal(shown(createView({}, env, 0)), null);
  const capsule = { x: 280, y: 6, width: 87, height: 32 };
  assert.deepEqual({ ...shown(createView({}, { ...env, capsule }, 0)) }, capsule);
});

test('a class with a theme condition marks the appearance whether or not it applies', () => {
  reset(); configureClasses([{ name: 'dark:mark', when: { theme: 'dark' }, style: { backgroundColor: '#ff0000' } }]);
  assert.equal(run(() => h('page', {}, h('text', {}, 'x'))), 2);
  configureClasses([]);
  assert.equal(run(() => h('page', {}, h('text', {}, 'x'))), 0);
});

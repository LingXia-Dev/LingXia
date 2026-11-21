import { test, beforeEach } from 'node:test';
import assert from 'node:assert/strict';
import { h, prepare, commit, reset, createView, registerCatalogues } from '../src/internal.js';
import { t } from '../src/index.js';

beforeEach(() => reset());

const all = { root: { start: 0, end: 3, leading: 0, trailing: 0 } };
const f = (a, f) => f.j([f.p(a.n, 0, false, { '=0': () => f.j(['none']), one: h => f.j([f.n(h, ''), ' item']), other: h => f.j([f.n(h, ''), ' items']) })]);
const zh = (a, f) => f.j([f.n(a.n, ''), ' 项']);

// The build's formatter is the crate's; this one shows only which language `t` asked for.
const formatter = tag => ({
  j: parts => parts.join(''), n: value => String(value),
  p: (value, offset, ordinal, cases) => (cases[`=${value}`] ?? cases[tag.startsWith('en') && value === 1 ? 'one' : 'other'] ?? cases.other)(value - offset),
});

function say(env, key, args) {
  let said;
  prepare(() => { said = t(key, args); return h('page', {}); }, createView({}, env), all);
  commit();
  return said;
}

test('t speaks the page language, narrowed to a catalogue, else the source', () => {
  registerCatalogues({ source: 'en-US', formatter, languages: { 'en-US': { 'cart.items': f, 'cart.only': () => 'English only' }, 'zh-CN': { 'cart.items': zh } } });
  assert.equal(say({ contentLanguage: 'en-US' }, 'cart.items', { n: 1234 }), '1234 items');
  assert.equal(say({ contentLanguage: 'en-GB' }, 'cart.items', { n: 1 }), '1 item');
  assert.equal(say({ contentLanguage: 'en-US' }, 'cart.items', { n: 0 }), 'none');
  assert.equal(say({ contentLanguage: 'zh-Hans-CN' }, 'cart.items', { n: 2 }), '2 项');
  assert.equal(say({ contentLanguage: 'ja-JP' }, 'cart.items', { n: 2 }), '2 items');
  assert.equal(say({ contentLanguage: 'zh-CN' }, 'cart.only'), 'English only');
  assert.throws(() => say({ contentLanguage: 'en-US' }, 'cart.gone'), /UI_I18N: no message 'cart.gone'/);
  assert.throws(() => t('cart.items'), /UI_SCOPE/);
});

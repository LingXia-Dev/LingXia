import { test, beforeEach } from 'node:test';
import assert from 'node:assert/strict';
import { h, prepare, commit, reset, createView, registerCatalogues, dispatch, takeHandlerFailure } from '../src/internal.js';
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
  assert.equal(say({ language: 'en-US' }, 'cart.items', { n: 1234 }), '1234 items');
  assert.equal(say({ language: 'en-GB' }, 'cart.items', { n: 1 }), '1 item');
  assert.equal(say({ language: 'en-US' }, 'cart.items', { n: 0 }), 'none');
  assert.equal(say({ language: 'zh-Hans-CN' }, 'cart.items', { n: 2 }), '2 项');
  assert.equal(say({ language: 'ja-JP' }, 'cart.items', { n: 2 }), '2 items');
  assert.equal(say({ language: 'zh-CN' }, 'cart.only'), 'English only');
  assert.throws(() => say({ language: 'en-US' }, 'cart.gone'), /UI_I18N: no message 'cart.gone'/);
});

test('a handler calls t after an await, in the shown language', async () => {
  registerCatalogues({ source: 'en-US', formatter, languages: { 'en-US': { 'cart.items': f }, 'zh-CN': { 'cart.items': zh } } });
  assert.throws(() => t('cart.items', { n: 1 }), /UI_SCOPE: t\('cart.items'\) before the page's first frame; call it in render or an event handler/);
  let said;
  const records = prepare(() => h('button', { onClick: async () => { await Promise.resolve(); said = t('cart.items', { n: 2 }); } }, 'go'), createView({}, { language: 'zh-CN' }), all);
  commit();
  assert.equal(dispatch(records[7]), true);
  for (let i = 0; i < 4; i++) await Promise.resolve();
  assert.equal(takeHandlerFailure(), '');
  assert.equal(said, '2 项');
});

test("the View's words follow the language; an lxapp's @view key overrides one", async () => {
  const { word } = await import('../src/runtime/words.js');
  registerCatalogues({ source: 'en-US', formatter, languages: { 'en-US': { '@view.ok': () => 'Got it' }, 'zh-CN': { '@view.done': () => '好了' } } });
  assert.equal(word('@view.ok', 'en-US'), 'Got it');
  assert.equal(word('@view.ok', 'zh-CN'), '确定', 'an override in one language leaves the others');
  assert.equal(word('@view.cancel', 'zh-CN'), '取消');
  assert.equal(word('@view.done', 'zh-CN'), '好了');
  assert.equal(word('@view.done', 'de-AT'), 'Fertig');
  assert.equal(word('@view.done', 'sw'), 'Done', 'a language LingXia does not translate reads English');
});

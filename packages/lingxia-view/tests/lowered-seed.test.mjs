import { test, before, after } from 'node:test';
import assert from 'node:assert/strict';
import { compileUtilities } from '../compiler/tailwind.mjs';
import { configureClasses } from '../src/style/classes.js';
import { h, describe, takeLowered, seedLowered } from '../src/internal.js';
import { decode } from './support/wire.mjs';

const classes = ['font-bold', 'flex', 'flex-col', 'gap-2', 'p-4', 'md:p-8', 'text-sm', 'font-bold', 'items-center', 'rounded-lg', 'text-muted', 'bg-surface', 'disabled:opacity-40'];
before(async () => configureClasses(await compileUtilities(new Set(classes))));
after(() => configureClasses([]));

const page = () => h('view', { className: 'flex flex-col gap-2 p-4' },
  h('text', { className: 'text-sm font-bold' }, 'title'),
  h('button', { className: 'flex items-center rounded-lg disabled:opacity-40', disabled: true }, h('text', { className: 'text-muted bg-surface' }, 'row')));
const env = width => ({ env: { appearance: 'light', viewport: { width, height: 800 } } });
const wire = view => decode(describe(page, view)).map(({ tag, text, props }) => ({ tag, text, props }));

test('a seed from another isolate renders the same wire without lowering again', () => {
  const first = wire(env(301));
  const taken = takeLowered();
  assert.notEqual(taken, '');
  assert.equal(takeLowered(), '', 'nothing new since the last take');
  wire(env(302));
  assert.equal(seedLowered(taken), true);
  takeLowered();
  assert.deepEqual(wire(env(301)), first);
  assert.equal(takeLowered(), '', 'every lowering came from the seed');
});

test('a seed for another environment is not used', () => {
  wire(env(311));
  const taken = takeLowered();
  wire(env(312));
  assert.equal(seedLowered(taken), true);
  takeLowered();
  wire(env(313));
  assert.notEqual(takeLowered(), '');
});

test('a malformed seed is refused and changes nothing', () => {
  for (const text of ['', 'nope', '{}', '{"env":1,"entries":[]}', '{"env":"x","entries":[["k"]]}', '{"env":"x","entries":[[1,{}]]}', '{"env":"x","entries":[["k",null]]}'])
    assert.equal(seedLowered(text), false, text);
  const before = wire(env(321));
  takeLowered();
  wire(env(322));
  assert.deepEqual(wire(env(321)), before);
});

test('an equal inline style reuses its lowering; a changed value lowers anew', () => {
  const styled = grow => () => h('view', { className: 'flex flex-col gap-2' }, h('view', { className: 'p-4', style: { flexGrow: grow, minWidth: '12ch' } }));
  const view = env(331);
  const at = grow => decode(describe(styled(grow), view)).map(({ props }) => props);
  const two = at(2);
  takeLowered();
  assert.deepEqual(at(2), two);
  assert.equal(takeLowered(), '', 'the same style came from the cache');
  assert.notDeepEqual(at(3), two);
  assert.notEqual(takeLowered(), '', 'a new value is lowered');
});

test('a lowered set too large to keep is not serialized', () => {
  const many = count => () => h('view', {}, ...Array.from({ length: count }, (_, i) =>
    h('view', { style: { fontSize: 8 + i / 100 } }, h('text', { className: 'font-bold' }, 'x'))));
  describe(many(10), env(331));
  assert.notEqual(takeLowered(), '');
  describe(many(401), env(332));
  assert.equal(takeLowered(), '');
});

test('a seed survives the class table its module configures before seeding', async () => {
  const first = wire(env(351));
  const taken = takeLowered();
  assert.notEqual(taken, '');
  // A new isolate of the module: its evaluation configures the same table, then the host seeds it.
  configureClasses(await compileUtilities(new Set(classes)));
  assert.equal(seedLowered(taken), true);
  assert.deepEqual(wire(env(351)), first);
  assert.equal(takeLowered(), '', 'every lowering came from the seed');
});

test('a new class table drops what the old one lowered, and the old seed with it', () => {
  configureClasses([{ name: 'p-2', when: {}, style: { paddingTop: 8 } }]);
  const padding = () => decode(describe(() => h('view', { className: 'p-2' }), env(341)))[0].props.paddingTop;
  assert.equal(padding(), 8);
  const taken = takeLowered();
  configureClasses([{ name: 'p-2', when: {}, style: { paddingTop: 12 } }]);
  assert.equal(padding(), 12);
  assert.equal(seedLowered(taken), true);
  configureClasses([{ name: 'p-2', when: {}, style: { paddingTop: 16 } }]);
  assert.equal(padding(), 16, 'a seed taken under an older table is not applied');
});

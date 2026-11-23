import { test } from 'node:test';
import assert from 'node:assert/strict';
import { h, prepare, commit, reset, createView, useView, SizeClassScope, useBackHandler } from '../src/internal.js';
import { compileUtilities } from '../compiler/tailwind.mjs';
import { configureClasses } from '../src/style/classes.js';
import { decode } from './support/wire.mjs';

const render = (view, env = {}, viaCreateView = true) => {
  const input = viaCreateView ? createView({}, env, 0) : { env };
  const out = decode(prepare(view, input));
  commit();
  return out;
};
const byId = (nodes, id) => nodes.find(node => node.testId === id);

test('regular: variant follows env.sizeClass and nearest SizeClassScope', async () => {
  reset();
  configureClasses(await compileUtilities(['grid', 'grid-cols-1', 'regular:grid-cols-2']));

  function Probe({ testId }) {
    const { env } = useView();
    return h('view', { 'data-testid': testId, className: 'grid grid-cols-1 regular:grid-cols-2' },
      h('text', { 'data-testid': `${testId}-sc` }, env.sizeClass));
  }

  const view = () => h('page', {},
    h(Probe, { testId: 'outer' }),
    h(SizeClassScope, { value: 'compact' }, h(Probe, { testId: 'inner' })),
  );

  const regularNodes = render(view, { sizeClass: 'regular', viewport: { width: 800, height: 600 } });
  assert.equal(byId(regularNodes, 'outer-sc').text, 'regular');
  assert.equal(byId(regularNodes, 'inner-sc').text, 'compact');
  assert.equal(byId(regularNodes, 'outer').props.gridTemplateColumns, 'repeat(2, minmax(0, 1fr))');
  assert.equal(byId(regularNodes, 'inner').props.gridTemplateColumns, 'repeat(1, minmax(0, 1fr))');

  const compactNodes = render(view, { sizeClass: 'compact', viewport: { width: 393, height: 852 } });
  assert.equal(byId(compactNodes, 'outer-sc').text, 'compact');
  assert.equal(byId(compactNodes, 'outer').props.gridTemplateColumns, 'repeat(1, minmax(0, 1fr))');
});

test('bleed is allowed on direct <page> children and rejected on nested nodes', () => {
  reset();
  const valid = () => h('page', { title: 'Map' },
    h('view', { bleed: true, 'data-testid': 'hero' }),
    h('view', { bleed: 'top', 'data-testid': 'top' }),
  );
  const nodes = render(valid, { sizeClass: 'compact' });
  assert.equal(nodes[0].props.pageTitle, 'Map');
  assert.equal(byId(nodes, 'hero').props.bleed, 'all');
  assert.equal(byId(nodes, 'top').props.bleed, 'top');

  const invalid = () => h('page', {},
    h('view', {}, h('view', { bleed: true })),
  );
  assert.throws(() => render(invalid, { sizeClass: 'compact' }), /UI_BLEED_PARENT/);
});

test('env.insets exposes logical inlineStart and inlineEnd for LTR and RTL', () => {
  reset();
  const view = () => {
    const { env } = useView();
    return h('page', {}, h('text', { 'data-testid': 'edges' },
      `${env.insets.inlineStart},${env.insets.inlineEnd}`));
  };
  const ltr = render(view, { direction: 'ltr', insets: { top: 0, right: 20, bottom: 0, left: 10 } });
  assert.equal(byId(ltr, 'edges').text, '10,20');

  const rtl = render(view, { direction: 'rtl', insets: { top: 0, right: 20, bottom: 0, left: 10 } }, false);
  assert.equal(byId(rtl, 'edges').text, '20,10');
});

test('useBackHandler validates arguments cleanly', () => {
  reset();
  const ok = () => {
    useBackHandler(true, () => {});
    return h('page', {});
  };
  assert.doesNotThrow(() => render(ok, { sizeClass: 'compact' }));

  const badEnabled = () => {
    useBackHandler('yes', () => {});
    return h('page', {});
  };
  assert.throws(() => render(badEnabled, { sizeClass: 'compact' }), /UI_BACK/);
});

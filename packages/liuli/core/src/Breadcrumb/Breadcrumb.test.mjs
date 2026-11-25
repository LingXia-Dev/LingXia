import { test } from 'node:test';
import assert from 'node:assert/strict';
import { textOf } from '@lingxia/view/testing';
import { renderExample } from '../testing/harness.mjs';

test('Path: a named list of levels; the ones above are buttons, the current page is plain and heavier', async () => {
  const screen = await renderExample('Breadcrumb/Breadcrumb', 'Path');
  const trail = screen.getByTestId('path');
  assert.deepEqual([trail.props.role, trail.props.accessibilityLabel], ['list', 'Breadcrumb']);
  const items = trail.children;
  assert.deepEqual(items.map(item => item.props.role), ['listitem', 'listitem', 'listitem']);
  assert.equal(screen.getByTestId('path-projects').tag, 'button');
  const current = screen.getByTestId('path-plan');
  assert.deepEqual([current.tag, current.text, current.props.color, current.props.fontWeight], ['text', 'Q3 plan', 'text', 500]);
  assert.equal(items[0].children.filter(node => node.tag === 'vector').length, 0, 'no separator before the first level');
  assert.ok(items[1].children[0].tag === 'vector' && items[1].children[0].props.accessibilityHidden === 1, 'a chevron, not read');
  screen.press(screen.getByTestId('path-projects'));
  screen.getByText('Opened Projects');
});

test('Deep: past four levels the middle ones fold into a named menu; the first and the last two stay', async () => {
  const screen = await renderExample('Breadcrumb/Breadcrumb', 'Deep');
  const trail = screen.getByTestId('deep');
  assert.equal(trail.children.length, 3);
  assert.deepEqual(['deep-drive', 'deep-invoices', 'deep-inv'].map(id => textOf(screen.getByTestId(id))), ['Drive', 'Invoices', 'INV-1043.pdf']);
  const more = screen.getByTestId('deep-more');
  assert.equal(more.props.accessibilityLabel, 'More levels');
  screen.press(more);
  assert.deepEqual(['deep-clients', 'deep-northwind', 'deep-2026'].map(id => screen.getByTestId(id).props.accessibilityLabel ?? textOf(screen.getByTestId(id))), ['Clients', 'Northwind', '2026']);
});

test('Pages: a level with to is a block link to its page; the current level is text', async () => {
  const screen = await renderExample('Breadcrumb/Breadcrumb', 'Pages');
  assert.equal(screen.getByTestId('pages-home').tag, 'link-box');
  assert.equal(screen.getByTestId('pages-settings').tag, 'link-box');
  assert.equal(screen.getByTestId('pages-privacy').tag, 'text');
});

test('to: a folded level with to is a link row in the menu', async () => {
  const { renderPage } = await import('../testing/harness.mjs');
  const screen = await renderPage(`
    import { Breadcrumb } from '@liulidesign/core/Breadcrumb';
    const page = (key) => ({ key, label: key, to: { page: key } });
    export function render() {
      return <page><Breadcrumb label="Breadcrumb" moreLabel="More levels" items={[page('a'), page('b'), page('c'), page('d'), page('e'), { key: 'f', label: 'f' }]} data-testid="t" /></page>;
    }
  `);
  screen.press(screen.getByTestId('t-more'));
  const folded = screen.getByTestId('t-c');
  assert.equal(folded.tag, 'link-box');
  assert.equal(folded.props.role, 'menuitem');
});

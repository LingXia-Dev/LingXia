import { test } from 'node:test';
import assert from 'node:assert/strict';
import { renderExample } from '../testing/harness.mjs';

// Lays the line out as a host does: each item at its own width, the line at `width`, its natural width the children and 8 pt gaps.
function layOut(screen, testId, width, widthOf) {
  // Each layout renders again and replaces the nodes, so every child is found afresh.
  const own = screen.getByTestId(testId).children.map(child => widthOf(child));
  own.forEach((width, index) => screen.layout(screen.getByTestId(testId).children[index], { width, height: 28, minContentWidth: width, maxContentWidth: width }));
  const natural = own.reduce((sum, each) => sum + each, 0) + 8 * Math.max(0, own.length - 1);
  screen.layout(screen.getByTestId(testId), { width, height: 28, minContentWidth: 0, maxContentWidth: natural });
}
const textIn = node => (node.text ?? '') + node.children.map(textIn).join('');
const shown = (screen, testId) => screen.getByTestId(testId).children.map(textIn);

test('Tags: measured on the first frame with the count unseen, then the tags that fit and a count of the rest', async () => {
  const screen = await renderExample('OverflowList/OverflowList', 'Tags');
  const line = screen.getByTestId('tags');
  assert.equal(line.props.role, 'group');
  assert.equal(line.children.length, 9, 'every tag and the stand-in count');
  assert.equal(line.children[8].props.opacity, 0, 'the stand-in is unseen');
  assert.ok(line.children[8].props.accessibilityHidden, 'and unread');
  // Each tag 80 pt and the count 40: in 300 pt, three tags (3 × 80 + 3 × 8 gaps + 40 = 304 > 300, so two).
  layOut(screen, 'tags', 300, node => (/^\+/.test(textIn(node)) ? 40 : 80));
  assert.deepEqual(shown(screen, 'tags'), ['Design', 'Research', 'Mobile', '+5'].slice(0, 2).concat(['+6']));
  assert.equal(screen.getByTestId('tags').children.at(-1).props.opacity, undefined, 'the count is seen once the fit is known');
});

test('Tags: a line wide enough for all of them draws no count; FromStart keeps the end of the path', async () => {
  const screen = await renderExample('OverflowList/OverflowList', 'Tags');
  layOut(screen, 'tags', 2000, node => (/^\+/.test(textIn(node)) ? 40 : 80));
  assert.equal(shown(screen, 'tags').length, 8);
  assert.ok(!shown(screen, 'tags').some(text => text.startsWith('+')));
  const path = await renderExample('OverflowList/OverflowList', 'FromStart');
  layOut(path, 'path', 200, node => (textIn(node) === '…' ? 20 : 60));
  assert.deepEqual(shown(path, 'path'), ['…', '/Components', '/OverflowList'], 'the ellipsis first, where it is read; 20 + 2 × (60 + 8) fits 200');
});

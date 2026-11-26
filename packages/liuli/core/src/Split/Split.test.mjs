import { test } from 'node:test';
import assert from 'node:assert/strict';
import { renderExample } from '../testing/harness.mjs';
import { textOf } from '@lingxia/view/testing';

const regularWide = { env: { sizeClass: 'regular', viewport: { width: 1024, height: 768 } } };
const tabletopFold = {
  env: {
    sizeClass: 'regular',
    viewport: { width: 868, height: 784 },
    hinge: { x: 0, y: 380, width: 868, height: 24, orientation: 'horizontal', posture: 'half-opened' },
  },
};
const compact = { env: { sizeClass: 'compact', viewport: { width: 393, height: 852 } } };

test('Split regular: docks companion aside at inline end', async () => {
  const screen = await renderExample('Split/Split', 'LiveStreamSplit', regularWide);
  assert.equal(screen.getByTestId('live-split').props.flexDirection, 'row');
  assert.equal(screen.getByTestId('live-split-aside').props.width, 300);
});

test('Split tabletop fold: places primary on upper leaf and aside on lower leaf', async () => {
  const screen = await renderExample('Split/Split', 'LiveStreamSplit', tabletopFold);
  assert.equal(screen.getByTestId('live-split').props.flexDirection, 'column');
  // 784 tall: the lower leaf runs from the crease's far edge (380 + 24) to the bottom.
  assert.equal(screen.getByTestId('live-split-aside').props.height, 784 - 380 - 24);
});

test('Split compact="sheet": hides inline aside and opens BottomSheet on trigger press', async () => {
  const screen = await renderExample('Split/Split', 'DocumentSheetSplit', compact);
  assert.equal(screen.queryAll(n => n.testId === 'doc-split-aside').length, 0);
  screen.press(screen.getByTestId('doc-open-comments'));
  assert.ok(screen.getByTestId('doc-split-sheet'));
});

test('Split compact="sheet" opens its sheet full, so a reply field at the aside\'s bottom is in view', async () => {
  const screen = await renderExample('Split/Split', 'DocumentSheetSplit', compact);
  screen.press(screen.getByTestId('doc-open-comments'));
  const drag = JSON.parse(screen.getByTestId('doc-split-sheet').props.drag);
  assert.deepEqual([drag.points, drag.index], [[0, '45%', '100%'], 0]);
  assert.ok(screen.getByTestId('doc-reply'));
});

test('The aside is a group named by asideLabel; on a phone its sheet takes the same name', async () => {
  const wide = await renderExample('Split/Split', 'LiveStreamSplit', regularWide);
  const aside = wide.getByTestId('live-split-aside');
  assert.deepEqual([aside.props.role, aside.props.accessibilityLabel], ['group', 'Live chat']);
  const screen = await renderExample('Split/Split', 'DocumentSheetSplit', compact);
  screen.press(screen.getByTestId('doc-open-comments'));
  assert.equal(screen.getByTestId('doc-split-sheet').props.accessibilityLabel, 'Comments');
});

test('the sheet is named by asideLabel and draws no title of its own: the companion\'s heading is the one shown', async () => {
  const screen = await renderExample('Split/Split', 'DocumentSheetSplit', compact);
  screen.press(screen.getByTestId('doc-open-comments'));
  const sheet = screen.getByTestId('doc-split-sheet');
  assert.equal(sheet.props.accessibilityLabel, 'Comments');
  const headings = [];
  const walk = node => { if (node?.props?.role === 'heading') headings.push(textOf(node)); (node?.children ?? []).forEach(walk); };
  walk(sheet);
  assert.deepEqual(headings, ['Comments'], 'one heading, the companion\'s own');
});


test("Resizable: the aside's edge drags its width between its bounds, steps with the keys, and reports where it ended", async () => {
  const screen = await renderExample('Split/Split', 'Resizable', regularWide);
  screen.layout(screen.getByTestId('resizable'), { width: 900, height: 400 });
  const handle = () => screen.getByTestId('resizable-handle');
  const aside = () => screen.getByTestId('resizable-aside');
  assert.equal(aside().props.width, 300);
  assert.equal(handle().props.accessibilityLabel, 'Resize Comments');
  const at = { width: 12, height: 400, y: 100 };
  screen.pointer(handle(), { ...at, phase: 'down', x: 6, pointer: 'mouse' });
  screen.pointer(handle(), { ...at, phase: 'move', x: -54, pointer: 'mouse' });
  assert.equal(aside().props.width, 360, 'a drag toward the main content widens the aside');
  screen.pointer(handle(), { ...at, phase: 'move', x: -900, pointer: 'mouse' });
  assert.equal(aside().props.width, 480, 'no wider than its bound');
  screen.pointer(handle(), { ...at, phase: 'up', x: 206, pointer: 'mouse' });
  assert.ok(screen.getByText('Comments kept at 240 pt'), 'no narrower than its bound, reported on release');
  screen.pointer(handle(), { ...at, phase: 'increment', x: 0, pointer: 'keyboard' });
  assert.equal(aside().props.width, 256, 'a key steps it by 16 pt');
});

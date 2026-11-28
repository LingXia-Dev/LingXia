import { test } from 'node:test';
import assert from 'node:assert/strict';
import { renderExample } from './testing/harness.mjs';

// Every example the reference and the gallery show compiles and draws, on a phone and on a desktop.
const examples = {
  JinshuView: ['Lesson', 'Resume', 'Course', 'Attachments', 'Unavailable', 'Loading', 'Specimen'],
  JinshuEditor: ['Draft', 'AskAgent', 'ReadOnly', 'Specimen'],
  JinshuDiff: ['Revision', 'Inline', 'Block', 'NoChanges', 'Specimen'],
  JinshuOutline: ['Progress', 'BesideTheDocument', 'InPopover', 'Specimen'],
};
const desk = { sizeClass: 'regular', pointer: 'fine', viewport: { width: 1280, height: 900 } };

for (const [name, exports] of Object.entries(examples))
  test(`${name}: every example renders`, async () => {
    for (const example of exports)
      for (const env of [undefined, desk]) {
        const screen = await renderExample(`${name}/${name}`, example, env && { env });
        assert.ok(screen, `${example}${env ? ' (desktop)' : ''}`);
      }
  });

test('Diff: inline words a text edit; block shows every change before and after', async () => {
  const befores = screen => screen.queryAll(node => node.text === 'Before').length;
  assert.equal(befores(await renderExample('JinshuDiff/JinshuDiff', 'Inline')), 1, 'only the callout, whose change has no words');
  assert.equal(befores(await renderExample('JinshuDiff/JinshuDiff', 'Block')), 2);
});

test('Editor: a block menu hands the block to the app\'s agent', async () => {
  const screen = await renderExample('JinshuEditor/JinshuEditor', 'AskAgent', { env: desk });
  screen.fire(screen.queryAll(n => String(n.props.elementId ?? n.props.id ?? '').endsWith('-r-n2'))[0], 'onPointerEnter');
  screen.press(screen.queryAll(n => n.props.accessibilityLabel === 'Block actions')[0]);
  let node = screen.getByText('Ask agent');
  while (node.tag !== 'button') node = node.parent;
  screen.press(node);
  assert.ok(screen.getByText('Handed to the assistant: n2'));
});

test('a formula spaces its relations thick and its binary operators medium, a sign and a script tight', async () => {
  const screen = await renderExample('JinshuView/JinshuView', 'Lesson');
  // The lesson's one formula: lim_{x→1} (x²−1)/(x−1) = 2.
  const ops = screen.queryAll(node => node.tag === 'text' && ['=', '−', '→'].includes(node.text));
  const margin = value => ops.filter(node => node.text === value).map(node => node.props.marginInlineStart ?? node.props.marginLeft ?? 0);
  assert.deepEqual(margin('='), [5], 'a relation: a thick space');
  // A minus is U+2212 over a plus laid out unseen, so it is the plus's width.
  const minuses = screen.queryAll(node => node.tag === 'text' && node.text === '+').map(node => node.parent);
  assert.deepEqual(minuses.map(node => node.props.marginInlineStart ?? node.props.marginLeft ?? 0), [3, 3], 'a binary operator in a fraction: a medium space at its size');
  assert.ok(minuses.every(node => node.children[1].text === '−' && node.children[1].props.position === 'absolute'));
  assert.deepEqual(margin('→'), [0], 'under lim, a script: none');
});

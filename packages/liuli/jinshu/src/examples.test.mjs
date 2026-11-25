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

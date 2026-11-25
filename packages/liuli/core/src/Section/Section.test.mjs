import { test } from 'node:test';
import assert from 'node:assert/strict';
import { textOf } from '@lingxia/view/testing';
import { renderExample } from '../testing/harness.mjs';

test('Recent: a level-2 heading with its line, the action centred on the title\'s first line, then the content', async () => {
  const screen = await renderExample('Section/Section', 'Recent');
  const [header, content] = screen.getByTestId('recent').children;
  const [heading, action] = header.children;
  assert.deepEqual([heading.children[0].props.role, heading.children[0].props.headingLevel, heading.children[0].text], ['heading', 2, 'Recent notes']);
  assert.equal(heading.children[1].text, 'Edited in the last week');
  assert.equal(textOf(action.children[1]), 'See all');
  assert.equal(action.props.shrink, 0, 'the action keeps its width; the heading gives way');
  // Level with the title's first line, not the description's last, at any text size: a strut in the title's style
  // makes the box one title line tall, and the controls, with no height of their own, centre on it.
  const [strut, controls] = action.children;
  const title = heading.children[0];
  assert.deepEqual([strut.props.fontSize, strut.props.lineHeight, strut.props.width, strut.props.accessibilityHidden], [title.props.fontSize, title.props.lineHeight, 0, 1]);
  assert.deepEqual([header.props.align, action.props.align, controls.props.height, controls.props.align], ['start', 'center', 0, 'center']);
  assert.equal(content.props.role, 'list');
});

test('Nested: an inner section\'s heading is a level down and smaller', async () => {
  const screen = await renderExample('Section/Section', 'Nested');
  const title = id => screen.getByTestId(id).children[0].children[0].children[0];
  assert.deepEqual([title('outer').props.headingLevel, title('inner').props.headingLevel], [2, 3]);
  assert.ok(title('outer').props.fontSize > title('inner').props.fontSize);
});

test('Recent at large text: the action stands under the heading, a link\'s text flush with the title', async () => {
  for (const [textScale, isStacked] of [[1.4, false], [1.5, true], [2, true]]) {
    const screen = await renderExample('Section/Section', 'Recent', { env: { textScale } });
    const [header] = screen.getByTestId('recent').children;
    const [heading, action] = header.children;
    assert.equal(header.props.flexDirection === 'row', !isStacked, `the header at ${textScale}`);
    if (!isStacked) continue;
    assert.equal(heading.props.alignSelf, 'stretch');
    const link = action.children[0];
    assert.equal(textOf(link), 'See all');
    assert.equal(link.props.paddingInlineStart ?? link.props.paddingLeft, 0, 'no padding before the text');
  }
});

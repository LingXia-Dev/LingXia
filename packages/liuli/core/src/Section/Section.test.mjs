import { test } from 'node:test';
import assert from 'node:assert/strict';
import { textOf } from '@lingxia/view/testing';
import { renderExample } from '../testing/harness.mjs';

test('Recent: a level-2 heading with its line, the action at the end of that line, then the content', async () => {
  const screen = await renderExample('Section/Section', 'Recent');
  const [header, content] = screen.getByTestId('recent').children;
  const [heading, action] = header.children;
  assert.deepEqual([heading.children[0].props.role, heading.children[0].props.headingLevel, heading.children[0].text], ['heading', 2, 'Recent notes']);
  assert.equal(heading.children[1].text, 'Edited in the last week');
  assert.equal(textOf(action), 'See all');
  assert.equal(action.props.shrink, 0, 'the action keeps its width; the heading gives way');
  assert.equal(content.props.role, 'list');
});

test('Nested: an inner section\'s heading is a level down and smaller', async () => {
  const screen = await renderExample('Section/Section', 'Nested');
  const title = id => screen.getByTestId(id).children[0].children[0].children[0];
  assert.deepEqual([title('outer').props.headingLevel, title('inner').props.headingLevel], [2, 3]);
  assert.ok(title('outer').props.fontSize > title('inner').props.fontSize);
});

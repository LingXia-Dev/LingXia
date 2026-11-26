import { test } from 'node:test';
import assert from 'node:assert/strict';
import { renderExample } from '../testing/harness.mjs';

const find = (node, test) => (test(node) ? [node] : node.children.flatMap(child => find(child, test)));

test('Price: the group is named once; the select rounds the start, the field the end, and the field takes the room', async () => {
  const screen = await renderExample('InputGroup/InputGroup', 'Price');
  const group = screen.getByTestId('price');
  assert.equal(group.props.role, 'group');
  assert.equal(group.props.accessibilityLabel, 'Price');
  const select = screen.getByTestId('currency');
  const amount = screen.getByTestId('amount');
  assert.equal(select.props.accessibilityLabel, 'Currency', 'each control keeps its own name');
  assert.equal(amount.props.accessibilityLabel, 'Amount');
  assert.equal(find(group, node => node.tag === 'text' && node.text === 'Amount').length, 0, 'no visible label inside the row');
  const box = amount.parent;
  // Square everywhere, then round on the group's own ends only, by logical corner.
  const corners = node => [node.props.radius, node.props.radiusStartStart, node.props.radiusEndStart, node.props.radiusStartEnd, node.props.radiusEndEnd];
  assert.deepEqual(corners(select), [0, 10, 10, undefined, undefined], 'the select rounds the start and is square where it meets the field');
  assert.deepEqual(corners(box), [0, undefined, undefined, 10, 10], 'the field rounds the end');
  const field = group.children[1];
  assert.equal(field.props.marginInlineStart, -1, 'the field draws its start edge over the select\'s');
  assert.equal(field.props.grow, 1);
});

test('Website and Sizes: fixed text rounds the end it sits at and takes the group\'s size; a joined button is secondary', async () => {
  const site = await renderExample('InputGroup/InputGroup', 'Website');
  const scheme = site.getByText('https://').parent;
  assert.deepEqual([scheme.props.radius, scheme.props.radiusStartStart, scheme.props.radiusStartEnd], [0, 10, undefined], 'round at the start it sits at');
  const sizes = await renderExample('InputGroup/InputGroup', 'Sizes');
  const [small, medium] = sizes.queryAll(node => node.tag === 'text' && node.text === 'kg');
  assert.ok(small.props.fontSize < medium.props.fontSize, 'the small group\'s unit is set smaller');
  const invite = await renderExample('InputGroup/InputGroup', 'Invite');
  invite.press(invite.getByTestId('send'));
  invite.getByText('Add an email first');
});

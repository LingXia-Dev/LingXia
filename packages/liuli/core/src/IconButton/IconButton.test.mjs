import { test } from 'node:test';
import assert from 'node:assert/strict';
import { textOf } from '@lingxia/view/testing';
import { described, renderExample, renderPage } from '../testing/harness.mjs';

const icon = node => node.children.find(child => child.tag === 'vector');

test('Toolbar: a button named by its label, with nothing drawn but the icon; a press runs it', async () => {
  const screen = await renderExample('IconButton/IconButton', 'Toolbar');
  const like = () => screen.getByTestId('like');
  assert.equal(like().tag, 'button');
  assert.equal(like().props.accessibilityLabel, 'Like');
  assert.equal(textOf(like()), '', 'no visible text');
  assert.deepEqual(like().children.map(child => child.tag), ['vector']);
  assert.equal(icon(like()).props.accessibilityHidden, 1);
  assert.deepEqual([like().props.width, like().props.height], [40, 40]);
  assert.equal(like().props.background, '#00000000', 'quiet by default');
  screen.press(like());
  screen.press(like());
  assert.equal(textOf(screen.getByTestId('likes')), '2 likes');
});

test('Variants: Button\'s fills, with the icon in the fill\'s text colour', async () => {
  const screen = await renderExample('IconButton/IconButton', 'Variants');
  assert.deepEqual([screen.getByTestId('add').props.background, screen.getByTestId('add').props.color], ['accent', 'on-accent']);
  assert.equal(screen.getByTestId('quiet').props.background, '#00000000');
});

test('Sizes: 32, 40 and 48 points square, the icon 16, 18 and 20', async () => {
  const screen = await renderExample('IconButton/IconButton', 'Sizes');
  const sizes = ['small', 'medium', 'large'].map(id => screen.getByTestId(id));
  assert.deepEqual(sizes.map(button => [button.props.width, button.props.height]), [[32, 32], [40, 40], [48, 48]]);
  assert.deepEqual(sizes.map(button => icon(button).props.width), [16, 18, 20]);
  assert.ok(sizes.every(button => button.props.shrink === 0));
});

test('states: busy shows the spinner under the same name; disabled reads its reason', async () => {
  const busy = (await renderExample('IconButton/IconButton', 'StateBusy')).getByTestId('busy');
  assert.equal(busy.props.accessibilityLabel, 'Refresh');
  assert.equal(busy.props.stateBusy, 1);
  assert.match(icon(busy).props.animation, /"name":"spin"/);
  const screen = await renderExample('IconButton/IconButton', 'StateDisabled');
  const disabled = screen.getByTestId('disabled');
  assert.equal(disabled.props.onClick, undefined, 'a disabled button takes no press');
  assert.deepEqual(described(screen, disabled), ['Nothing new to load']);
});

test('InRows: each row\'s button is named with its row, and removes it', async () => {
  const screen = await renderExample('IconButton/IconButton', 'InRows');
  const remove = screen.getByTestId('remove-Notes.txt');
  assert.equal(remove.props.accessibilityLabel, 'Remove Notes.txt');
  assert.equal(remove.props.width, 32);
  screen.press(remove);
  assert.equal(screen.queryAll(node => node.props.role === 'listitem').length, 2);
});

test('edge: a trailing icon button hangs its inset past the edge, so the drawing meets the content edge', async () => {
  const screen = await renderPage(`
    import { IconButton } from '@liulidesign/core/IconButton';
    export function render() { return <page><IconButton label="More" icon="more" size="md" edge="end" data-testid="more" /></page>; }
  `);
  assert.equal(screen.getByTestId('more').props.marginInlineEnd, -14.75);
});

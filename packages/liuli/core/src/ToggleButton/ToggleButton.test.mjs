import { test } from 'node:test';
import assert from 'node:assert/strict';
import { textOf } from '@lingxia/view/testing';
import { renderExample, renderPage } from '../testing/harness.mjs';

test('Filters: each toggle is a named button reporting pressed; a press flips it and the list', async () => {
  const screen = await renderExample('ToggleButton/ToggleButton', 'Filters');
  const unread = () => screen.getByTestId('unread');
  assert.equal(unread().tag, 'button');
  assert.equal(unread().props.accessibilityLabel, 'Unread');
  assert.equal(unread().props.statePressed, 0);
  assert.equal(textOf(screen.getByTestId('count')), '4 notes');
  screen.press(unread());
  assert.equal(unread().props.statePressed, 1);
  assert.equal(unread().props.background, 'accent-soft');
  assert.equal(textOf(screen.getByTestId('count')), '2 notes');
  screen.press(screen.getByTestId('starred'));
  assert.equal(textOf(screen.getByTestId('count')), '1 note');
  screen.press(unread());
  assert.equal(unread().props.statePressed, 0);
  assert.equal(unread().props.accessibilityLabel, 'Unread', 'the name does not change with the state');
});

test('StateDisabledOn: disabled keeps the pressed state but takes no press', async () => {
  const screen = await renderExample('ToggleButton/ToggleButton', 'StateDisabledOn');
  const button = screen.queryAll(node => node.tag === 'button')[0];
  assert.equal(button.props.statePressed, 1);
  assert.equal(button.props.onClick, undefined);
});

test('Sizes: heights come from the control tokens', async () => {
  const screen = await renderExample('ToggleButton/ToggleButton', 'Sizes');
  assert.deepEqual(screen.queryAll(node => node.tag === 'button').map(node => node.props.minHeight), [32, 40, 48]);
});

test('off: the page\'s surface on a page, the overlay\'s in a sheet\'s footer; on keeps its fill', async () => {
  const screen = await renderPage(`
    import { ToggleButton } from '@liulidesign/core/ToggleButton';
    import { BottomSheet } from '@liulidesign/core/BottomSheet';
    export function render() { return <page>
      <ToggleButton label="Bold" value={false} onChange={() => {}} data-testid="page" />
      <BottomSheet isOpen onOpenChange={() => {}} label="Format" actions={<><ToggleButton label="Italic" value={false} onChange={() => {}} data-testid="off" /><ToggleButton label="Mono" value onChange={() => {}} data-testid="on" /></>} />
    </page>; }`);
  assert.equal(screen.getByTestId('page').props.background, 'surface');
  assert.equal(screen.getByTestId('off').props.background, 'surface-overlay');
  assert.notEqual(screen.getByTestId('on').props.background, 'surface-overlay');
});

test('Icons: icon-only toggles are named by their labels; pressedIcon swaps the picture, not the name', async () => {
  const screen = await renderExample('ToggleButton/ToggleButton', 'Icons');
  const mute = () => screen.getByTestId('icons-mute');
  assert.equal(mute().props.accessibilityLabel, 'Mute');
  assert.equal(JSON.stringify(mute().children).includes('Mute'), false, 'the label is not drawn');
  const before = JSON.stringify(mute().children);
  screen.press(mute());
  assert.equal(mute().props.statePressed, 1);
  assert.notEqual(JSON.stringify(mute().children), before, 'the icon changes');
  assert.equal(mute().props.accessibilityLabel, 'Mute');
  assert.equal(mute().props.width, mute().props.height, 'a square');
  assert.ok(textOf(screen.getByTestId('icons-starred')).includes('Starred'));
});

test('Toolbar: a ghost toggle has no edge or fill while off, like the icon button beside it; on, the soft accent fill', async () => {
  const screen = await renderExample('ToggleButton/ToggleButton', 'Toolbar');
  const italic = screen.getByTestId('toolbar-italic');
  const link = screen.getByTestId('toolbar-link');
  assert.equal(italic.props.background, link.props.background);
  assert.equal(italic.props.borderColor, link.props.borderColor);
  assert.equal(italic.props.shadow, undefined);
  assert.equal(screen.getByTestId('toolbar-bold').props.background, 'accent-soft');
  screen.press(italic);
  assert.equal(screen.getByTestId('toolbar-italic').props.statePressed, 1);
  assert.equal(screen.getByTestId('toolbar-italic').props.background, 'accent-soft');
});

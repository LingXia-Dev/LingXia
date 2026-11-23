import { test } from 'node:test';
import assert from 'node:assert/strict';
import { renderExample } from '../testing/harness.mjs';

const dialogs = screen => screen.queryAll(node => node.tag === 'dialog');

test('FromGrid: closed renders nothing; a thumbnail opens it at its picture, named and counted', async () => {
  const screen = await renderExample('Lightbox/Lightbox', 'FromGrid');
  assert.equal(dialogs(screen).length, 0);
  screen.press(screen.getByTestId('open-2'));
  const viewer = screen.getByTestId('viewer');
  assert.deepEqual([viewer.props.accessibilityLabel, viewer.props.widthPercent, viewer.props.heightPercent, viewer.props.background], ['Photos of the house', 100, 100, 'media-canvas']);
  screen.getByText('3 of 4');
  screen.getByText('Dusk over the ridge');
  const pictures = screen.queryAll(node => node.tag === 'image' && node.props.imageFit === 'contain');
  assert.equal(pictures.length, 4, 'every picture is shown whole');
});

test('FromGrid: a dot moves to its picture and its caption; the close button closes', async () => {
  const screen = await renderExample('Lightbox/Lightbox', 'FromGrid');
  screen.press(screen.getByTestId('open-0'));
  screen.press(screen.getByTestId('viewer-dots-1'));
  screen.getByText('2 of 4');
  screen.getByText('Fields behind the house');
  const close = screen.getByTestId('viewer-close');
  assert.deepEqual([close.props.accessibilityLabel, close.props.width], ['Close', 44]);
  screen.press(close);
  assert.equal(dialogs(screen).length, 0);
});

test('Single: one picture has no count and no dots', async () => {
  const screen = await renderExample('Lightbox/Lightbox', 'Single');
  screen.press(screen.getByTestId('one-open'));
  assert.equal(screen.queryAll(node => node.props.role === 'tablist').length, 0);
});

test('FromGrid: opens at the thumbnail pressed through `index`, and the scroll snaps there', async () => {
  const screen = await renderExample('Lightbox/Lightbox', 'FromGrid');
  screen.press(screen.getByTestId('open-2'));
  const scroller = screen.queryAll(node => node.tag === 'scroll')[0];
  assert.equal(scroller.props.snapIndex, 2);
  assert.equal(scroller.props.bind, undefined);
});

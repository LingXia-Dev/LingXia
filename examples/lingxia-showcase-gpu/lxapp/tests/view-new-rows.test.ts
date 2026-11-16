import { expect, spec } from '@lingxia/test';
import { openDemo } from './open-demo';

// A popover placed beside its anchor: at the inline end, on the other side when that has no
// room, and under the anchor when neither side has.
spec('a popover opens beside its button, flips sides, and falls back under a wide one', { covers: ['view:popover'], start: openDemo('beside') }, async (t) => {
  const view = t.app.view;
  const note = view.testId('lab-beside-note');
  const state = view.testId('lab-beside-state');
  const screen = await view.viewport();
  const near = (a: number, b: number) => Math.abs(a - b) <= 1.5;
  // Level with its button, or as low as the window lets it sit.
  const level = (anchor: { y: number }, box: { height: number }) => Math.min(anchor.y, screen.top + screen.height - 8 - box.height);

  await view.testId('lab-beside-start').click();
  await expect(state).toHaveAttribute('data-state', 'open=start');
  let [anchor, box] = [await view.testId('lab-beside-start').boundingBox(), await note.boundingBox()];
  const fitsAfter = anchor.x + anchor.width + 4 + box.width <= screen.width - 8;
  if (fitsAfter) {
    expect(near(box.x, anchor.x + anchor.width + 4)).toBe(true);
    expect(near(box.y, level(anchor, box))).toBe(true);
  }

  await view.testId('lab-beside-end').click();
  await expect(state).toHaveAttribute('data-state', 'open=end');
  [anchor, box] = [await view.testId('lab-beside-end').boundingBox(), await note.boundingBox()];
  // No room past the end button: the note sits before it.
  expect(near(box.x + box.width, anchor.x - 4)).toBe(true);
  expect(near(box.y, level(anchor, box))).toBe(true);

  await view.testId('lab-beside-wide').click();
  await expect(state).toHaveAttribute('data-state', 'open=wide');
  [anchor, box] = [await view.testId('lab-beside-wide').boundingBox(), await note.boundingBox()];
  // Room on neither side: under the button, never over it.
  expect(near(box.y, anchor.y + anchor.height + 4)).toBe(true);
  expect(near(box.x + box.width / 2, anchor.x + anchor.width / 2)).toBe(true);
});

spec('a parent checkbox reads mixed while some of its boxes are checked, and a tap checks them all', { covers: ['view:checkbox'], start: openDemo('partly') }, async (t) => {
  const view = t.app.view;
  const all = view.testId('lab-partly-all');
  const state = view.testId('lab-partly-state');
  await expect(state).toHaveAttribute('data-state', 'checked=0 of=3');
  expect(await all.getAttribute('aria-checked')).toBe(null);
  await view.testId('lab-partly-1').click();
  await expect(state).toHaveAttribute('data-state', 'checked=1 of=3');
  await expect(all).toHaveAttribute('aria-checked', 'mixed');
  await all.click();
  await expect(state).toHaveAttribute('data-state', 'checked=3 of=3');
  await expect(all).toBeChecked();
  await all.click();
  await expect(state).toHaveAttribute('data-state', 'checked=0 of=3');
});

spec('a page with a disabled link becomes ready; the link takes no press and its neighbour does', { covers: ['view:link'], start: openDemo('linkOff') }, async (t) => {
  const view = t.app.view;
  const state = view.testId('lab-link-state');
  await expect(view.testId('lab-link-disabled')).toBeDisabled();
  await expect(view.testId('lab-link-enabled')).toBeEnabled();
  await view.testId('lab-link-enabled').click();
  await expect(state).toHaveAttribute('data-state', 'taps=1');
});

spec('a small control reports the box a finger reaches', { covers: ['view:touch-target'], start: openDemo('small') }, async (t) => {
  const view = t.app.view;
  const button = view.testId('lab-small-button');
  const [box, target] = [await button.boundingBox(), await button.touchTarget()];
  expect(Math.round(box.width)).toBe(24);
  console.log('ROWS touchTarget ' + JSON.stringify({ box, target }));
  // Grown around the centre for a finger; the drawn box for a mouse.
  expect(target.width === box.width || target.width >= 44).toBe(true);
  expect(Math.abs(target.x + target.width / 2 - (box.x + box.width / 2))).toBeLessThanOrEqual(0.5);
});

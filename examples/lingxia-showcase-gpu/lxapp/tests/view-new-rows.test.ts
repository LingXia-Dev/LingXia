import { expect, test } from '@lingxia/test';
import { openDemo } from './open-demo';

// A popover placed beside its anchor: at the inline end, on the other side when that has no
// room, and under the anchor when neither side has.
test('a popover opens beside its button, flips sides, and falls back under a wide one', { covers: ['view:popover'], start: openDemo('beside') }, async (t) => {
  const view = t.app.view;
  const note = view.testId('lab-beside-note');
  const state = view.testId('lab-beside-state');
  const screen = await view.viewport();
  const near = (a: number, b: number) => Math.abs(a - b) <= 1.5;
  // Level with its button; beside one at the bottom edge it is pushed up to stay on screen, still beside it.
  const level = (anchor: { y: number }, box: { y: number; height: number }) => near(box.y, anchor.y)
    || (box.y < anchor.y && box.y + box.height > anchor.y && box.y + box.height <= screen.top + screen.height - 8 + 1.5);

  await view.testId('lab-beside-start').click();
  await expect(state).toHaveAttribute('data-state', 'open=start');
  let [anchor, box] = [await view.testId('lab-beside-start').boundingBox(), await note.boundingBox()];
  const fitsAfter = anchor.x + anchor.width + 4 + box.width <= screen.width - 8;
  if (fitsAfter) {
    expect(near(box.x, anchor.x + anchor.width + 4)).toBe(true);
    expect(level(anchor, box)).toBe(true);
  }

  await view.testId('lab-beside-end').click();
  await expect(state).toHaveAttribute('data-state', 'open=end');
  [anchor, box] = [await view.testId('lab-beside-end').boundingBox(), await note.boundingBox()];
  // No room past the end button: the note sits before it.
  expect(near(box.x + box.width, anchor.x - 4)).toBe(true);
  expect(level(anchor, box)).toBe(true);

  await view.testId('lab-beside-wide').click();
  await expect(state).toHaveAttribute('data-state', 'open=wide');
  [anchor, box] = [await view.testId('lab-beside-wide').boundingBox(), await note.boundingBox()];
  // Room on neither side: under the button, or above it at the bottom edge; never covering it.
  expect(near(box.y, anchor.y + anchor.height + 4) || near(box.y + box.height, anchor.y - 4)).toBe(true);
  expect(near(box.x + box.width / 2, anchor.x + anchor.width / 2)).toBe(true);
});

// A callout keeps its own margin from its anchor on either side, and a translate moves it after placement.
test('a callout sits its margin clear of its anchor above or below, and a translate moves it further', { covers: ['view:popover'], start: openDemo('gap') }, async (t) => {
  const view = t.app.view;
  const note = view.testId('lab-gap-note');
  const state = view.testId('lab-gap-state');
  const target = view.testId('lab-gap-target');
  const near = (a: number, b: number) => Math.abs(a - b) <= 1.5;
  const centred = (anchor: { x: number; width: number }, box: { x: number; width: number }) => near(box.x + box.width / 2, anchor.x + anchor.width / 2);

  await view.testId('lab-gap-below').click();
  await expect(state).toHaveAttribute('data-state', 'side=below nudged=false');
  let [anchor, box] = [await target.boundingBox(), await note.boundingBox()];
  // Under the target by its 12 pt margin, unless the screen's bottom flips it over with the same gap.
  expect(near(box.y, anchor.y + anchor.height + 12) || near(box.y + box.height, anchor.y - 12)).toBe(true);
  expect(centred(anchor, box)).toBe(true);

  await view.testId('lab-gap-nudge').click();
  await expect(state).toHaveAttribute('data-state', 'side=below nudged=true');
  const nudged = await note.boundingBox();
  expect(near(nudged.y - box.y, 8)).toBe(true);

  await view.testId('lab-gap-nudge').click();
  await view.testId('lab-gap-above').click();
  await expect(state).toHaveAttribute('data-state', 'side=above nudged=false');
  [anchor, box] = [await target.boundingBox(), await note.boundingBox()];
  expect(near(box.y + box.height, anchor.y - 12) || near(box.y, anchor.y + anchor.height + 12)).toBe(true);
  expect(centred(anchor, box)).toBe(true);
  // Both carets ride the callout's edges, centred on it; `side-*:` shows the one facing the box.
  for (const [id, edge] of [['lab-gap-caret-up', box.y], ['lab-gap-caret-down', box.y + box.height]] as const) {
    const caret = await view.testId(id).boundingBox();
    expect(near(caret.y + caret.height / 2, edge)).toBe(true);
    expect(near(caret.x + caret.width / 2, box.x + box.width / 2)).toBe(true);
  }
});

test('a parent checkbox reads mixed while some of its boxes are checked, and a tap checks them all', { covers: ['view:checkbox'], start: openDemo('partly') }, async (t) => {
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

// An email and a URL wider than their box wrap inside it, over several lines.
test('a long email and URL wrap inside a narrow box', { covers: ['view:text'], start: openDemo('addresses') }, async (t) => {
  const view = t.app.view;
  const box = await view.testId('lab-addresses-box').boundingBox();
  for (const id of ['lab-addresses-email', 'lab-addresses-link']) {
    const text = await view.testId(id).boundingBox();
    expect(text.x + text.width).toBeLessThanOrEqual(box.x + box.width + 0.5);
    expect(text.height).toBeGreaterThan(40);
  }
});

test('a page with a disabled link becomes ready; the link takes no press and its neighbour does', { covers: ['view:link'], start: openDemo('linkOff') }, async (t) => {
  const view = t.app.view;
  const state = view.testId('lab-link-state');
  await expect(view.testId('lab-link-disabled')).toBeDisabled();
  await expect(view.testId('lab-link-enabled')).toBeEnabled();
  await view.testId('lab-link-enabled').click();
  await expect(state).toHaveAttribute('data-state', 'taps=1');
});

test('a small control reports the box a finger reaches', { covers: ['view:touch-target'], start: openDemo('small') }, async (t) => {
  const view = t.app.view;
  const button = view.testId('lab-small-button');
  const [box, target] = [await button.boundingBox(), await button.touchTarget()];
  expect(Math.round(box.width)).toBe(24);
  console.log('ROWS touchTarget ' + JSON.stringify({ box, target }));
  // Grown around the centre for a finger; the drawn box for a mouse.
  expect(target.width === box.width || target.width >= 44).toBe(true);
  expect(Math.abs(target.x + target.width / 2 - (box.x + box.width / 2))).toBeLessThanOrEqual(0.5);
});

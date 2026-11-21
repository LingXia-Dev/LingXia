import { expect, test } from '@lingxia/test';
import { openDemo } from './open-demo';

// `onPointer` on the scrub plot: twelve months across its width, the marker on the one under x.
test('a pointer scrubs along x and a vertical drag from the plot still scrolls the page', { covers: ['view:pointer', 'view:scroll'], start: openDemo('pointer') }, async (t) => {
  const view = t.app.view;
  const plot = view.testId('lab-scrub-plot');
  const state = view.testId('lab-scrub-state');
  await expect(state).toHaveAttribute('data-state', /at=3 last=none/);
  await t.waitFor(() => state.getAttribute('data-state').then(value => value ?? ''), { until: text => !text.includes('box=none') });
  const box = await plot.boundingBox();
  const middle = box.height / 2;
  await view.scrub(plot, { from: [20, middle], to: [box.width - 10, middle], durationMs: 600 });
  await expect(state).toHaveAttribute('data-state', /at=11 last=up/);
  await view.scrub(plot, { from: [box.width - 10, middle], to: [20, middle], durationMs: 400 });
  await expect(state).toHaveAttribute('data-state', /at=0 last=up/);
  // Along y the gesture is the page's: a finger scrolls it and the plot hears cancel. On a
  // desktop the swipe is a wheel scroll, not a press, and the plot hears nothing.
  const touch = (await state.getAttribute('data-state').then(value => value ?? '')).includes('last=up touch');
  const centre: [number, number] = [box.x + box.width / 2, box.y + middle];
  await view.swipe({ from: centre, to: [centre[0], centre[1] - 160], durationMs: 400 });
  if (touch) {
    await expect(state).toHaveAttribute('data-state', /last=cancel/);
    await t.waitFor(() => plot.boundingBox(), { until: now => now.y < box.y - 40 });
  } else {
    await expect(state).toHaveAttribute('data-state', /at=0 last=up mouse/);
  }
});

test('arrow keys and assistive input step the scrub plot in place', { covers: ['view:pointer', 'view:a11y'], start: openDemo('pointer') }, async (t) => {
  const view = t.app.view;
  const plot = view.testId('lab-scrub-plot');
  const state = view.testId('lab-scrub-state');
  await expect(state).toHaveAttribute('data-state', /at=3/);
  await plot.press('ArrowRight');
  await expect(state).toHaveAttribute('data-state', /at=4 last=increment keyboard/);
  await plot.press('ArrowLeft');
  await plot.press('ArrowLeft');
  await expect(state).toHaveAttribute('data-state', /at=2 last=decrement keyboard/);
  // The x axis leaves Up and Down alone.
  await plot.press('ArrowUp');
  await expect(state).toHaveAttribute('data-state', /at=2/);
  await plot.performAction('increment');
  await expect(state).toHaveAttribute('data-state', /at=3 last=increment keyboard/);
  // A window key takes a bare name as well as the options object.
  await t.app.window.key.press('Tab');
  await t.app.window.key.press({ key: 'Tab', modifiers: ['shift'] });
  await expect(state).toHaveAttribute('data-state', /at=3/);
});

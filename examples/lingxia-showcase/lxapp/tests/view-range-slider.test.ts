import { expect, test } from '@lingxia/test';
import { openDemo } from './open-demo';

// Two thumbs on one track: a press moves the nearer one, keys move the focused one, and neither passes the other.
test('a slider with two thumbs moves the nearer one, stops at the other, and reports where a press ended', { covers: ['view:slider'], start: openDemo('range') }, async (t) => {
  const view = t.app.view;
  const state = view.testId('lab-range-state');
  const lower = view.testId('lab-range-price-lower');
  const upper = view.testId('lab-range-price-upper');
  await expect(state).toHaveAttribute('data-state', 'low=20 high=60 done=none');
  // Assistive tech meets two sliders, each with its own name, inside a named group.
  await expect(view.getByRole('slider', { name: 'Lowest price' })).toHaveCount(1);
  await expect(view.getByRole('slider', { name: 'Highest price' })).toHaveCount(1);

  // Keys move the focused thumb by a step and stop at the other thumb.
  await upper.press('ArrowLeft');
  await expect(state).toHaveAttribute('data-state', /^low=20 high=50 /);
  await lower.press('End');
  await expect(state).toHaveAttribute('data-state', /^low=50 high=50 /);
  await lower.press('ArrowRight');
  await upper.press('Home');
  await expect(state).toHaveAttribute('data-state', /^low=50 high=50 /);

  // A press near the end of the track is the upper thumb's; one near the start the lower's.
  // The box is wider than the thumb's travel, so a share of it is not that share of the range.
  // Each press ends, so the pair it left is reported.
  const read = async () => /^low=(\d+) high=(\d+) done=(\S+)$/.exec((await state.getAttribute('data-state')) ?? '')!.slice(1);
  // Raw touches skip actionability: on a tall window the track may sit in the system gesture zone.
  await lower.scrollIntoView();
  const track = await lower.boundingBox();
  const y = track.y + track.height / 2;
  await (await view.touch([track.x + track.width * 0.8, y])).up();
  await expect(state).toHaveAttribute('data-state', /^low=50 high=(70|80|90) /);
  let [low, high, done] = await read();
  expect(done).toBe(`${low}-${high}`);
  await (await view.touch([track.x + track.width * 0.2, y])).up();
  await expect(state).toHaveAttribute('data-state', new RegExp(`^low=(10|20|30) high=${high} `));
  [low, high, done] = await read();
  expect(done).toBe(`${low}-${high}`);
});

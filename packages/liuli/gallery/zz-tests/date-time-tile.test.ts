import { expect, spec, type TestView } from '@lingxia/test';
import { reveal, type Box } from './finger.js';

// DateTimeInput's parts have no least width of their own; the View keeps each editor as wide as its
// widest value, so a part that is drawn whole inside its container shows its whole value.
const inside = (part: Box, box: Box, inset: number) => {
  expect(part.x).toBeGreaterThanOrEqual(box.x + inset);
  expect(part.x + part.width).toBeLessThanOrEqual(box.x + box.width - inset);
};
const parts = async (view: TestView, id: string) =>
  [await view.testId(`${id}-date`).boundingBox(), await view.testId(`${id}-time`).boundingBox()];

spec('category-inputs: the DateTimeInput tile holds both parts inside its stage', {
  id: 'DATE-TIME-TILE-001', start: { page: 'category-inputs' }, timeout: 60_000,
}, async (t) => {
  const view = t.app.view;
  const tile = await reveal(view, view.testId('open-date-time-input'));
  const [date, time] = await parts(view, 'tile-when');
  // The stage's own padding is 12 pt; the editors sit inside their bordered boxes, further in.
  for (const part of [date, time]) inside(part, tile, 12);
  // A phone's tile is about 148 pt inside: too narrow for the pair, so the time goes under the date.
  if ((await view.viewport()).width < 700) expect(time.y, 'on a phone the time goes under the date').toBeGreaterThan(date.y + date.height);
});

spec('date-time-input: side by side from 280 pt, the time under the date below it', {
  id: 'DATE-TIME-WRAP-001', start: { page: 'date-time-input' }, timeout: 60_000,
}, async (t) => {
  const view = t.app.view;
  // Reminder fills the page's width, 280 pt or more on every preset: one row.
  const [date, time] = await parts(view, 'remind');
  expect(time.y, 'the pair shares a row').toBe(date.y);
  expect(time.x).toBeGreaterThan(date.x + date.width);
  // Narrow is 224 pt wide: two rows, each part inside the column.
  const column = await reveal(view, view.testId('due'));
  const [due, at] = await parts(view, 'due');
  expect(at.y, 'the time goes under the date').toBeGreaterThan(due.y + due.height);
  for (const part of [due, at]) inside(part, column, 0);
});
